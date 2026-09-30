//! سرویس ایمپورت متاتریدر — کل جریان در یک تراکنش اتمیک (فاز ۱.۱۵).
//!
//! جریان: رمزگشایی → تجزیه (CSV/HTML) → نگاشت کانونی → تشخیص تکرار
//! (هش ردیف + تیکت) → گروه‌بندی بر اساس شناسه پوزیشن → ساخت
//! معامله/پا/اجرا/ردیف منبع → رویداد ابطال آمار → حسابرسی → کامیت.
//!
//! نکته: ردیف‌های ناموفق در نگاشت (مثلاً عدد نامعتبر) در «گزارش» با
//! شماره ردیف و پیام فارسی ثبت می‌شوند؛ متن خام آن‌ها در handoff نگه
//! داشته نشده و بنابراین source_record دریافت نمی‌کنند.

use crate::dedup::{is_file_duplicate, is_row_duplicate};
use crate::decode::decode;
use crate::error::ImportError;
use crate::map::{map_file, now_iso, MapOutcome};
use crate::model::{DealRole, ImportReport, MappedDeal, SourceFormat};
use crate::parse::detect_format;
use aria_storage_engine::Database;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::json;
use std::collections::HashSet;

/// دستور رویداد ابطال کش داشبورد — هم‌تراز موتور پرس‌وجو.
const STATS_INVALIDATED_EVENT: &str = "domain.stats_invalidated";

/// حداکثر حجم فایل ایمپورت — ۲۰ مگابایت (هم‌تراز `MAX_IMPORT_BYTES` فرانت).
pub const MAX_IMPORT_BYTES: usize = 20 * 1024 * 1024;

/// فراداده دسته ایمپورت — پارامترهای ثابت یک اجرای import.
struct ImportMeta<'m> {
    batch_id: &'m str,
    file_hash: &'m str,
    file_name: &'m str,
    account_id: &'m str,
    source: &'m str,
}

/// سرویس ایمپورت روی یک پایگاه‌داده باز.
pub struct ImportService<'a> {
    db: &'a Database,
}

