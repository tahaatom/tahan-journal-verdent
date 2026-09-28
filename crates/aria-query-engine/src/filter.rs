//! فیلترهای پرس‌وجو — ساده، فیلد سفارشی و گروه‌های AND/OR.
//!
//! همه شرط‌ها به‌صورت پارامتری ساخته می‌شوند؛ هیچ رشته‌ای از ورودی کاربر
//! مستقیماً در SQL قرار نمی‌گیرد (ترکیب رشته‌ای فقط برای عملگرهای ثابت و
//! نام ستون‌های سفید-لیست `StorageType::value_column`).

use crate::error::QueryError;
use aria_schema_engine::model::StorageType;
use rusqlite::types::Value as SqlValue;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// نتیجه معامله بر مبنای `realized_pnl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeResult {
    Win,
    Loss,
    Breakeven,
    Open,
}

impl TradeResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Win => "win",
            Self::Loss => "loss",
            Self::Breakeven => "breakeven",
            Self::Open => "open",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "win" => Self::Win,
            "loss" => Self::Loss,
            "breakeven" => Self::Breakeven,
            "open" => Self::Open,
            _ => return None,
        })
    }
}

/// فیلترهای ساده روی ستون‌های `journal_trades`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TradeFilter {
    pub account_id: Option<String>,
    pub symbol_id: Option<String>,
    pub direction: Option<String>,
    pub status: Option<String>,
    pub strategy: Option<String>,
    pub timeframe: Option<String>,
    pub session: Option<String>,
    /// برچسب‌ها — تطبیق «شامل»
    pub tags: Option<String>,
    /// احساسات — تطبیق «شامل»
    pub emotions: Option<String>,
    /// از تاریخ/زمان ورود (ISO؛ رشته ۱۰حرفی = فقط تاریخ)
    pub entry_from: Option<String>,
    /// تا تاریخ/زمان ورود (شامل؛ رشته ۱۰حرفی = فقط تاریخ)
    pub entry_to: Option<String>,
    pub result: Option<TradeResult>,
}

/// مقدار تایپ‌دار فیلد سفارشی برای مقایسه.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CustomValue {
    Text(String),
    Integer(i64),
    Decimal(f64),
    Boolean(bool),
}

impl CustomValue {
    /// معادل SQL برای لاگ و سریال‌سازی — تبدیل واقعی در شرط فیلتر انجام می‌شود.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Integer(_) => "integer",
            Self::Decimal(_) => "decimal",
            Self::Boolean(_) => "boolean",
        }
    }
}

/// عملگرهای فیلد سفارشی طبق قرارداد.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CustomFieldOp {
    Equals(CustomValue),
    NotEquals(CustomValue),
    Contains(String),
    Min(f64),
    Max(f64),
    In(Vec<CustomValue>),
    NotIn(Vec<CustomValue>),
    Exists,
}

/// فیلتر روی یک فیلد سفارشی با کلید فنی.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomFieldFilter {
    pub field_key: String,
    pub op: CustomFieldOp,
}

/// گره درخت فیلتر — ترکیب AND/OR با تودرتویی دلخواه.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FilterNode {
    Simple(Box<TradeFilter>),
    Custom(CustomFieldFilter),
    All(Vec<FilterNode>),
    Any(Vec<FilterNode>),
}

impl FilterNode {
    /// فیلتر خالی = بدون قید.
    pub fn empty() -> Self {
        Self::All(vec![])
    }
}

/// فراداده یک فیلد سفارشی که از `custom_fields` خوانده می‌شود.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldMeta {
    pub storage_type: StorageType,
    pub filterable: bool,
    pub stat_enabled: bool,
    pub analysis_enabled: bool,
    pub active: bool,
}

/// خروجی ساخت شرط SQL.
#[derive(Debug, Default)]
pub(crate) struct SqlFilter {
    pub where_clause: String,
    pub params: Vec<SqlValue>,
}

impl SqlFilter {
    fn push_and(&mut self, cond: &str, params: &[SqlValue]) {
        if self.where_clause.is_empty() {
            self.where_clause.push_str(" WHERE ");
        } else {
            self.where_clause.push_str(" AND ");
        }
        self.where_clause.push_str(cond);
        self.params.extend_from_slice(params);
    }
}

/// رشته ورودی LIKE را برای ESCAPE '\' امن می‌کند.
pub(crate) fn like_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn is_date_only(s: &str) -> bool {
    s.len() == 10 && s.as_bytes().get(4) == Some(&b'-')
}

