//! سرویس پرس‌وجو — فهرست صفحه‌بندی‌شده، تجمیع، عملکرد گروهی، داشبورد پروجکشن.

use crate::error::QueryError;
use crate::filter::{build_filter, lookup_field, FilterNode, SqlFilter};
use crate::projection::{self, DailySummaryRow};
use crate::stats::{
    fold_equity, weekday_label, CoreStats, Dimension, EquityPoint, GroupStat, HeatCell,
};
use aria_storage_engine::Database;
use aria_contracts::EventEnvelope;
use aria_schema_engine::model::StorageType;
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, ErrorCode};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// حداکثر اندازه صفحه مجاز — سقف بودجه پرس‌وجو برای فهرست‌ها.
pub const MAX_PAGE_SIZE: u32 = 200;
/// بودجه پیش‌فرض گام‌های ماشین مجازی SQLite برای هر پرس‌وجو.
pub const DEFAULT_ROW_BUDGET: u64 = 20_000_000;
/// نوع رویداد باطل‌سازی آمار (هم‌ارز aria_domain_engine::events::event_type::STATS_INVALIDATED).
pub const STATS_INVALIDATED_EVENT: &str = "domain.stats_invalidated";

/// یک سطر خلاصه فهرست معاملات.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeListRow {
    pub id: String,
    pub account_id: String,
    pub symbol_id: String,
    pub direction: String,
    pub status: String,
    pub strategy: Option<String>,
    pub timeframe: Option<String>,
    pub session: Option<String>,
    pub entry_time: Option<String>,
    pub exit_time: Option<String>,
    pub realized_pnl: Option<f64>,
    pub realized_r: Option<f64>,
}

/// خروجی صفحه‌بندی‌شده فهرست.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedTrades {
    pub items: Vec<TradeListRow>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

/// گزارش داشبورد از پروجکشن (بدون محاسبه سنگین روی داده خام).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardReport {
    /// زمان آخرین بازسازی پروجکشن
    pub refreshed_at: Option<String>,
    /// آیا در همین فراخوانی بازسازی انجام شد؟
    pub refreshed_now: bool,
    pub days: Vec<DailySummaryRow>,
}

/// اطلاعات پرس‌وجوپذیری یک فیلد سفارشی برای UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatFieldInfo {
    pub technical_key: String,
    pub display_label: String,
    pub storage_type: String,
    pub stat_enabled: bool,
    pub analysis_enabled: bool,
}

/// اجرای `f` با بودجه گام‌های ماشین مجازی SQLite؛ در سرریز، خطای ۱۷۰۲.
fn with_row_budget<T>(
    conn: &Connection,
    budget: u64,
    f: impl FnOnce() -> Result<T, QueryError>,
) -> Result<T, QueryError> {
    let used = Arc::new(AtomicU64::new(0));
    let counter = used.clone();
    conn.progress_handler(1_000, Some(move || -> bool {
        let n = counter.fetch_add(1_000, Ordering::Relaxed) + 1_000;
        n > budget
    }));
    let result = f();
    conn.progress_handler(0, None::<fn() -> bool>);
    match result {
        Err(QueryError::Storage(e))
            if e.sqlite_error_code() == Some(ErrorCode::OperationInterrupted) =>
        {
            Err(QueryError::budget_exceeded(
                "پرس‌وجو از بودجه گام‌های مجاز فراتر رفت",
            ))
        }
        other => other,
    }
}

/// سطر تجمیع مرکبی: (کل، بسته، برد، باخت، سربه‌سر، میانگین R، جمع PnL).
type AggregateRow = (
    i64,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<f64>,
    Option<f64>,
);

/// استخراج شرط پارامتری از درخت فیلتر با اعتبارسنجی فیلدهای سفارشی.
fn prepared_filter(conn: &Connection, node: &FilterNode) -> Result<SqlFilter, QueryError> {
    build_filter(conn, node)
}

/// پسوند شرط «حذف‌نشده» به WHERE موجود — معاملات حذف‌نرم هرگز در نتایج ظاهر نمی‌شوند.
fn and_not_deleted(where_clause: &str) -> String {
    if where_clause.is_empty() {
        " WHERE journal_trades.deleted_at IS NULL".to_string()
    } else {
        format!("{where_clause} AND journal_trades.deleted_at IS NULL")
    }
}

fn row_to_list_row(r: &rusqlite::Row<'_>) -> Result<TradeListRow, rusqlite::Error> {
    Ok(TradeListRow {
        id: r.get(0)?,
        account_id: r.get(1)?,
        symbol_id: r.get(2)?,
        direction: r.get(3)?,
        status: r.get(4)?,
        strategy: r.get(5)?,
        timeframe: r.get(6)?,
        session: r.get(7)?,
        entry_time: r.get(8)?,
        exit_time: r.get(9)?,
        realized_pnl: r.get(10)?,
        realized_r: r.get(11)?,
    })
}

const LIST_COLUMNS: &str = "id, account_id, symbol_id, direction, status, strategy, timeframe, \
     session, entry_time, exit_time, realized_pnl, realized_r";

/// عبارت کلید گروه‌بندی برای فیلد سفارشی — تایپ‌دار بر مبنای نوع ذخیره‌سازی.
fn custom_group_expr(storage: StorageType) -> (&'static str, Option<&'static str>) {
    match storage {
        StorageType::Text | StorageType::LongText | StorageType::Enum => ("fv.text_value", None),
        StorageType::MultiEnum | StorageType::TagSet => ("fv.json_value", None),
        StorageType::Integer | StorageType::Rating => {
            ("CAST(fv.integer_value AS TEXT)", Some("fv.integer_value"))
        }
        StorageType::Decimal => ("CAST(fv.decimal_value AS TEXT)", Some("fv.decimal_value")),
        StorageType::Boolean => (
            "CASE fv.boolean_value WHEN 1 THEN 'بله' WHEN 0 THEN 'خیر' END",
            None,
        ),
        StorageType::Datetime => ("fv.datetime_value", None),
    }
}

/// برچسب نمایشی گروه فیلد سفارشی.
fn custom_group_label(storage: StorageType, key: &str) -> String {
    match storage {
        StorageType::Boolean => match key {
            "بله" | "1" => "بله".into(),
            "خیر" | "0" => "خیر".into(),
            other => other.into(),
        },
        _ => key.to_string(),
    }
}

/// سرویس پرس‌وجوی کرنل — تنها راه دسترسی پلاگین‌ها به داده (بدون SQL مستقیم).
pub struct QueryService<'a> {
    db: &'a Database,
}

impl<'a> QueryService<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

/// کلیدهای مجاز مرتب‌سازی فهرست — هر کلید دیگر به پیش‌فرض می‌رود.
/// (whitelist — هیچ عبارت SQL از ورودی بیرونی ساخته نمی‌شود)
fn sort_column(key: &str) -> Option<&'static str> {
    match key {
        "entry_time" => Some("entry_time"),
        "exit_time" => Some("exit_time"),
        "realized_pnl" => Some("realized_pnl"),
        "realized_r" => Some("realized_r"),
        _ => None,
    }
}