impl<'a> ImportService<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// ایمپورت کامل فایل خروجی متاتریدر.
    ///
    /// `account_id` حساب مقصد، `source` برچسب منبع (مثل «mt5-deals»).
    /// خروجی: گزارش فارسی اعداد و هشدارها.
    pub fn import(
        &self,
        bytes: &[u8],
        file_name: &str,
        account_id: &str,
        source: &str,
    ) -> Result<ImportReport, ImportError> {
        // سقف حجم — دفاع لایه‌ای در کنار بررسی فرانت
        if bytes.len() > MAX_IMPORT_BYTES {
            return Err(ImportError::msg(format!(
                "حجم فایل ({} مگابایت) بیش از سقف مجاز ({} مگابایت) است.",
                bytes.len() / (1024 * 1024),
                MAX_IMPORT_BYTES / (1024 * 1024)
            )));
        }
        let decoded = decode(bytes)?;
        let format = detect_format(file_name, &decoded.text);
        let MapOutcome {
            deals,
            errors: map_errors,
            skipped,
            failed_rows,
        } = map_file(format, &decoded.text);
        let file_hash = blake3::hash(bytes).to_hex().to_string();
        let batch_id = uuid::Uuid::new_v4().to_string();

        let mut report = ImportReport {
            batch_id: batch_id.clone(),
            file_hash: file_hash.clone(),
            // کل ردیف‌های داده‌ای فایل = دیل‌ها + ردیف‌های غیرمعاملاتی
            total_rows: deals.len() + skipped,
            skipped,
            ..Default::default()
        };
        if !decoded.certain {
            report.warn("رمزگذاری فایل قطعی تشخیص داده نشد و با Windows-1252 رمزگشایی شد؛ اگر حروف به‌هم‌ریخته‌اند، فایل را با کدگذاری UTF-8 دوباره خروجی بگیرید.");
        }
        if format == SourceFormat::HtmlStatement && !deals.is_empty() {
            report.warn("در گزارش خلاصه MT4 قیمت بستن جداگانه موجود نیست؛ فقط اجرای ورود ثبت شد و سود/زیان از ستون Profit گرفته شد.");
        }
        report.errors = map_errors.len();
        for (row_no, msg) in map_errors.iter().take(5) {
            report.warn(format!("ردیف {row_no}: {msg}"));
        }
        if map_errors.len() > 5 {
            report.warn(format!("و {} خطای نگاشت دیگر که در جزئیات گزارش نیامده است.", map_errors.len() - 5));
        }
        if deals.is_empty() {
            if map_errors.is_empty() && skipped == 0 {
                report.warn("هیچ ردیف معاملاتی قابل ایمپورت در فایل یافت نشد.");
            } else if map_errors.is_empty() {
                report.warn(format!(
                    "در فایل فقط ردیف‌های غیرمعاملاتی (موجودی/اعتبار) بود؛ {skipped} ردیف رد شد."
                ));
            }
            // متن خام ردیف‌های ناموفق و حسابرسی تلاش — حتی بدون معامله
            let guard = self.db.lock();
            let meta = ImportMeta {
                batch_id: &batch_id,
                file_hash: &file_hash,
                file_name,
                account_id,
                source,
            };
            Self::persist_failed_rows(&guard, &meta, &failed_rows)?;
            Self::audit(&guard, &meta, &report)?;
            return Ok(report);
        }

        let mut guard = self.db.lock();
        let tx = guard
            .transaction()
            .map_err(|e| ImportError::Storage(e.to_string()))?;
        let meta = ImportMeta {
            batch_id: &batch_id,
            file_hash: &file_hash,
            file_name,
            account_id,
            source,
        };
        // متن خام ردیف‌های ناموفق پیش از پردازش حفظ می‌شود (داده اصلی تغییرناپذیر)
        Self::persist_failed_rows(&tx, &meta, &failed_rows)?;
        Self::run(&tx, &mut report, &meta, &deals)?;
        tx.commit().map_err(|e| ImportError::Storage(e.to_string()))?;
        Ok(report)
    }

    fn run(
        tx: &Transaction<'_>,
        report: &mut ImportReport,
        meta: &ImportMeta<'_>,
        deals: &[MappedDeal],
    ) -> Result<(), ImportError> {
        let now = now_iso();
        let account_ok: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM trading_accounts WHERE id = ?1)",
                [meta.account_id],
                |r| r.get(0),
            )
            .map_err(|e| ImportError::Storage(e.to_string()))?;
        if !account_ok {
            return Err(ImportError::msg(format!(
                "حساب معاملاتی «{}» یافت نشد؛ ابتدا حساب را در تنظیمات بسازید.",
                meta.account_id
            )));
        }

        // ===== تشخیص تکرار — ردیف‌های تکراری در source_records با وضعیت duplicate حفظ می‌شوند
        let mut new_deals: Vec<&MappedDeal> = Vec::new();
        for deal in deals {
            if is_row_duplicate(tx, deal)? {
                report.duplicates += 1;
                tx.execute(
                    "INSERT INTO source_records (id, import_batch_id, raw_payload, payload_hash, source, imported_at, processed_status)
                     VALUES (?1,?2,?3,?4,?5,?6,'duplicate')",
                    params![uuid::Uuid::new_v4().to_string(), meta.batch_id, deal.raw, deal.payload_hash, meta.source, now],
                )
                .map_err(|e| ImportError::Storage(e.to_string()))?;
            } else {
                new_deals.push(deal);
            }
        }

        // ===== گروه‌بندی بر اساس شناسه پوزیشن — حفظ ترتیب اولین ظهور
        let mut groups: Vec<Vec<&MappedDeal>> = Vec::new();
        let mut index_of: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for deal in &new_deals {
            let key = deal
                .position_id
                .clone()
                .unwrap_or_else(|| format!("__row_{}", deal.row_index));
            match index_of.get(&key) {
                Some(&gi) => groups[gi].push(deal),
                None => {
                    index_of.insert(key, groups.len());
                    groups.push(vec![deal]);
                }
            }
        }

        // ===== ساخت معامله برای هر گروه
        let mut warned_symbols: HashSet<String> = HashSet::new();
        for group in &groups {
            let symbol_id =
                Self::resolve_symbol(tx, &group[0].symbol, report, &mut warned_symbols, &now)?;
            let entry = group.iter().find(|d| d.role == DealRole::In).unwrap_or(&group[0]);
            let direction = entry.direction.as_str();
            let closed = group
                .iter()
                .any(|d| d.role == DealRole::Out || d.close_time.is_some());
            let entry_time = entry.time.clone();
            let exit_time = if closed {
                group
                    .iter()
                    .filter_map(|d| d.close_time.clone().or_else(|| {
                        (d.role == DealRole::Out).then(|| d.time.clone())
                    }))
                    .max()
            } else {
                None
            };
            let needs_assignment = group.iter().any(|d| d.role == DealRole::Unknown);
            let risk_status = if needs_assignment { "needs_assignment" } else { "no_stop_loss" };
            let realized_pnl: f64 = group.iter().map(|d| d.profit).sum();
            let commission: f64 = group.iter().map(|d| d.commission).sum();
            let swap: f64 = group.iter().map(|d| d.swap).sum();
            let trade_id = uuid::Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO journal_trades (id, account_id, symbol_id, direction, status, entry_time, exit_time,
                    risk_calculation_status, realized_pnl, commission, swap, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12)",
                params![
                    trade_id,
                    meta.account_id,
                    symbol_id,
                    direction,
                    if closed { "closed" } else { "open" },
                    entry_time,
                    exit_time,
                    risk_status,
                    realized_pnl,
                    commission,
                    swap,
                    now
                ],
            )
            .map_err(|e| ImportError::Storage(e.to_string()))?;
            report.trades_created += 1;

            for deal in group {
                // ردیف منبع — داده خام تغییرناپذیر
                let source_record_id = uuid::Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO source_records (id, import_batch_id, raw_payload, payload_hash, source, imported_at, processed_status)
                     VALUES (?1,?2,?3,?4,?5,?6,'processed')",
                    params![source_record_id, meta.batch_id, deal.raw, deal.payload_hash, meta.source, now],
                )
                .map_err(|e| ImportError::Storage(e.to_string()))?;

                // پا — فقط برای دیل‌هایی که نقش ورود/خروج مشخص دارند
                let (leg_id, leg_kind) = match deal.role {
                    DealRole::In => {
                        let leg_id = uuid::Uuid::new_v4().to_string();
                        tx.execute(
                            "INSERT INTO entry_legs (id, trade_id, executed_price, volume, entry_time, created_at, updated_at)
                             VALUES (?1,?2,?3,?4,?5,?6,?6)",
                            params![leg_id, trade_id, deal.price, deal.volume, deal.time, now],
                        )
                        .map_err(|e| ImportError::Storage(e.to_string()))?;
                        (Some(leg_id), Some("entry"))
                    }
                    DealRole::Out => {
                        let leg_id = uuid::Uuid::new_v4().to_string();
                        tx.execute(
                            "INSERT INTO exit_legs (id, trade_id, exit_reason, executed_price, volume, exit_time, created_at, updated_at)
                             VALUES (?1,?2,?3,?4,?5,?6,?7,?7)",
                            params![leg_id, trade_id, deal.comment, deal.price, deal.volume, deal.time, now],
                        )
                        .map_err(|e| ImportError::Storage(e.to_string()))?;
                        (Some(leg_id), Some("exit"))
                    }
                    DealRole::Unknown => (None, None),
                };

                let assignment_status = if leg_id.is_some() { "assigned" } else { "needs_assignment" };
                tx.execute(
                    "INSERT INTO executions (id, trade_id, leg_id, leg_kind, source_record_id, kind, direction, price, volume,
                        executed_at, commission, swap, ticket, magic, comment, assignment_status, created_at)
                     VALUES (?1,?2,?3,?4,?5,'imported',?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
                    params![
                        uuid::Uuid::new_v4().to_string(),
                        trade_id,
                        leg_id,
                        leg_kind,
                        source_record_id,
                        deal.direction.as_str(),
                        deal.price,
                        deal.volume,
                        deal.time,
                        deal.commission,
                        deal.swap,
                        deal.ticket,
                        deal.magic,
                        deal.comment,
                        assignment_status,
                        now
                    ],
                )
                .map_err(|e| ImportError::Storage(e.to_string()))?;
                report.imported += 1;
                if assignment_status == "needs_assignment" {
                    report.needs_assignment += 1;
                }
            }
        }

        // ردیف‌های معاملاتی = درج‌شده + تکراری (ردیف‌های غیرمعاملاتی جدا شمرده می‌شوند)
        let deal_rows = report.imported + report.duplicates;
        if is_file_duplicate(deal_rows, report.imported, report.duplicates, report.errors) {
            report.file_duplicate = true;
            report.warn("این فایل قبلاً به‌طور کامل ایمپورت شده است؛ هیچ معامله جدیدی ساخته نشد.");
        }

        // ===== رویداد ابطال کش داشبورد — فقط وقتی داده موثر تغییر کرده
        if report.imported > 0 {
            tx.execute(
                "INSERT INTO system_events (id, event_type, event_version, source, correlation_id, payload, published, created_at)
                 VALUES (?1,?2,1,'kernel:import_engine',?3,?4,0,?5)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    STATS_INVALIDATED_EVENT,
                    meta.batch_id,
                    json!({ "trade_id": null, "scope": "trade" }).to_string(),
                    now
                ],
            )
            .map_err(|e| ImportError::Storage(e.to_string()))?;
        }

        // ===== حسابرسی — همیشه حتی برای فایل کاملاً تکراری
        Self::audit(tx, meta, report)?;
        Ok(())
    }

    /// ثبت متن خام ردیف‌های ناموفق نگاشت — داده اصلی هرگز دور ریخته نمی‌شود.
    fn persist_failed_rows(
        conn: &Connection,
        meta: &ImportMeta<'_>,
        failed_rows: &[(usize, String)],
    ) -> Result<(), ImportError> {
        for (row_no, raw) in failed_rows {
            conn.execute(
                "INSERT INTO source_records (id, import_batch_id, raw_payload, payload_hash, source, imported_at, processed_status)
                 VALUES (?1,?2,?3,?4,?5,?6,'failed')",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    meta.batch_id,
                    raw,
                    format!("{}#row{row_no}", crate::parse::row_hash(raw)),
                    meta.source,
                    now_iso()
                ],
            )
            .map_err(|e| ImportError::Storage(e.to_string()))?;
        }
        Ok(())
    }

    /// ثبت رکورد حسابرسی دسته ایمپورت — روی تراکنش یا اتصال.
    fn audit(conn: &Connection, meta: &ImportMeta<'_>, report: &ImportReport) -> Result<(), ImportError> {
        conn.execute(
            "INSERT INTO audit_logs (id, action, actor, target, detail, created_at)
             VALUES (?1,'import.mt','kernel:import_engine',?2,?3,?4)",
            params![
                uuid::Uuid::new_v4().to_string(),
                meta.file_name,
                json!({
                    "batch_id": meta.batch_id,
                    "file_hash": meta.file_hash,
                    "source": meta.source,
                    "total_rows": report.total_rows,
                    "skipped": report.skipped,
                    "imported": report.imported,
                    "duplicates": report.duplicates,
                    "errors": report.errors,
                    "needs_assignment": report.needs_assignment,
                    "trades_created": report.trades_created,
                    "file_duplicate": report.file_duplicate,
                })
                .to_string(),
                now_iso()
            ],
        )
        .map_err(|e| ImportError::Storage(e.to_string()))?;
        Ok(())
    }

    /// یافتن شناسه نماد؛ اگر نبود، خودکار ساخته می‌شود (هشدار یک‌بار برای هر نماد).
    fn resolve_symbol(
        tx: &Transaction<'_>,
        name: &str,
        report: &mut ImportReport,
        warned: &mut HashSet<String>,
        now: &str,
    ) -> Result<String, ImportError> {
        let existing: Option<String> = tx
            .query_row("SELECT id FROM symbols WHERE name = ?1", [name], |r| r.get(0))
            .optional()
            .map_err(|e| ImportError::Storage(e.to_string()))?;
        if let Some(id) = existing {
            return Ok(id);
        }
        let id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO symbols (id, name, description, contract_size, created_at)
             VALUES (?1,?2,?3,1,?4)",
            params![id, name, "ساخته‌شده خودکار از ایمپورت متاتریدر", now],
        )
        .map_err(|e| ImportError::Storage(e.to_string()))?;
        if warned.insert(name.to_string()) {
            report.warn(format!("نماد «{name}» در فهرست نمادها نبود و به‌صورت خودکار ساخته شد؛ اندازه قرارداد پیش‌فرض ۱ است و می‌توانید آن را اصلاح کنید."));
        }
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SourceFormat;

    fn db() -> Database {
        let db = Database::open_memory(Some("test-pass-1")).unwrap();
        {
            let mut conn = db.lock();
            aria_storage_engine::migrations::run_migrations(&mut conn, |_| Ok(())).unwrap();
        }
        db
    }

    fn seed_account(db: &Database) -> String {
        let conn = db.lock();
        let now = "2026-09-28T12:00:00Z";
        conn.execute(
            "INSERT INTO profiles (id, name, created_at, updated_at) VALUES ('p1','پروفایل',?1,?1)",
            params![now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO trading_accounts (id, profile_id, name, currency, created_at, updated_at)
             VALUES ('a1','p1','حساب متاتریدر','USD',?1,?1)",
            params![now],
        )
        .unwrap();
        "a1".into()
    }

    const MT5_CSV: &str = "Time,Position,Type,Direction,Volume,Price,Order,Commission,Swap,Profit,Symbol,Comment\n\
        2024.01.15 10:30:00,123456,buy,in,0.10,2035.50,789,0.00,0.00,0.00,XAUUSD,\n\
        2024.01.15 12:00:00,123456,sell,out,0.10,2040.00,790,-0.50,0.20,45.00,XAUUSD,take profit\n";

    #[test]
    fn imports_mt5_csv_end_to_end() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);

        let report = svc
            .import(MT5_CSV.as_bytes(), "deals.csv", "a1", "mt5-deals")
            .unwrap();
        assert_eq!(report.total_rows, 2);
        assert_eq!(report.imported, 2);
        assert_eq!(report.duplicates, 0);
        assert_eq!(report.errors, 0);
        assert_eq!(report.trades_created, 1);
        assert_eq!(report.needs_assignment, 0);
        assert!(!report.file_duplicate);

        let conn = database.lock();
        // معامله بسته‌شده با مبالغ تجمیعی
        let (status, pnl, comm, swap, entry, exit, risk): (
            String, f64, f64, f64, String, String, String,
        ) = conn
            .query_row(
                "SELECT status, realized_pnl, commission, swap, entry_time, exit_time, risk_calculation_status
                 FROM journal_trades",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )
            .unwrap();
        assert_eq!(status, "closed");
        assert!((pnl - 45.0).abs() < 1e-9);
        assert!((comm + 0.5).abs() < 1e-9);
        assert!((swap - 0.2).abs() < 1e-9);
        assert_eq!(entry, "2024-01-15T10:30:00Z");
        assert_eq!(exit, "2024-01-15T12:00:00Z");
        assert_eq!(risk, "no_stop_loss");
        // پاها و اجراها
        assert_eq!(
            conn.query_row("SELECT count(*) FROM entry_legs", [], |r| r.get::<_, i64>(0)).unwrap(),
            1
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM exit_legs", [], |r| r.get::<_, i64>(0)).unwrap(),
            1
        );
        let (n_assigned, n_imported): (i64, i64) = conn
            .query_row(
                "SELECT sum(assignment_status='assigned'), sum(kind='imported') FROM executions",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((n_assigned, n_imported), (2, 2));
        // هر اجرا به ردیف منبع و پا متصل است
        let orphan: i64 = conn
            .query_row(
                "SELECT count(*) FROM executions WHERE source_record_id IS NULL OR leg_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphan, 0);
        // ردیف‌های منبع پردازش‌شده
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM source_records WHERE processed_status='processed'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        // حسابرسی و رویداد ابطال آمار
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM audit_logs WHERE action='import.mt'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM system_events WHERE event_type='domain.stats_invalidated'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn reimport_same_file_is_fully_duplicate() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        svc.import(MT5_CSV.as_bytes(), "deals.csv", "a1", "mt5-deals").unwrap();

        let report = svc
            .import(MT5_CSV.as_bytes(), "deals.csv", "a1", "mt5-deals")
            .unwrap();
        assert!(report.file_duplicate);
        assert_eq!(report.imported, 0);
        assert_eq!(report.duplicates, 2);
        assert_eq!(report.trades_created, 0);
        assert!(report.warnings.iter().any(|w| w.contains("قبلاً")));

        let conn = database.lock();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM journal_trades", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1,
            "no second trade is created"
        );
        // ردیف‌های تکراری حفظ شده‌اند (ممیزی)
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM source_records WHERE processed_status='duplicate'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn modified_file_with_same_tickets_is_duplicate() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        svc.import(MT5_CSV.as_bytes(), "deals.csv", "a1", "mt5-deals").unwrap();

        // همان تیکت‌ها با محتوای متفاوت (هش ردیف عوض می‌شود)
        let modified = MT5_CSV.replace("2035.50", "2035.51");
        assert_ne!(modified, MT5_CSV);
        let report = svc
            .import(modified.as_bytes(), "deals2.csv", "a1", "mt5-deals")
            .unwrap();
        assert!(report.file_duplicate, "ticket check catches cross-file duplicates");
        assert_eq!(report.duplicates, 2);
        assert_eq!(report.trades_created, 0);
    }

    #[test]
    fn missing_direction_creates_needs_assignment() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        let csv = "Time,Position,Type,Volume,Price,Order,Commission,Swap,Profit,Symbol,Comment\n\
            2024.01.15 10:30:00,555,buy,0.10,2035.50,999,0.00,0.00,0.00,XAUUSD,\n";
        let report = svc.import(csv.as_bytes(), "deals.csv", "a1", "mt5-deals").unwrap();
        assert_eq!(report.imported, 1);
        assert_eq!(report.needs_assignment, 1);

        let conn = database.lock();
        let (risk, status): (String, String) = conn
            .query_row(
                "SELECT risk_calculation_status, assignment_status FROM journal_trades
                 JOIN executions ON executions.trade_id = journal_trades.id",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(risk, "needs_assignment");
        assert_eq!(status, "needs_assignment");
    }

    #[test]
    fn creates_missing_symbol_with_warning() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        let report = svc
            .import(MT5_CSV.as_bytes(), "deals.csv", "a1", "mt5-deals")
            .unwrap();
        assert!(report.warnings.iter().any(|w| w.contains("نماد")));
        let conn = database.lock();
        let desc: String = conn
            .query_row("SELECT description FROM symbols WHERE name='XAUUSD'", [], |r| r.get(0))
            .unwrap();
        assert!(desc.contains("ایمپورت"));
    }

    #[test]
    fn rejects_unknown_account() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        let err = svc
            .import(MT5_CSV.as_bytes(), "deals.csv", "missing", "mt5-deals")
            .unwrap_err();
        assert_eq!(err.code(), 1500);
        assert!(err.to_string().contains("حساب"));
    }

    #[test]
    fn rejects_files_over_the_size_cap() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        let huge = vec![b'a'; MAX_IMPORT_BYTES + 1];
        let err = svc.import(&huge, "big.csv", "a1", "mt5-deals").unwrap_err();
        assert_eq!(err.code(), 1500);
        assert!(err.to_string().contains("حجم فایل"));
    }

    #[test]
    fn counts_skipped_non_trade_rows() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        // قالب CSV صورت‌حساب MT4 با یک ردیف موجودی
        let csv = "Ticket,Open Time,Type,Size,Item,Price,Profit\n\
            1,2024.01.15 10:30,buy,0.10,XAUUSD,2035.50,45.00\n\
            2,2024.01.16 00:00,balance,,,0,1000.00\n";
        let report = svc.import(csv.as_bytes(), "statement.csv", "a1", "mt4-statement").unwrap();
        assert_eq!(report.skipped, 1);
        assert_eq!(report.total_rows, 2, "skipped row counts toward total");
        assert_eq!(report.imported, 1);
        assert_eq!(report.trades_created, 1);
        assert_eq!(report.errors, 0);
    }

    #[test]
    fn skipped_only_file_creates_no_trade_and_is_audited() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        let csv = "Ticket,Open Time,Type,Size,Item,Price,Profit\n\
            2,2024.01.16 00:00,balance,,,0,1000.00\n";
        let report = svc.import(csv.as_bytes(), "balance.csv", "a1", "mt4-statement").unwrap();
        assert_eq!(report.imported, 0);
        assert_eq!(report.trades_created, 0);
        assert_eq!(report.skipped, 1);
        assert!(report.warnings.iter().any(|w| w.contains("غیرمعاملاتی")));
        let conn = database.lock();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM journal_trades", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM audit_logs WHERE action='import.mt'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1,
            "failed attempt is still audited"
        );
    }

    #[test]
    fn reports_missing_symbol_once_per_symbol() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        // دو معامله روی یک نماد جدید — هشدار نماد باید یک‌بار باشد
        let csv = "Time,Position,Type,Direction,Volume,Price,Order,Commission,Swap,Profit,Symbol,Comment\n\
            2024.01.15 10:30:00,111,buy,in,0.10,2035.50,1,0,0,0,EURUSD,\n\
            2024.01.15 12:00:00,111,sell,out,0.10,2040.00,2,0,0,30,EURUSD,\n\
            2024.01.16 10:30:00,222,buy,in,0.10,2035.50,3,0,0,0,EURUSD,\n\
            2024.01.16 12:00:00,222,sell,out,0.10,2040.00,4,0,0,30,EURUSD,\n";
        let report = svc.import(csv.as_bytes(), "deals.csv", "a1", "mt5-deals").unwrap();
        assert_eq!(report.trades_created, 2);
        let symbol_warnings = report
            .warnings
            .iter()
            .filter(|w| w.contains("نماد «EURUSD»"))
            .count();
        assert_eq!(symbol_warnings, 1, "one warning per missing symbol");
    }

    #[test]
    fn imports_mt4_html_statement() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        let html = r#"<html><body><table>
            <tr><td>Ticket</td><td>Open Time</td><td>Type</td><td>Size</td><td>Item</td><td>Price</td><td>Close Time</td><td>Commission</td><td>Taxes</td><td>Swap</td><td>Profit</td></tr>
            <tr><td>771001</td><td>2024.01.15 10:30</td><td>buy</td><td>0.10</td><td>xauusd</td><td>2035.50</td><td>2024.01.15 12:00</td><td>-0.50</td><td>0.00</td><td>0.20</td><td>45.00</td></tr>
            <tr><td></td><td>2024.01.16 00:00</td><td>balance</td><td></td><td></td><td>0</td><td></td><td>0.00</td><td>0.00</td><td>0.00</td><td>1000.00</td></tr>
        </table></body></html>"#;
        let report = svc
            .import(html.as_bytes(), "statement.html", "a1", "mt4-statement")
            .unwrap();
        assert_eq!(report.total_rows, 2, "one deal plus one skipped balance row");
        assert_eq!(report.skipped, 1, "balance rows are not deals");
        assert_eq!(report.imported, 1);
        assert_eq!(report.trades_created, 1);
        assert_eq!(report.needs_assignment, 0);
        assert!(report.warnings.iter().any(|w| w.contains("MT4")));

        let conn = database.lock();
        let (status, exit, pnl): (String, String, f64) = conn
            .query_row(
                "SELECT status, exit_time, realized_pnl FROM journal_trades",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, "closed");
        assert_eq!(exit, "2024-01-15T12:00:00Z");
        assert!((pnl - 45.0).abs() < 1e-9);
        // اجرا به پای ورود متصل و assigned است
        let (leg_kind, assignment): (String, String) = conn
            .query_row(
                "SELECT leg_kind, assignment_status FROM executions",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((leg_kind.as_str(), assignment.as_str()), ("entry", "assigned"));
        assert_eq!(
            conn.query_row("SELECT count(*) FROM exit_legs", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0,
            "MT4 summary has no separate close execution"
        );
    }

    #[test]
    fn failed_rows_are_preserved_as_source_records() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        let csv = "Time,Position,Type,Direction,Volume,Price,Order,Commission,Swap,Profit,Symbol,Comment\n\
            2024.01.15 10:30:00,1,buy,in,0.10,2035.50,1,0,0,0,XAUUSD,\n\
            2024.01.15 11:00:00,2,buy,in,abc,2035.50,2,0,0,0,XAUUSD,\n";
        let report = svc.import(csv.as_bytes(), "deals.csv", "a1", "mt5-deals").unwrap();
        assert_eq!(report.imported, 1);
        assert_eq!(report.errors, 1);

        let conn = database.lock();
        let raw: String = conn
            .query_row(
                "SELECT raw_payload FROM source_records WHERE processed_status='failed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(raw.contains("abc"), "raw text of the failing row is kept");
        // ردیف‌های ناموفق هرگز تکرار تشخیص داده نمی‌شوند (هش با اندیس ردیف یکتاست)
        let dup_rows: i64 = conn
            .query_row(
                "SELECT count(*) FROM source_records WHERE processed_status='duplicate'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dup_rows, 0);
    }

    #[test]
    fn all_error_rows_are_reported_without_trades() {
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        let csv = "Time,Position,Type,Direction,Volume,Price,Order,Commission,Swap,Profit,Symbol,Comment\n\
            2024.01.15 10:30:00,1,buy,in,abc,2035.50,1,0,0,0,XAUUSD,\n\
            2024.01.15 11:00:00,2,sell,out,xyz,2040.00,2,0,0,0,XAUUSD,\n";
        let report = svc.import(csv.as_bytes(), "deals.csv", "a1", "mt5-deals").unwrap();
        assert_eq!(report.errors, 2);
        assert_eq!(report.imported, 0);
        assert!(!report.file_duplicate, "errored file is not a clean duplicate");
        let conn = database.lock();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM journal_trades", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn detects_csv_format_for_tsv_content() {
        // رمزگشایی UTF-16LE با BOM + جدول‌بند تِب — قالب رایج خروجی MT5
        let database = db();
        seed_account(&database);
        let svc = ImportService::new(&database);
        let tsv = "Time\tPosition\tType\tDirection\tVolume\tPrice\tOrder\tCommission\tSwap\tProfit\tSymbol\tComment\n\
            2024.01.15 10:30:00\t123\tbuy\tin\t0.10\t2035.50\t10\t0\t0\t0\tXAUUSD\t\n\
            2024.01.15 12:00:00\t123\tsell\tout\t0.10\t2040.00\t11\t0\t0\t30\tXAUUSD\t\n";
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(tsv.encode_utf16().flat_map(u16::to_le_bytes));
        let report = svc.import(&bytes, "deals.tsv", "a1", "mt5-deals").unwrap();
        assert_eq!(report.imported, 2);
        assert_eq!(report.trades_created, 1);
        assert_eq!(
            SourceFormat::CsvDeals,
            detect_format("deals.tsv", tsv),
            "tsv is csv-family"
        );
    }
}