/// شرط محدوده زمان ورود؛ مقایسه تاریخ‌محور برای رشته‌های ۱۰حرفی
/// تا مستقل از آفست منطقه‌زمانی مقادیر ذخیره‌شده درست باشد.
fn entry_range_conditions(f: &TradeFilter) -> Vec<(String, Vec<SqlValue>)> {
    let mut out = Vec::new();
    if let Some(from) = &f.entry_from {
        if is_date_only(from) {
            out.push(("substr(entry_time, 1, 10) >= ?".into(), vec![SqlValue::Text(from.clone())]));
        } else {
            out.push(("entry_time >= ?".into(), vec![SqlValue::Text(from.clone())]));
        }
    }
    if let Some(to) = &f.entry_to {
        if is_date_only(to) {
            out.push(("substr(entry_time, 1, 10) <= ?".into(), vec![SqlValue::Text(to.clone())]));
        } else {
            out.push(("entry_time <= ?".into(), vec![SqlValue::Text(to.clone())]));
        }
    }
    out
}

fn result_condition(r: TradeResult) -> (&'static str, Vec<SqlValue>) {
    match r {
        TradeResult::Win => ("(realized_pnl IS NOT NULL AND realized_pnl > 0)", vec![]),
        TradeResult::Loss => ("(realized_pnl IS NOT NULL AND realized_pnl < 0)", vec![]),
        TradeResult::Breakeven => ("realized_pnl = 0", vec![]),
        TradeResult::Open => ("realized_pnl IS NULL", vec![]),
    }
}