/// فهرست صفحه‌بندی‌شده معاملات با فیلتر مرکب و مرتب‌سازی فهرست‌شده.
///
/// پیش‌فرض مرتب‌سازی: `entry_time DESC, id DESC`. کلید غیرمجاز هم
/// به همین پیش‌فرض می‌رود (نه خطا) تا فرانت‌اند بتواند کلید دلخواه بفرستد.
pub fn list_trades(
    &self,
    node: &FilterNode,
    page: u32,
    page_size: u32,
    sort_key: Option<&str>,
    sort_desc: bool,
) -> Result<PagedTrades, QueryError> {
    if page == 0 {
        return Err(QueryError::invalid_query("شماره صفحه از ۱ شروع می‌شود"));
    }
    if page_size == 0 {
        return Err(QueryError::invalid_query("اندازه صفحه باید حداقل ۱ باشد"));
    }
    if page_size > MAX_PAGE_SIZE {
        return Err(QueryError::budget_exceeded(format!(
            "اندازه صفحه حداکثر {MAX_PAGE_SIZE} است"
        )));
    }
    let order = match sort_key.and_then(Self::sort_column) {
        Some(col) => {
            let dir = if sort_desc { "DESC" } else { "ASC" };
            // NULLها همیشه در انتهای مرتب‌سازی می‌مانند
            format!("{col} IS NULL, {col} {dir}, id DESC")
        }
        None => "entry_time DESC, id DESC".to_string(),
    };
    let conn = self.db.lock();
    with_row_budget(&conn, DEFAULT_ROW_BUDGET, || {
        let filter = prepared_filter(&conn, node)?;
        let base = format!("FROM journal_trades{}", and_not_deleted(&filter.where_clause));
        let total: i64 = conn.query_row(
            &format!("SELECT COUNT(*) {base}"),
            rusqlite::params_from_iter(filter.params.clone()),
            |r| r.get(0),
        )?;
        let sql = format!("SELECT {LIST_COLUMNS} {base} ORDER BY {order} LIMIT ? OFFSET ?");
            let mut params = filter.params.clone();
            params.push(SqlValue::Integer(page_size as i64));
            params.push(SqlValue::Integer(((page - 1) as i64) * page_size as i64));
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query(rusqlite::params_from_iter(params))?;
            let mut items = Vec::with_capacity(page_size as usize);
            while let Some(r) = rows.next()? {
                items.push(row_to_list_row(r)?);
            }
            Ok(PagedTrades {
                items,
                total,
                page,
                page_size,
            })
        })
    }

    /// آمار مرکبی روی معاملات منطبق.
    pub fn aggregate(&self, node: &FilterNode) -> Result<CoreStats, QueryError> {
        let conn = self.db.lock();
        with_row_budget(&conn, DEFAULT_ROW_BUDGET, || {
            let filter = prepared_filter(&conn, node)?;
            let base = format!("FROM journal_trades{}", and_not_deleted(&filter.where_clause));
            let mut stats = CoreStats::default();
            let params = rusqlite::params_from_iter(filter.params.clone());
            let aggregate: AggregateRow = conn.query_row(
                &format!(
                    "SELECT COUNT(*),
                        SUM(CASE WHEN realized_pnl IS NOT NULL THEN 1 ELSE 0 END),
                        SUM(CASE WHEN realized_pnl > 0 THEN 1 ELSE 0 END),
                        SUM(CASE WHEN realized_pnl < 0 THEN 1 ELSE 0 END),
                        SUM(CASE WHEN realized_pnl = 0 THEN 1 ELSE 0 END),
                        AVG(realized_r),
                        SUM(realized_pnl)
                     {base}"
                ),
                params,
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )?;
            let (total, closed_raw, wins_raw, losses_raw, breakevens_raw, avg_r, total_pnl) =
                aggregate;
            // در مجموعه خالی SUM برابر NULL است نه صفر
            let closed = closed_raw.unwrap_or(0);
            stats.total_trades = total;
            stats.closed_trades = closed;
            stats.wins = wins_raw.unwrap_or(0);
            stats.losses = losses_raw.unwrap_or(0);
            stats.breakevens = breakevens_raw.unwrap_or(0);
            stats.win_rate = if closed > 0 {
                Some(stats.wins as f64 / closed as f64 * 100.0)
            } else {
                None
            };
            stats.avg_r = avg_r;
            stats.total_pnl = total_pnl;

            // حداکثر افت سرمایه از PnL روزانه بسته‌ها
            let daily = self.daily_pnl(&conn, &filter)?;
            let (_, max_dd) = fold_equity(&daily);
            stats.max_drawdown = max_dd;
            Ok(stats)
        })
    }

    /// منحنی سرمایه از PnL روزانه معاملات بسته.
    pub fn equity_curve(&self, node: &FilterNode) -> Result<Vec<EquityPoint>, QueryError> {
        let conn = self.db.lock();
        with_row_budget(&conn, DEFAULT_ROW_BUDGET, || {
            let filter = prepared_filter(&conn, node)?;
            let daily = self.daily_pnl(&conn, &filter)?;
            Ok(fold_equity(&daily).0)
        })
    }

    /// نقشه حرارتی زمان — تجمیع معاملات بسته روی ماتریس روز هفته × ساعت
    /// بستن (فاز ۱.۱۴). فقط معاملات با PnL قطعی‌شده.
    pub fn heatmap(&self, node: &FilterNode) -> Result<Vec<HeatCell>, QueryError> {
        let conn = self.db.lock();
        with_row_budget(&conn, DEFAULT_ROW_BUDGET, || {
            let filter = prepared_filter(&conn, node)?;
            let where_extra = if filter.where_clause.is_empty() {
                " WHERE journal_trades.deleted_at IS NULL AND realized_pnl IS NOT NULL".to_string()
            } else {
                format!(
                    "{} AND journal_trades.deleted_at IS NULL AND realized_pnl IS NOT NULL",
                    filter.where_clause
                )
            };
            let sql = format!(
                "SELECT CAST(strftime('%w', ts) AS INTEGER) AS wd,
                        CAST(strftime('%H', ts) AS INTEGER) AS hr,
                        COUNT(*),
                        SUM(realized_pnl)
                 FROM (
                     SELECT COALESCE(exit_time, entry_time, created_at) AS ts, realized_pnl
                     FROM journal_trades{where_extra}
                 )
                 GROUP BY wd, hr ORDER BY wd, hr"
            );
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query(rusqlite::params_from_iter(filter.params.clone()))?;
            let mut out = Vec::new();
            while let Some(r) = rows.next()? {
                out.push(HeatCell {
                    weekday: r.get(0)?,
                    hour: r.get(1)?,
                    trades: r.get(2)?,
                    total_pnl: r.get(3)?,
                });
            }
            Ok(out)
        })
    }

    /// PnL روزانه معاملات بسته به مبنای روز بستن — مرتب‌شده صعودی.
    fn daily_pnl(
        &self,
        conn: &Connection,
        filter: &SqlFilter,
    ) -> Result<Vec<(String, f64)>, QueryError> {
        let where_extra = if filter.where_clause.is_empty() {
            " WHERE journal_trades.deleted_at IS NULL AND realized_pnl IS NOT NULL".to_string()
        } else {
            format!(
                "{} AND journal_trades.deleted_at IS NULL AND realized_pnl IS NOT NULL",
                filter.where_clause
            )
        };
        let sql = format!(
            "SELECT substr(COALESCE(exit_time, entry_time, created_at), 1, 10) AS d,
                    SUM(realized_pnl)
             FROM journal_trades{where_extra}
             GROUP BY d ORDER BY d"
        );
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(filter.params.clone()))?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push((r.get::<_, String>(0)?, r.get::<_, f64>(1)?));
        }
        Ok(out)
    }

    /// عملکرد گروهی روی یک بُعد (نماد، استراتژی، …، فیلد سفارشی).
    pub fn performance_by(
        &self,
        dim: &Dimension,
        node: &FilterNode,
    ) -> Result<Vec<GroupStat>, QueryError> {
        let conn = self.db.lock();
        with_row_budget(&conn, DEFAULT_ROW_BUDGET, || {
            let filter = prepared_filter(&conn, node)?;

            // ساخت FROM / key_expr / label-map بر اساس بُعد
            let mut extra_join = String::new();
            let mut extra_wheres: Vec<String> = vec!["journal_trades.deleted_at IS NULL".into()];
            let mut extra_params: Vec<SqlValue> = Vec::new();
            let key_expr: String;
            let mut avg_expr: Option<String> = None;
            let mut is_weekday = false;
            let mut custom_storage: Option<StorageType> = None;

            match dim {
                Dimension::Symbol => {
                    extra_join =
                        " LEFT JOIN symbols s ON s.id = journal_trades.symbol_id".to_string();
                    key_expr = "COALESCE(s.name, 'نامشخص')".to_string();
                }
                Dimension::Strategy => {
                    key_expr = "COALESCE(journal_trades.strategy, 'نامشخص')".to_string();
                }
                Dimension::Timeframe => {
                    key_expr = "COALESCE(journal_trades.timeframe, 'نامشخص')".to_string();
                }
                Dimension::Session => {
                    key_expr = "COALESCE(journal_trades.session, 'نامشخص')".to_string();
                }
                Dimension::Weekday => {
                    key_expr =
                        "strftime('%w', COALESCE(journal_trades.exit_time, journal_trades.entry_time, journal_trades.created_at))"
                            .to_string();
                    is_weekday = true;
                }
                Dimension::CustomField(field_key) => {
                    let meta = lookup_field(&conn, field_key)?.ok_or_else(|| {
                        QueryError::field_not_queryable(format!("فیلد «{field_key}» یافت نشد"))
                    })?;
                    if !meta.active {
                        return Err(QueryError::field_not_queryable(format!(
                            "فیلد «{field_key}» غیرفعال است"
                        )));
                    }
                    if !meta.stat_enabled && !meta.analysis_enabled {
                        return Err(QueryError::field_not_queryable(format!(
                            "فیلد «{field_key}» برای آمار/تحلیل فعال نشده است"
                        )));
                    }
                    let column = meta.storage_type.value_column();
                    let (expr, numeric) = custom_group_expr(meta.storage_type);
                    key_expr = format!("COALESCE({expr}, 'نامشخص')");
                    // شرط کلید فیلد در WHERE می‌آید (نه JOIN) تا ترتیب پارامترهای
                    // موقعیتی `?` حفظ شود: اول شرط‌های فیلتر، بعد کلید فیلد.
                    extra_join = " INNER JOIN field_values fv ON fv.trade_id = journal_trades.id
                          INNER JOIN custom_fields cf ON cf.id = fv.field_id"
                        .to_string();
                    extra_wheres.push("cf.technical_key = ?".to_string());
                    extra_wheres.push(format!("fv.{column} IS NOT NULL"));
                    extra_params.push(SqlValue::Text(field_key.clone()));
                    avg_expr = numeric.map(|c| format!("AVG({c})"));
                    custom_storage = Some(meta.storage_type);
                }
            }

            let where_extra = if filter.where_clause.is_empty() {
                if extra_wheres.is_empty() {
                    String::new()
                } else {
                    format!(" WHERE {}", extra_wheres.join(" AND "))
                }
            } else if extra_wheres.is_empty() {
                filter.where_clause.clone()
            } else {
                format!("{} AND {}", filter.where_clause, extra_wheres.join(" AND "))
            };
            let sql = format!(
                "SELECT {key_expr} AS gkey, COUNT(*),
                        SUM(CASE WHEN journal_trades.realized_pnl > 0 THEN 1 ELSE 0 END),
                        SUM(CASE WHEN journal_trades.realized_pnl IS NOT NULL THEN 1 ELSE 0 END),
                        SUM(journal_trades.realized_pnl),
                        AVG(journal_trades.realized_r){avg_select}
                 FROM journal_trades{extra_join}{where_extra}
                 GROUP BY gkey ORDER BY 2 DESC, 1",
                avg_select = avg_expr
                    .as_ref()
                    .map(|e| format!(", {e}"))
                    .unwrap_or_default(),
            );

            let mut params = filter.params.clone();
            params.extend(extra_params);
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query(rusqlite::params_from_iter(params))?;
            let mut out = Vec::new();
            while let Some(r) = rows.next()? {
                let key: String = r.get(0)?;
                let trades: i64 = r.get(1)?;
                let wins: i64 = r.get(2)?;
                let closed: i64 = r.get(3)?;
                let total_pnl: Option<f64> = r.get(4)?;
                let avg_r: Option<f64> = r.get(5)?;
                let avg_custom: Option<f64> = if avg_expr.is_some() {
                    r.get(6)?
                } else {
                    None
                };
                let label = if is_weekday {
                    weekday_label(&key)
                } else if let Some(st) = custom_storage {
                    custom_group_label(st, &key)
                } else {
                    key.clone()
                };
                out.push(GroupStat {
                    key,
                    label,
                    trades,
                    wins,
                    // مخرج نرخ برد = معاملات بسته (هم‌ارز aggregate)، نه همه ردیف‌ها
                    win_rate: if closed > 0 {
                        Some(wins as f64 / closed as f64 * 100.0)
                    } else {
                        None
                    },
                    total_pnl,
                    avg_r,
                    avg_custom_value: avg_custom,
                });
            }
            Ok(out)
        })
    }

    /// فهرست فیلدهای سفارشی پرس‌وجوپذیر برای UI.
    pub fn stat_fields(&self) -> Result<Vec<StatFieldInfo>, QueryError> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare(
            "SELECT technical_key, display_label, storage_type, stat_enabled, analysis_enabled
             FROM custom_fields WHERE active = 1 AND (stat_enabled = 1 OR analysis_enabled = 1)
             ORDER BY display_order, technical_key",
        )?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(StatFieldInfo {
                technical_key: r.get(0)?,
                display_label: r.get(1)?,
                storage_type: r.get(2)?,
                stat_enabled: r.get::<_, i64>(3)? != 0,
                analysis_enabled: r.get::<_, i64>(4)? != 0,
            });
        }
        Ok(out)
    }

    /// گزارش داشبورد از پروجکشن — در صورت کهنگی، ابتدا بازسازی می‌شود.
    pub fn dashboard(&self, account_id: Option<&str>) -> Result<DashboardReport, QueryError> {
        let mut conn = self.db.lock();
        projection::ensure_tables(&conn)?;
        let refreshed_now = projection::refresh_if_stale(&mut conn)?;
        let refreshed_at = projection::meta_value(&conn, "refreshed_at")?;
        let days = projection::read_summary(&conn, account_id)?;
        Ok(DashboardReport {
            refreshed_at,
            refreshed_now,
            days,
        })
    }

    /// باطل‌سازی دستی پروجکشن داشبورد.
    pub fn invalidate(&self) -> Result<(), QueryError> {
        let conn = self.db.lock();
        projection::invalidate(&conn)
    }

    /// بررسی کهنگی پروجکشن.
    pub fn projection_is_stale(&self) -> Result<bool, QueryError> {
        let conn = self.db.lock();
        projection::is_stale(&conn)
    }

    /// نسخه فعلی پروجکشن.
    pub fn projection_version(&self) -> Result<i64, QueryError> {
        let conn = self.db.lock();
        projection::version(&conn)
    }

    /// پردازش پاکت رویداد — باطل‌سازی پروجکشن روی StatsInvalidated.
    /// خروجی: آیا رویداد مصرف شد؟
    pub fn handle_event(&self, env: &EventEnvelope) -> Result<bool, QueryError> {
        if env.event_type == STATS_INVALIDATED_EVENT {
            self.invalidate()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{CustomFieldFilter, CustomFieldOp, CustomValue, TradeFilter, TradeResult};
    use aria_domain_engine::events::event_type;
    use aria_schema_engine::model::{FieldDefinition, SemanticType, StorageType};
    use aria_schema_engine::service::SchemaService;
    use aria_storage_engine::migrations;
    use rusqlite::Connection;

    fn db() -> Database {
        let db = Database::open_memory(Some("query-pass-1")).unwrap();
        {
            let mut conn = db.lock();
            migrations::run_migrations(&mut conn, |_| Ok(())).unwrap();
            conn.execute(
                "INSERT INTO profiles (id, name, created_at, updated_at)
                 VALUES ('p1', 'پروفایل آزمون', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .unwrap();
        }
        db
    }

    /// ساخت حساب و نماد و درج مستقیم معامله (سطح پایین برای کنترل دقیق داده آزمون).
    #[allow(clippy::too_many_arguments)]
    fn seed_trade(
        conn: &Connection,
        id: &str,
        account: &str,
        symbol: &str,
        status: &str,
        entry: &str,
        exit: Option<&str>,
        pnl: Option<f64>,
        r: Option<f64>,
        strategy: Option<&str>,
    ) {
        conn.execute(
            "INSERT OR IGNORE INTO trading_accounts (id, profile_id, name, created_at, updated_at)
             VALUES (?1, 'p1', 'حساب اصلی', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![account],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO symbols (id, name, created_at)
             VALUES (?1, ?2, '2026-01-01T00:00:00Z')",
            rusqlite::params![symbol, symbol],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO journal_trades
                (id, account_id, symbol_id, direction, status, strategy, timeframe, session,
                 tags, emotions, entry_time, exit_time, realized_pnl, realized_r,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, 'buy', ?4, ?5, 'H1', 'لندن', 'اسکالپ،روند', 'طمع',
                     ?6, ?7, ?8, ?9, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![
                id,
                account,
                symbol,
                status,
                strategy,
                entry,
                exit,
                pnl,
                r
            ],
        )
        .unwrap();
    }

    fn define_field(svc: &SchemaService, key: &str, st: StorageType, filterable: bool, stat: bool) -> String {
        let mut def = FieldDefinition::new(key, &format!("برچسب-{key}"), st, SemanticType::Number);
        def.filterable = filterable;
        def.stat_enabled = stat;
        def.analysis_enabled = stat;
        svc.define_field(def).unwrap().id
    }

    // ---------- فیلترهای ساده ----------

    #[test]
    fn filter_by_symbol_and_direction() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "XAUUSD", "closed", "2026-02-01T10:00:00Z", Some("2026-02-01T11:00:00Z"), Some(100.0), Some(2.0), Some("برک‌اوت"));
            seed_trade(&conn, "t2", "acc1", "EURUSD", "closed", "2026-02-02T10:00:00Z", Some("2026-02-02T11:00:00Z"), Some(-50.0), Some(-1.0), Some("برگشت"));
            seed_trade(&conn, "t3", "acc1", "XAUUSD", "open", "2026-02-03T10:00:00Z", None, None, None, Some("برک‌اوت"));
        }
        let svc = QueryService::new(&db);
        let sym = db
            .lock()
            .query_row("SELECT id FROM symbols WHERE name = 'XAUUSD'", [], |r| r.get::<_, String>(0))
            .unwrap();
        let page = svc
            .list_trades(
                &FilterNode::Simple(Box::new(TradeFilter {
                    symbol_id: Some(sym.clone()),
                    direction: Some("buy".into()),
                    ..Default::default()
                })),
                1,
                50, None, false)
            .unwrap();
        assert_eq!(page.total, 2);
        assert!(page.items.iter().all(|t| t.symbol_id == sym));
    }

    #[test]
    fn filter_by_result_and_status() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", Some("2026-02-01T11:00:00Z"), Some(100.0), Some(2.0), None);
            seed_trade(&conn, "t2", "acc1", "S", "closed", "2026-02-02T10:00:00Z", Some("2026-02-02T11:00:00Z"), Some(-50.0), Some(-1.0), None);
            seed_trade(&conn, "t3", "acc1", "S", "closed", "2026-02-03T10:00:00Z", Some("2026-02-03T11:00:00Z"), Some(0.0), Some(0.0), None);
            seed_trade(&conn, "t4", "acc1", "S", "open", "2026-02-04T10:00:00Z", None, None, None, None);
        }
        let svc = QueryService::new(&db);
        for (result, expected) in [
            (TradeResult::Win, 1),
            (TradeResult::Loss, 1),
            (TradeResult::Breakeven, 1),
            (TradeResult::Open, 1),
        ] {
            let page = svc
                .list_trades(
                    &FilterNode::Simple(Box::new(TradeFilter {
                        result: Some(result),
                        ..Default::default()
                    })),
                    1,
                    50, None, false)
                .unwrap();
            assert_eq!(page.total, expected, "result={result:?}");
        }
    }

    #[test]
    fn filter_by_date_range_is_inclusive_on_date_only() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T09:00:00Z", None, Some(1.0), Some(1.0), None);
            seed_trade(&conn, "t2", "acc1", "S", "closed", "2026-02-02T23:00:00Z", None, Some(1.0), Some(1.0), None);
            seed_trade(&conn, "t3", "acc1", "S", "closed", "2026-02-03T01:00:00Z", None, Some(1.0), Some(1.0), None);
        }
        let svc = QueryService::new(&db);
        let page = svc
            .list_trades(
                &FilterNode::Simple(Box::new(TradeFilter {
                    entry_from: Some("2026-02-02".into()),
                    entry_to: Some("2026-02-02".into()),
                    ..Default::default()
                })),
                1,
                50, None, false)
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "t2");
    }

    #[test]
    fn filter_tags_and_emotions_contains() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", None, Some(1.0), Some(1.0), None);
            seed_trade(&conn, "t2", "acc1", "S", "closed", "2026-02-02T10:00:00Z", None, Some(1.0), Some(1.0), None);
        }
        let conn = db.lock();
        conn.execute("UPDATE journal_trades SET tags = 'اسکالپ،خبر،روند' WHERE id = 't1'", [])
            .unwrap();
        conn.execute("UPDATE journal_trades SET emotions = 'ترس' WHERE id = 't2'", [])
            .unwrap();
        drop(conn);
        let svc = QueryService::new(&db);
        let page = svc
            .list_trades(
                &FilterNode::Simple(Box::new(TradeFilter {
                    tags: Some("خبر".into()),
                    ..Default::default()
                })),
                1,
                50, None, false)
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "t1");
        let page = svc
            .list_trades(
                &FilterNode::Simple(Box::new(TradeFilter {
                    emotions: Some("ترس".into()),
                    ..Default::default()
                })),
                1,
                50, None, false)
            .unwrap();
        assert_eq!(page.total, 1);
    }

    // ---------- فیلترهای ترکیبی ----------

    #[test]
    fn combined_and_or_groups() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", Some("2026-02-01T11:00:00Z"), Some(100.0), Some(2.0), Some("برک‌اوت"));
            seed_trade(&conn, "t2", "acc1", "S", "closed", "2026-02-02T10:00:00Z", Some("2026-02-02T11:00:00Z"), Some(-50.0), Some(-1.0), Some("برک‌اوت"));
            seed_trade(&conn, "t3", "acc1", "S", "closed", "2026-02-03T10:00:00Z", Some("2026-02-03T11:00:00Z"), Some(20.0), Some(0.5), Some("برگشت"));
        }
        let svc = QueryService::new(&db);
        // (برک‌اوت AND برد) OR برگشت → t1 یا t3
        let node = FilterNode::Any {
            children: vec![
                FilterNode::All {
                    children: vec![FilterNode::Simple(Box::new(TradeFilter {
                        strategy: Some("برک‌اوت".into()),
                        result: Some(TradeResult::Win),
                        ..Default::default()
                    }))],
                },
                FilterNode::Simple(Box::new(TradeFilter {
                    strategy: Some("برگشت".into()),
                    ..Default::default()
                })),
            ],
        };
        let page = svc.list_trades(&node, 1, 50, None, false).unwrap();
        assert_eq!(page.total, 2);
        let ids: Vec<&str> = page.items.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"t1") && ids.contains(&"t3"));
    }

    #[test]
    fn any_group_must_not_be_empty() {
        let db = db();
        let svc = QueryService::new(&db);
        let err = svc
            .list_trades(&FilterNode::Any { children: vec![] }, 1, 50, None, false)
            .unwrap_err();
        assert_eq!(err.code(), 1701);
    }

    // ---------- فیلدهای سفارشی ----------

    #[test]
    fn custom_field_filters_all_ops() {
        let db = db();
        let field_id;
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", None, Some(1.0), Some(1.0), None);
            seed_trade(&conn, "t2", "acc1", "S", "closed", "2026-02-02T10:00:00Z", None, Some(1.0), Some(1.0), None);
            seed_trade(&conn, "t3", "acc1", "S", "closed", "2026-02-03T10:00:00Z", None, Some(1.0), Some(1.0), None);
        }
        {
            let schema = SchemaService::new(&db);
            field_id = define_field(&schema, "confidence", StorageType::Integer, true, false);
            schema.set_value("t1", &field_id, &serde_json::json!(70)).unwrap();
            schema.set_value("t2", &field_id, &serde_json::json!(30)).unwrap();
        }
        let svc = QueryService::new(&db);
        let cf = |op| FilterNode::Custom(CustomFieldFilter {
            field_key: "confidence".into(),
            op,
        });
        let count = |node| svc.list_trades(&node, 1, 50, None, false).unwrap().total;
        use CustomFieldOp as Op;
        use CustomValue as V;
        assert_eq!(count(cf(Op::Equals { value: V::Integer { value: 70 } })), 1);
        assert_eq!(count(cf(Op::NotEquals { value: V::Integer { value: 70 } })), 1);
        assert_eq!(count(cf(Op::Min { value: 50.0 })), 1);
        assert_eq!(count(cf(Op::Max { value: 50.0 })), 1);
        assert_eq!(count(cf(Op::In { values: vec![V::Integer { value: 30 }, V::Integer { value: 70 }] })), 2);
        assert_eq!(count(cf(Op::NotIn { values: vec![V::Integer { value: 70 }] })), 1);
        assert_eq!(count(cf(Op::Exists)), 2);
        // فیلد بدون مقدار → t3
        assert_eq!(count(FilterNode::Custom(CustomFieldFilter {
            field_key: "confidence".into(),
            op: Op::Equals { value: V::Integer { value: 99 } },
        })), 0);
    }

    #[test]
    fn custom_field_text_contains_and_exists() {
        let db = db();
        let field_id;
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", None, Some(1.0), Some(1.0), None);
            seed_trade(&conn, "t2", "acc1", "S", "closed", "2026-02-02T10:00:00Z", None, Some(1.0), Some(1.0), None);
        }
        {
            let schema = SchemaService::new(&db);
            let mut def = FieldDefinition::new("setup_note", "یادداشت ستاپ", StorageType::Text, SemanticType::ShortText);
            def.filterable = true;
            field_id = schema.define_field(def).unwrap().id;
            schema.set_value("t1", &field_id, &serde_json::json!("برک‌اوت نقدینگی")).unwrap();
        }
        let svc = QueryService::new(&db);
        let page = svc
            .list_trades(
                &FilterNode::Custom(CustomFieldFilter {
                    field_key: "setup_note".into(),
                    op: CustomFieldOp::Contains { value: "نقدینگی".into() },
                }),
                1,
                50, None, false)
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "t1");
    }

    #[test]
    fn non_filterable_field_is_rejected_1703() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", None, Some(1.0), Some(1.0), None);
        }
        {
            let schema = SchemaService::new(&db);
            define_field(&schema, "hidden_note", StorageType::Text, false, false);
        }
        let svc = QueryService::new(&db);
        let err = svc
            .list_trades(
                &FilterNode::Custom(CustomFieldFilter {
                    field_key: "hidden_note".into(),
                    op: CustomFieldOp::Exists,
                }),
                1,
                50, None, false)
            .unwrap_err();
        assert_eq!(err.code(), 1703);
        // فیلد ناموجود
        let err = svc
            .list_trades(
                &FilterNode::Custom(CustomFieldFilter {
                    field_key: "ناموجود".into(),
                    op: CustomFieldOp::Exists,
                }),
                1,
                50, None, false)
            .unwrap_err();
        assert_eq!(err.code(), 1703);
    }

    #[test]
    fn type_mismatch_in_custom_filter_is_1701() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", None, Some(1.0), Some(1.0), None);
        }
        {
            let schema = SchemaService::new(&db);
            define_field(&schema, "confidence", StorageType::Integer, true, false);
        }
        let svc = QueryService::new(&db);
        let err = svc
            .list_trades(
                &FilterNode::Custom(CustomFieldFilter {
                    field_key: "confidence".into(),
                    op: CustomFieldOp::Equals { value: CustomValue::Text { value: "۷۰".into() } },
                }),
                1,
                50, None, false)
            .unwrap_err();
        assert_eq!(err.code(), 1701);
        // min روی فیلد متنی → ۱۷۰۱
        {
            let schema = SchemaService::new(&db);
            let mut def = FieldDefinition::new("note_txt", "یادداشت", StorageType::Text, SemanticType::ShortText);
            def.filterable = true;
            schema.define_field(def).unwrap();
        }
        let err = svc
            .list_trades(
                &FilterNode::Custom(CustomFieldFilter {
                    field_key: "note_txt".into(),
                    op: CustomFieldOp::Min { value: 1.0 },
                }),
                1,
                50, None, false)
            .unwrap_err();
        assert_eq!(err.code(), 1701);
    }

    #[test]
    fn aggregate_on_empty_set_returns_zeroed_stats() {
        let db = db();
        let svc = QueryService::new(&db);
        let stats = svc
            .aggregate(&FilterNode::Simple(Box::new(TradeFilter {
                strategy: Some("ناموجود".into()),
                ..Default::default()
            })))
            .unwrap();
        assert_eq!(stats.total_trades, 0);
        assert_eq!(stats.closed_trades, 0);
        assert_eq!(stats.wins, 0);
        assert_eq!(stats.losses, 0);
        assert_eq!(stats.breakevens, 0);
        assert!(stats.win_rate.is_none());
        assert!(stats.avg_r.is_none());
        assert!(stats.total_pnl.is_none());
        assert!(stats.max_drawdown.is_none());
        assert!(svc.equity_curve(&FilterNode::empty()).unwrap().is_empty());
    }

    #[test]
    fn soft_deleted_trades_excluded_from_all_queries() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", Some("2026-02-01T11:00:00Z"), Some(100.0), Some(2.0), None);
            seed_trade(&conn, "t2", "acc1", "S", "closed", "2026-02-02T10:00:00Z", Some("2026-02-02T11:00:00Z"), Some(-50.0), Some(-1.0), None);
            // حذف نرم t2
            conn.execute(
                "UPDATE journal_trades SET deleted_at = '2026-03-01T00:00:00Z' WHERE id = 't2'",
                [],
            )
            .unwrap();
        }
        let svc = QueryService::new(&db);
        // فهرست
        let page = svc.list_trades(&FilterNode::empty(), 1, 50, None, false).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "t1");
        // تجمیع
        let stats = svc.aggregate(&FilterNode::empty()).unwrap();
        assert_eq!(stats.total_trades, 1);
        assert_eq!(stats.closed_trades, 1);
        assert_eq!(stats.wins, 1);
        assert_eq!(stats.losses, 0);
        // منحنی سرمایه — فقط t1
        let curve = svc.equity_curve(&FilterNode::empty()).unwrap();
        assert_eq!(curve.len(), 1);
        assert!((curve[0].cumulative_pnl - 100.0).abs() < 1e-9);
        // عملکرد گروهی
        let groups = svc.performance_by(&Dimension::Symbol, &FilterNode::empty()).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].trades, 1);
        // داشبورد پروجکشن
        svc.invalidate().unwrap();
        let dash = svc.dashboard(None).unwrap();
        let total_rows: i64 = dash.days.iter().map(|d| d.trades_count).sum();
        assert_eq!(total_rows, 1);
    }

    #[test]
    fn performance_by_win_rate_uses_closed_denominator() {
        let db = db();
        {
            let conn = db.lock();
            // GOLD: یک برد بسته + یک معامله باز → نرخ برد باید ۱۰۰٪ باشد نه ۵۰٪
            seed_trade(&conn, "t1", "acc1", "GOLD", "closed", "2026-02-01T10:00:00Z", Some("2026-02-01T11:00:00Z"), Some(100.0), Some(2.0), None);
            seed_trade(&conn, "t2", "acc1", "GOLD", "open", "2026-02-02T10:00:00Z", None, None, None, None);
        }
        let svc = QueryService::new(&db);
        let groups = svc.performance_by(&Dimension::Symbol, &FilterNode::empty()).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].trades, 2);
        assert_eq!(groups[0].wins, 1);
        assert!((groups[0].win_rate.unwrap() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn custom_field_boolean_equals_filter() {
        let db = db();
        let field_id;
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", None, Some(1.0), Some(1.0), None);
            seed_trade(&conn, "t2", "acc1", "S", "closed", "2026-02-02T10:00:00Z", None, Some(1.0), Some(1.0), None);
        }
        {
            let schema = SchemaService::new(&db);
            let mut def = FieldDefinition::new("followed_plan", "پایبندی به پلن", StorageType::Boolean, SemanticType::Boolean);
            def.filterable = true;
            field_id = schema.define_field(def).unwrap().id;
            schema.set_value("t1", &field_id, &serde_json::json!(true)).unwrap();
            schema.set_value("t2", &field_id, &serde_json::json!(false)).unwrap();
        }
        let svc = QueryService::new(&db);
        let count = |op| {
            svc.list_trades(
                &FilterNode::Custom(CustomFieldFilter {
                    field_key: "followed_plan".into(),
                    op,
                }),
                1,
                50, None, false)
            .unwrap()
            .total
        };
        use CustomValue as V;
        assert_eq!(count(CustomFieldOp::Equals { value: V::Boolean { value: true } }), 1);
        assert_eq!(count(CustomFieldOp::Equals { value: V::Boolean { value: false } }), 1);
        assert_eq!(count(CustomFieldOp::NotEquals { value: V::Boolean { value: true } }), 1);
        assert_eq!(count(CustomFieldOp::In { values: vec![V::Boolean { value: true }] }), 1);
        assert_eq!(count(CustomFieldOp::Exists), 2);
    }

    #[test]
    fn json_field_rejects_equality_ops_1701() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", None, Some(1.0), Some(1.0), None);
        }
        {
            let schema = SchemaService::new(&db);
            let mut def = FieldDefinition::new("mistake_tags", "برچسب اشتباه", StorageType::TagSet, SemanticType::Tag);
            def.filterable = true;
            schema.define_field(def).unwrap();
        }
        let svc = QueryService::new(&db);
        let run = |op| {
            svc.list_trades(
                &FilterNode::Custom(CustomFieldFilter {
                    field_key: "mistake_tags".into(),
                    op,
                }),
                1,
                50, None, false)
            .map(|p| p.total)
        };
        assert_eq!(run(CustomFieldOp::Equals { value: CustomValue::Text { value: "x".into() } }).unwrap_err().code(), 1701);
        assert_eq!(run(CustomFieldOp::NotEquals { value: CustomValue::Text { value: "x".into() } }).unwrap_err().code(), 1701);
        assert_eq!(run(CustomFieldOp::In { values: vec![CustomValue::Text { value: "x".into() }] }).unwrap_err().code(), 1701);
        assert_eq!(run(CustomFieldOp::NotIn { values: vec![CustomValue::Text { value: "x".into() }] }).unwrap_err().code(), 1701);
        // contains روی json مجاز است
        assert_eq!(run(CustomFieldOp::Contains { value: "تعجیل".into() }).unwrap(), 0);
        assert_eq!(run(CustomFieldOp::Exists).unwrap(), 0);
    }

    // ---------- تجمیع ----------

    // ---------- نقشه حرارتی ----------

    #[test]
    fn heatmap_groups_by_close_weekday_and_hour() {
        let db = db();
        {
            let conn = db.lock();
            // بسته: یکشنبه ۲۰ آوریل ۲۰۲۶ ساعت ۱۰ و ۱۱ UTC + دوشنبه ساعت ۱۰
            seed_trade(&conn, "h1", "acc1", "S", "closed", "2026-04-19T08:00:00Z", Some("2026-04-19T10:30:00Z"), Some(100.0), Some(1.0), None);
            seed_trade(&conn, "h2", "acc1", "S", "closed", "2026-04-19T09:00:00Z", Some("2026-04-19T11:15:00Z"), Some(50.0), Some(0.5), None);
            seed_trade(&conn, "h3", "acc1", "S", "closed", "2026-04-20T08:00:00Z", Some("2026-04-20T10:45:00Z"), Some(-25.0), Some(-0.25), None);
            // باز بدون PnL — نباید در نقشه حرارتی بیاید (هم‌تراز daily_pnl)
            seed_trade(&conn, "h4", "acc1", "S", "open", "2026-04-21T08:00:00Z", None, None, None, None);
        }
        let svc = QueryService::new(&db);
        let cells = svc.heatmap(&FilterNode::All { children: vec![] }).unwrap();
        assert_eq!(cells.len(), 3, "open trades and empty buckets are excluded");
        // 2026-04-19 یکشنبه است (wd=0)، ساعت ۱۰ و ۱۱
        let sun10 = cells.iter().find(|c| c.weekday == 0 && c.hour == 10).unwrap();
        assert_eq!(sun10.trades, 1);
        assert!((sun10.total_pnl.unwrap() - 100.0).abs() < 1e-9);
        let sun11 = cells.iter().find(|c| c.weekday == 0 && c.hour == 11).unwrap();
        assert_eq!(sun11.trades, 1);
        // 2026-04-20 دوشنبه (wd=1) ساعت ۱۰
        let mon10 = cells.iter().find(|c| c.weekday == 1 && c.hour == 10).unwrap();
        assert!((mon10.total_pnl.unwrap() - (-25.0)).abs() < 1e-9);
        // مرتب‌سازی صعودی
        let mut sorted = cells.clone();
        sorted.sort_by_key(|c| (c.weekday, c.hour));
        assert_eq!(cells, sorted);
    }

    #[test]
    fn heatmap_respects_filter() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "k1", "acc1", "S", "closed", "2026-04-19T08:00:00Z", Some("2026-04-19T10:30:00Z"), Some(100.0), Some(1.0), Some("breakout"));
            seed_trade(&conn, "k2", "acc1", "S", "closed", "2026-04-19T09:00:00Z", Some("2026-04-19T11:15:00Z"), Some(50.0), Some(0.5), Some("reversal"));
        }
        let svc = QueryService::new(&db);
        let cells = svc
            .heatmap(&FilterNode::Simple(Box::new(TradeFilter {
                strategy: Some("breakout".into()),
                ..Default::default()
            })))
            .unwrap();
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].trades, 1);
    }

    #[test]
    fn aggregate_correctness_with_drawdown() {
        let db = db();
        {
            let conn = db.lock();
            // t1: +۱۰۰ (۲R)، t2: −۲۵۰ (−۲.۵R)، t3: +۵۰ (۱R)، t4: صفر، t5: باز
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", Some("2026-02-01T12:00:00Z"), Some(100.0), Some(2.0), None);
            seed_trade(&conn, "t2", "acc1", "S", "closed", "2026-02-02T10:00:00Z", Some("2026-02-02T12:00:00Z"), Some(-250.0), Some(-2.5), None);
            seed_trade(&conn, "t3", "acc1", "S", "closed", "2026-02-03T10:00:00Z", Some("2026-02-03T12:00:00Z"), Some(50.0), Some(1.0), None);
            seed_trade(&conn, "t4", "acc1", "S", "closed", "2026-02-04T10:00:00Z", Some("2026-02-04T12:00:00Z"), Some(0.0), Some(0.0), None);
            seed_trade(&conn, "t5", "acc1", "S", "open", "2026-02-05T10:00:00Z", None, None, None, None);
        }
        let svc = QueryService::new(&db);
        let stats = svc.aggregate(&FilterNode::empty()).unwrap();
        assert_eq!(stats.total_trades, 5);
        assert_eq!(stats.closed_trades, 4);
        assert_eq!(stats.wins, 2);
        assert_eq!(stats.losses, 1);
        assert_eq!(stats.breakevens, 1);
        assert!((stats.win_rate.unwrap() - 50.0).abs() < 1e-9);
        assert!((stats.avg_r.unwrap() - 0.125).abs() < 1e-9);
        assert!((stats.total_pnl.unwrap() - (-100.0)).abs() < 1e-9);
        // منحنی: +۱۰۰ → −۱۵۰ → −۱۰۰ ؛ قله ۱۰۰، کف −۱۵۰ → افت ۲۵۰
        assert!((stats.max_drawdown.unwrap() - 250.0).abs() < 1e-9);

        let curve = svc.equity_curve(&FilterNode::empty()).unwrap();
        assert_eq!(curve.len(), 4);
        assert!((curve[0].cumulative_pnl - 100.0).abs() < 1e-9);
        assert!((curve[1].cumulative_pnl - (-150.0)).abs() < 1e-9);
        assert!((curve[3].cumulative_pnl - (-100.0)).abs() < 1e-9);
    }

    #[test]
    fn aggregate_with_filter_scopes_results() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", Some("2026-02-01T12:00:00Z"), Some(100.0), Some(2.0), Some("الف"));
            seed_trade(&conn, "t2", "acc1", "S", "closed", "2026-02-02T10:00:00Z", Some("2026-02-02T12:00:00Z"), Some(-40.0), Some(-1.0), Some("ب"));
        }
        let svc = QueryService::new(&db);
        let stats = svc
            .aggregate(&FilterNode::Simple(Box::new(TradeFilter {
                strategy: Some("الف".into()),
                ..Default::default()
            })))
            .unwrap();
        assert_eq!(stats.total_trades, 1);
        assert!((stats.total_pnl.unwrap() - 100.0).abs() < 1e-9);
    }

    // ---------- عملکرد گروهی ----------

    #[test]
    fn performance_by_symbol_and_weekday() {
        let db = db();
        {
            let conn = db.lock();
            // 2026-02-02 دوشنبه (%w=1)، 2026-02-07 شنبه (%w=6)
            seed_trade(&conn, "t1", "acc1", "XAUUSD", "closed", "2026-02-02T10:00:00Z", Some("2026-02-02T12:00:00Z"), Some(100.0), Some(2.0), None);
            seed_trade(&conn, "t2", "acc1", "XAUUSD", "closed", "2026-02-07T10:00:00Z", Some("2026-02-07T12:00:00Z"), Some(-30.0), Some(-1.0), None);
            seed_trade(&conn, "t3", "acc1", "EURUSD", "closed", "2026-02-02T10:00:00Z", Some("2026-02-02T12:00:00Z"), Some(40.0), Some(1.0), None);
        }
        let svc = QueryService::new(&db);
        let groups = svc.performance_by(&Dimension::Symbol, &FilterNode::empty()).unwrap();
        assert_eq!(groups.len(), 2);
        let xau = groups.iter().find(|g| g.key == "XAUUSD").unwrap();
        assert_eq!(xau.trades, 2);
        assert_eq!(xau.wins, 1);
        assert!((xau.total_pnl.unwrap() - 70.0).abs() < 1e-9);
        assert!(xau.avg_custom_value.is_none());

        let wd = svc.performance_by(&Dimension::Weekday, &FilterNode::empty()).unwrap();
        assert_eq!(wd.len(), 2);
        let mon = wd.iter().find(|g| g.key == "1").unwrap();
        assert_eq!(mon.label, "دوشنبه");
        assert_eq!(mon.trades, 2);
        let sat = wd.iter().find(|g| g.key == "6").unwrap();
        assert_eq!(sat.label, "شنبه");
    }

    #[test]
    fn performance_by_custom_field_with_avg() {
        let db = db();
        let field_id;
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", Some("2026-02-01T12:00:00Z"), Some(100.0), Some(2.0), None);
            seed_trade(&conn, "t2", "acc1", "S", "closed", "2026-02-02T10:00:00Z", Some("2026-02-02T12:00:00Z"), Some(-40.0), Some(-1.0), None);
            seed_trade(&conn, "t3", "acc1", "S", "closed", "2026-02-03T10:00:00Z", Some("2026-02-03T12:00:00Z"), Some(60.0), Some(1.5), None);
        }
        {
            let schema = SchemaService::new(&db);
            field_id = define_field(&schema, "confidence", StorageType::Integer, false, true);
            schema.set_value("t1", &field_id, &serde_json::json!(80)).unwrap();
            schema.set_value("t2", &field_id, &serde_json::json!(20)).unwrap();
            schema.set_value("t3", &field_id, &serde_json::json!(80)).unwrap();
        }
        let svc = QueryService::new(&db);
        let groups = svc
            .performance_by(&Dimension::CustomField("confidence".into()), &FilterNode::empty())
            .unwrap();
        assert_eq!(groups.len(), 2);
        let g80 = groups.iter().find(|g| g.key == "80").unwrap();
        assert_eq!(g80.trades, 2);
        assert!((g80.avg_custom_value.unwrap() - 80.0).abs() < 1e-9);
        assert!((g80.total_pnl.unwrap() - 160.0).abs() < 1e-9);
        // فیلد بدون مجوز آمار → ۱۷۰۳
        {
            let schema = SchemaService::new(&db);
            define_field(&schema, "secret", StorageType::Integer, false, false);
        }
        let err = svc
            .performance_by(&Dimension::CustomField("secret".into()), &FilterNode::empty())
            .unwrap_err();
        assert_eq!(err.code(), 1703);
    }

    #[test]
    fn stat_fields_lists_enabled_only() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", None, Some(1.0), Some(1.0), None);
        }
        {
            let schema = SchemaService::new(&db);
            define_field(&schema, "confidence", StorageType::Integer, false, true);
            define_field(&schema, "note", StorageType::Text, false, false);
        }
        let svc = QueryService::new(&db);
        let fields = svc.stat_fields().unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].technical_key, "confidence");
        assert!(fields[0].stat_enabled);
    }

    // ---------- صفحه‌بندی و بودجه ----------

    #[test]
    fn pagination_bounds_and_pages() {
        let db = db();
        {
            let conn = db.lock();
            for i in 0..25 {
                seed_trade(&conn, &format!("t{i:03}"), "acc1", "S", "closed", &format!("2026-02-{:02}T10:00:00Z", 1 + i % 20), None, Some(1.0), Some(1.0), None);
            }
        }
        let svc = QueryService::new(&db);
        let p1 = svc.list_trades(&FilterNode::empty(), 1, 10, None, false).unwrap();
        assert_eq!(p1.total, 25);
        assert_eq!(p1.items.len(), 10);
        let p3 = svc.list_trades(&FilterNode::empty(), 3, 10, None, false).unwrap();
        assert_eq!(p3.items.len(), 5);
        // مرتب‌سازی نزولی entry_time: صفحه ۱ جدیدترین‌ها
        assert!(p1.items[0].entry_time >= p1.items[9].entry_time);
        // خطاهای بودجه
        assert_eq!(svc.list_trades(&FilterNode::empty(), 0, 10, None, false).unwrap_err().code(), 1701);
        assert_eq!(svc.list_trades(&FilterNode::empty(), 1, 0, None, false).unwrap_err().code(), 1701);
        assert_eq!(svc.list_trades(&FilterNode::empty(), 1, 201, None, false).unwrap_err().code(), 1702);
        assert!(svc.list_trades(&FilterNode::empty(), 1, 200, None, false).is_ok());
    }

    #[test]
    fn tiny_budget_interrupts_query_1702() {
        let db = db();
        {
            let conn = db.lock();
            for i in 0..2_000 {
                seed_trade(&conn, &format!("t{i:04}"), "acc1", "S", "closed", &format!("2026-03-{:02}T10:00:00Z", 1 + i % 28), None, Some(1.0), Some(1.0), None);
            }
        }
        let conn = db.lock();
        let used = Arc::new(AtomicU64::new(0));
        let counter = used.clone();
        conn.progress_handler(100, Some(move || {
            let n = counter.fetch_add(100, Ordering::Relaxed) + 100;
            n > 1_000
        }));
        let r: Result<(), rusqlite::Error> = (|| {
            let mut stmt = conn.prepare("SELECT realized_pnl FROM journal_trades")?;
            let mut rows = stmt.query([])?;
            while rows.next()?.is_some() {}
            Ok(())
        })();
        conn.progress_handler(0, None::<fn() -> bool>);
        let err = r.unwrap_err();
        assert_eq!(err.sqlite_error_code(), Some(ErrorCode::OperationInterrupted));
    }

    // ---------- پروجکشن، رویداد و داشبورد ----------

    #[test]
    fn event_stats_invalidated_triggers_invalidation() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", Some("2026-02-01T12:00:00Z"), Some(100.0), Some(2.0), None);
        }
        let svc = QueryService::new(&db);
        assert!(svc.projection_is_stale().unwrap());
        let report = svc.dashboard(None).unwrap();
        assert!(report.refreshed_now);
        assert_eq!(report.days.len(), 1);
        assert!(!svc.projection_is_stale().unwrap());
        assert_eq!(svc.projection_version().unwrap(), 0);

        // رویداد نامرتبط نادیده گرفته می‌شود
        let env = EventEnvelope::new(event_type::TRADE_CREATED, 1, aria_contracts::EventSource::DomainEngine, uuid::Uuid::new_v4(), serde_json::json!({}));
        assert!(!svc.handle_event(&env).unwrap());

        // رویداد باطل‌سازی آمار
        let env = EventEnvelope::new(event_type::STATS_INVALIDATED, 1, aria_contracts::EventSource::DomainEngine, uuid::Uuid::new_v4(), serde_json::json!({}));
        assert!(svc.handle_event(&env).unwrap());
        assert!(svc.projection_is_stale().unwrap());
        assert_eq!(svc.projection_version().unwrap(), 1);
        // داده قدیمی تا بازسازی بعدی می‌ماند
        let report = svc.dashboard(None).unwrap();
        assert!(report.refreshed_now);
        assert_eq!(report.days[0].trades_count, 1);
    }

    #[test]
    fn stats_invalidated_event_constant_matches_domain_engine() {
        assert_eq!(STATS_INVALIDATED_EVENT, event_type::STATS_INVALIDATED);
    }

    #[test]
    fn dashboard_respects_account_filter_and_reflects_new_data_after_invalidate() {
        let db = db();
        {
            let conn = db.lock();
            seed_trade(&conn, "t1", "acc1", "S", "closed", "2026-02-01T10:00:00Z", Some("2026-02-01T12:00:00Z"), Some(100.0), Some(2.0), None);
            seed_trade(&conn, "t2", "acc2", "S", "closed", "2026-02-01T10:00:00Z", Some("2026-02-01T12:00:00Z"), Some(50.0), Some(1.0), None);
        }
        let svc = QueryService::new(&db);
        let rep = svc.dashboard(Some("acc2")).unwrap();
        assert_eq!(rep.days.len(), 1);
        assert_eq!(rep.days[0].account_id, "acc2");
        assert!((rep.days[0].pnl.unwrap() - 50.0).abs() < 1e-9);

        // معامله جدید بدون باطل‌سازی → داده کهنه
        {
            let conn = db.lock();
            seed_trade(&conn, "t3", "acc2", "S", "closed", "2026-02-02T10:00:00Z", Some("2026-02-02T12:00:00Z"), Some(10.0), Some(0.5), None);
        }
        let rep = svc.dashboard(Some("acc2")).unwrap();
        assert!(!rep.refreshed_now);
        assert_eq!(rep.days.len(), 1);
        // باطل‌سازی → بازسازی → داده جدید
        svc.invalidate().unwrap();
        let rep = svc.dashboard(Some("acc2")).unwrap();
        assert!(rep.refreshed_now);
        assert_eq!(rep.days.len(), 2);
    }

    // ---------- دود عملکردی: ۱۰هزار معامله ----------

    #[test]
    fn performance_smoke_10k_trades() {
        let db = db();
        let started = std::time::Instant::now();
        {
            let conn = db.lock();
            conn.execute(
                "INSERT INTO trading_accounts (id, profile_id, name, created_at, updated_at)
                 VALUES ('acc1', 'p1', 'حساب', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .unwrap();
            for s in ["GOLD", "EUR", "BTC"] {
                conn.execute(
                    "INSERT INTO symbols (id, name, created_at) VALUES (?1, ?1, '2026-01-01T00:00:00Z')",
                    rusqlite::params![s],
                )
                .unwrap();
            }
            let tx = conn.unchecked_transaction().unwrap();
            // درج دسته‌ای با prepared statement
            let mut stmt = tx
                .prepare(
                    "INSERT INTO journal_trades
                        (id, account_id, symbol_id, direction, status, strategy, timeframe, session,
                         entry_time, exit_time, realized_pnl, realized_r, created_at, updated_at)
                     VALUES (?1, 'acc1', ?2, 'buy', 'closed', ?3, 'H1', 'لندن', ?4, ?4, ?5, ?6,
                             '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                )
                .unwrap();
            for i in 0..10_000u32 {
                let symbol = ["GOLD", "EUR", "BTC"][i as usize % 3];
                let strategy = ["برک‌اوت", "برگشت", "روند"][i as usize % 3];
                let day = 1 + (i % 365);
                let entry = format!("2025-{:02}-{:02}T10:00:00Z", 1 + day / 30 % 12, 1 + day % 28);
                // الگوی قطعی PnL برای بررسی صحت
                let pnl: f64 = match i % 4 {
                    0 => 120.0,
                    1 => -60.0,
                    2 => 80.0,
                    _ => -30.0,
                };
                let r: f64 = match i % 4 {
                    0 => 2.0,
                    1 => -1.0,
                    2 => 1.5,
                    _ => -0.5,
                };
                stmt.execute(rusqlite::params![
                    format!("perf-{i:05}"),
                    symbol,
                    strategy,
                    entry,
                    pnl,
                    r
                ])
                .unwrap();
            }
            drop(stmt);
            tx.commit().unwrap();
        }
        let insert_elapsed = started.elapsed();

        let svc = QueryService::new(&db);
        // صحت تجمیع: ۵۰٪ برد، میانگین R = (۲−۱+۱٫۵−۰٫۵)/۴ = ۰٫۵، مجموع PnL = ۲۵۰۰ × (۱۲۰−۶۰+۸۰−۳۰) = ۲۷۵٬۰۰۰
        let stats = svc.aggregate(&FilterNode::empty()).unwrap();
        assert_eq!(stats.total_trades, 10_000);
        assert_eq!(stats.closed_trades, 10_000);
        assert!((stats.win_rate.unwrap() - 50.0).abs() < 1e-9);
        assert!((stats.avg_r.unwrap() - 0.5).abs() < 1e-9);
        assert!((stats.total_pnl.unwrap() - 275_000.0).abs() < 1e-6);

        // فهرست صفحه‌بندی‌شده
        let page = svc.list_trades(&FilterNode::empty(), 7, 200, None, false).unwrap();
        assert_eq!(page.total, 10_000);
        assert_eq!(page.items.len(), 200);

        // عملکرد گروهی + فیلتر ترکیبی
        let groups = svc
            .performance_by(&Dimension::Symbol, &FilterNode::empty())
            .unwrap();
        assert_eq!(groups.len(), 3);
        let filtered = svc
            .aggregate(&FilterNode::Simple(Box::new(TradeFilter {
                strategy: Some("برک‌اوت".into()),
                result: Some(TradeResult::Win),
                ..Default::default()
            })))
            .unwrap();
        assert_eq!(filtered.total_trades, 1667); // برک‌اوت (i%3==0) و برد (pnl>0 → i%4∈{0,2}) → i%12∈{0,6} → 1667

        // داشبورد پروجکشن
        svc.invalidate().unwrap();
        let dash = svc.dashboard(None).unwrap();
        let total_rows: i64 = dash.days.iter().map(|d| d.trades_count).sum();
        assert_eq!(total_rows, 10_000);

        let total_elapsed = started.elapsed();
        tracing::info!(
            insert_ms = insert_elapsed.as_millis() as u64,
            total_ms = total_elapsed.as_millis() as u64,
            "دود عملکردی ۱۰هزار معامله"
        );
        // سقف آزاد: کل سناریو (درج + ۵ پرس‌وجو + بازسازی پروجکشن) زیر ۳۰ ثانیه
        assert!(
            total_elapsed.as_secs() < 30,
            "سناریوی عملکرد بیش از حد کند: {total_elapsed:?}"
        );
    }

    fn trade_with_note(db: &Database, note: &str, tags: Option<&str>) -> String {
        let conn = db.lock();
        conn.execute(
            "INSERT OR IGNORE INTO trading_accounts (id, profile_id, name, created_at, updated_at)
             VALUES ('a', 'p1', 'حساب', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO symbols (id, name, created_at)
             VALUES ('s', 's', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        let id = format!("t-{}", uuid::Uuid::new_v4());
        conn.execute(
            "INSERT INTO journal_trades (id, account_id, symbol_id, direction, status,
             commission, swap, note, tags, created_at, updated_at)
             VALUES (?1,'a','s','buy','open',0,0,?2,?3,datetime('now'),datetime('now'))",
            rusqlite::params![id, note, tags],
        )
        .unwrap();
        id
    }

    #[test]
    fn search_matches_note_and_tags_with_escape() {
        let db = db();
        let svc = QueryService::new(&db);
        let t1 = trade_with_note(&db, "ورود روی سطح حمایت", Some("A+, آزمایشی"));
        let t2 = trade_with_note(&db, "خروج زودهنگام", Some("برک%اوت"));
        trade_with_note(&db, "بی‌ربط", None);

        let find = |q: &str| {
            svc.list_trades(
                &FilterNode::Simple(Box::new(TradeFilter {
                    search: Some(q.into()),
                    ..Default::default()
                })),
                1,
                50,
                None,
                false,
            )
            .unwrap()
        };
        assert_eq!(find("حمایت").total, 1);
        assert_eq!(find("حمایت").items[0].id, t1);
        // جست‌وجو در برچسب‌ها هم می‌گردد
        assert_eq!(find("آزمایشی").total, 1);
        // % در ورودی کاربر تحت‌الحمایه است — با LIKE wildcards تفسیر نمی‌شود
        assert_eq!(find("برک%اوت").total, 1);
        assert_eq!(find("برک%اوت").items[0].id, t2);
        assert_eq!(find("وجودندارد").total, 0);
    }

    #[test]
    fn list_sort_whitelisted_columns_and_nulls_last() {
        let db = db();
        let svc = QueryService::new(&db);
        let a = trade_with_note(&db, "pnl 100", None);
        let b = trade_with_note(&db, "pnl -5", None);
        {
            let conn = db.lock();
            conn.execute(
                "UPDATE journal_trades SET realized_pnl = 100 WHERE id = ?1",
                rusqlite::params![a],
            )
            .unwrap();
            conn.execute(
                "UPDATE journal_trades SET realized_pnl = -5 WHERE id = ?1",
                rusqlite::params![b],
            )
            .unwrap();
        }
        // نزولی: ۱۰۰ قبل از −۵
        let desc = svc
            .list_trades(&FilterNode::empty(), 1, 50, Some("realized_pnl"), true)
            .unwrap();
        assert_eq!(desc.items[0].id, a);
        assert_eq!(desc.items[1].id, b);
        // صعودی: −۵ قبل از ۱۰۰
        let asc = svc
            .list_trades(&FilterNode::empty(), 1, 50, Some("realized_pnl"), false)
            .unwrap();
        assert_eq!(asc.items[0].id, b);
        // کلید غیرمجاز → پیش‌فرض (نه خطا)
        assert!(svc
            .list_trades(&FilterNode::empty(), 1, 50, Some("note; DROP"), false)
            .is_ok());
        // NULLها در انتهای مرتب‌سازی صعودی می‌مانند
        let c = trade_with_note(&db, "بدون pnl", None);
        let asc2 = svc
            .list_trades(&FilterNode::empty(), 1, 50, Some("realized_pnl"), false)
            .unwrap();
        assert_eq!(asc2.items.last().unwrap().id, c);
    }
}