/// شرط مقایسه روی ستون مقدار فیلد سفارشی — با اعتبارسنجی نوع.
/// خروجی: (شرط، پارامترها).
fn custom_value_condition(
    op: &CustomFieldOp,
    column: &'static str,
) -> Result<(String, Vec<SqlValue>), QueryError> {
    let json_col = column == "json_value";
    let numeric_col = matches!(column, "integer_value" | "decimal_value");

    // مقایسه برابری: متن، عدد صحیح، اعشاری و بولی بر اساس ستون مقصد
    let expect_cmp = |v: &CustomValue| -> Result<SqlValue, QueryError> {
        match (v, column) {
            (CustomValue::Text(s), "text_value" | "datetime_value") => Ok(SqlValue::Text(s.clone())),
            (CustomValue::Integer(i), "integer_value" | "boolean_value") => {
                Ok(SqlValue::Integer(*i))
            }
            (CustomValue::Boolean(b), "integer_value" | "boolean_value") => {
                Ok(SqlValue::Integer(*b as i64))
            }
            (CustomValue::Decimal(d), "decimal_value") => Ok(SqlValue::Real(*d)),
            (CustomValue::Integer(i), "decimal_value") => Ok(SqlValue::Real(*i as f64)),
            _ => Err(QueryError::invalid_query(
                "نوع مقدار با نوع ذخیره‌سازی فیلد سفارشی سازگار نیست",
            )),
        }
    };

    // ساخت نهایی شرط + پارامترها
    match op {
        CustomFieldOp::Equals(v) => {
            if json_col {
                return Err(QueryError::invalid_query(
                    "برای فیلدهای چندمقداری از contains استفاده کنید",
                ));
            }
            let p = expect_cmp(v)?;
            Ok((format!("{column} = ?"), vec![p]))
        }
        CustomFieldOp::NotEquals(v) => {
            if json_col {
                return Err(QueryError::invalid_query(
                    "برای فیلدهای چندمقداری از contains استفاده کنید",
                ));
            }
            let p = expect_cmp(v)?;
            Ok((format!("({column} IS NOT NULL AND {column} <> ?)"), vec![p]))
        }
        CustomFieldOp::Contains(v) => {
            if numeric_col || column == "boolean_value" {
                return Err(QueryError::invalid_query(
                    "عملگر contains فقط برای فیلدهای متنی/چندمقداری مجاز است",
                ));
            }
            let escaped = like_escape(v);
            if json_col {
                Ok((format!("{column} LIKE ?"), vec![SqlValue::Text(format!("%\"{escaped}\"%"))]))
            } else {
                Ok((
                    format!(r#"{column} LIKE ? ESCAPE '\'"#),
                    vec![SqlValue::Text(format!("%{escaped}%"))],
                ))
            }
        }
        CustomFieldOp::Min(v) => {
            if !numeric_col {
                return Err(QueryError::invalid_query(
                    "عملگر min فقط برای فیلدهای عددی مجاز است",
                ));
            }
            Ok((format!("{column} >= ?"), vec![SqlValue::Real(*v)]))
        }
        CustomFieldOp::Max(v) => {
            if !numeric_col {
                return Err(QueryError::invalid_query(
                    "عملگر max فقط برای فیلدهای عددی مجاز است",
                ));
            }
            Ok((format!("{column} <= ?"), vec![SqlValue::Real(*v)]))
        }
        CustomFieldOp::In(values) => {
            if json_col {
                return Err(QueryError::invalid_query(
                    "برای فیلدهای چندمقداری از contains استفاده کنید",
                ));
            }
            let mut ps = Vec::with_capacity(values.len());
            for v in values {
                ps.push(expect_cmp(v)?);
            }
            if ps.is_empty() {
                Ok(("0".into(), vec![]))
            } else {
                let marks = vec!["?"; ps.len()].join(",");
                Ok((format!("{column} IN ({marks})"), ps))
            }
        }
        CustomFieldOp::NotIn(values) => {
            if json_col {
                return Err(QueryError::invalid_query(
                    "برای فیلدهای چندمقداری از contains استفاده کنید",
                ));
            }
            let mut ps = Vec::with_capacity(values.len());
            for v in values {
                ps.push(expect_cmp(v)?);
            }
            if ps.is_empty() {
                Ok((format!("{column} IS NOT NULL"), vec![]))
            } else {
                let marks = vec!["?"; ps.len()].join(",");
                Ok((format!("({column} IS NOT NULL AND {column} NOT IN ({marks}))"), ps))
            }
        }
        CustomFieldOp::Exists => Ok((format!("{column} IS NOT NULL"), vec![])),
    }
}

/// فراداده فیلد سفارشی را با کلید فنی از `custom_fields` می‌خواند.
pub(crate) fn lookup_field(conn: &Connection, key: &str) -> Result<Option<FieldMeta>, QueryError> {
    let mut stmt = conn.prepare_cached(
        "SELECT storage_type, filterable, stat_enabled, analysis_enabled, active
         FROM custom_fields WHERE technical_key = ?1",
    )?;
    let mut rows = stmt.query(rusqlite::params![key])?;
    match rows.next()? {
        Some(row) => {
            let storage_raw: String = row.get(0)?;
            let storage = StorageType::parse(&storage_raw).ok_or_else(|| {
                QueryError::Storage(rusqlite::Error::InvalidColumnType(
                    0,
                    "storage_type".into(),
                    rusqlite::types::Type::Text,
                ))
            })?;
            Ok(Some(FieldMeta {
                storage_type: storage,
                filterable: row.get::<_, i64>(1)? != 0,
                stat_enabled: row.get::<_, i64>(2)? != 0,
                analysis_enabled: row.get::<_, i64>(3)? != 0,
                active: row.get::<_, i64>(4)? != 0,
            }))
        }
        None => Ok(None),
    }
}

/// شرط EXISTS برای یک فیلتر فیلد سفارشی.
fn custom_field_condition(
    conn: &Connection,
    f: &CustomFieldFilter,
) -> Result<(String, Vec<SqlValue>), QueryError> {
    let meta = lookup_field(conn, &f.field_key)?
        .ok_or_else(|| QueryError::field_not_queryable(format!("فیلد «{}» یافت نشد", f.field_key)))?;
    if !meta.active {
        return Err(QueryError::field_not_queryable(format!(
            "فیلد «{}» غیرفعال است",
            f.field_key
        )));
    }
    if !meta.filterable {
        return Err(QueryError::field_not_queryable(format!(
            "فیلد «{}» برای فیلتر کردن فعال نشده است",
            f.field_key
        )));
    }
    let column = meta.storage_type.value_column();
    let (value_cond, mut value_params) = custom_value_condition(&f.op, column)?;
    let mut params = vec![SqlValue::Text(f.field_key.clone())];
    params.append(&mut value_params);
    Ok((
        format!(
            "EXISTS (SELECT 1 FROM field_values fv JOIN custom_fields cf ON cf.id = fv.field_id \
             WHERE fv.trade_id = journal_trades.id AND cf.technical_key = ?1 AND {value_cond})"
        ),
        params,
    ))
}

/// شرط‌های فیلتر ساده را به SQL اضافه می‌کند.
fn apply_simple(f: &TradeFilter, out: &mut SqlFilter) {
    if let Some(v) = &f.account_id {
        out.push_and("account_id = ?", &[SqlValue::Text(v.clone())]);
    }
    if let Some(v) = &f.symbol_id {
        out.push_and("symbol_id = ?", &[SqlValue::Text(v.clone())]);
    }
    if let Some(v) = &f.direction {
        out.push_and("direction = ?", &[SqlValue::Text(v.clone())]);
    }
    if let Some(v) = &f.status {
        out.push_and("status = ?", &[SqlValue::Text(v.clone())]);
    }
    if let Some(v) = &f.strategy {
        out.push_and("strategy = ?", &[SqlValue::Text(v.clone())]);
    }
    if let Some(v) = &f.timeframe {
        out.push_and("timeframe = ?", &[SqlValue::Text(v.clone())]);
    }
    if let Some(v) = &f.session {
        out.push_and("session = ?", &[SqlValue::Text(v.clone())]);
    }
    if let Some(v) = &f.tags {
        let escaped = like_escape(v);
        out.push_and(
            r#"tags LIKE ? ESCAPE '\'"#,
            &[SqlValue::Text(format!("%{escaped}%"))],
        );
    }
    if let Some(v) = &f.emotions {
        let escaped = like_escape(v);
        out.push_and(
            r#"emotions LIKE ? ESCAPE '\'"#,
            &[SqlValue::Text(format!("%{escaped}%"))],
        );
    }
    for (cond, params) in entry_range_conditions(f) {
        out.push_and(&cond, &params);
    }
    if let Some(r) = f.result {
        let (cond, params) = result_condition(r);
        out.push_and(cond, &params);
    }
}

/// ساخت شرط کامل از درخت فیلتر — بازگشتی روی گروه‌های AND/OR.
pub(crate) fn build_filter(
    conn: &Connection,
    node: &FilterNode,
) -> Result<SqlFilter, QueryError> {
    fn build(
        conn: &Connection,
        node: &FilterNode,
        out: &mut SqlFilter,
    ) -> Result<(), QueryError> {
        match node {
            FilterNode::Simple(f) => {
                apply_simple(f, out);
                Ok(())
            }
            FilterNode::Custom(f) => {
                let (cond, params) = custom_field_condition(conn, f)?;
                out.push_and(&cond, &params);
                Ok(())
            }
            FilterNode::All(children) => {
                for c in children {
                    build(conn, c, out)?;
                }
                Ok(())
            }
            FilterNode::Any(children) => {
                if children.is_empty() {
                    return Err(QueryError::invalid_query(
                        "گروه OR نمی‌تواند خالی باشد",
                    ));
                }
                let mut parts = Vec::with_capacity(children.len());
                let mut params = Vec::new();
                for c in children {
                    let mut sub = SqlFilter::default();
                    build(conn, c, &mut sub)?;
                    if sub.where_clause.is_empty() {
                        parts.push("1".into());
                    } else {
                        parts.push(format!("({})", trim_where(&sub.where_clause)));
                        params.append(&mut sub.params);
                    }
                }
                out.push_and(&format!("({})", parts.join(" OR ")), &params);
                Ok(())
            }
        }
    }
    let mut out = SqlFilter::default();
    build(conn, node, &mut out)?;
    Ok(out)
}

/// حذف پیشوند " WHERE " از شرط زیرگروه.
fn trim_where(clause: &str) -> &str {
    clause.strip_prefix(" WHERE ").unwrap_or(clause)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_escape_escapes_wildcards() {
        assert_eq!(like_escape(r"50\%"), r"50\\\%");
        assert_eq!(like_escape("a_b"), "a\\_b");
        assert_eq!(like_escape("abc"), "abc");
    }

    #[test]
    fn trade_result_roundtrip() {
        for r in [
            TradeResult::Win,
            TradeResult::Loss,
            TradeResult::Breakeven,
            TradeResult::Open,
        ] {
            assert_eq!(TradeResult::parse(r.as_str()), Some(r));
        }
        assert_eq!(TradeResult::parse("x"), None);
    }

    #[test]
    fn date_only_detection() {
        assert!(is_date_only("2026-01-31"));
        assert!(!is_date_only("2026-01-31T10:00:00Z"));
    }
}
