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
///
/// نکته serde: واریانت‌ها ساختاری‌اند (`{"kind":"integer","value":70}`) تا با
/// برچسب داخلی تجزیه‌شدنی باشند (تاپلی روی عدد/رشته با تگ داخلی کار نمی‌کند).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CustomValue {
    Text { value: String },
    Integer { value: i64 },
    Decimal { value: f64 },
    Boolean { value: bool },
}

impl CustomValue {
    /// معادل SQL برای لاگ و سریال‌سازی — تبدیل واقعی در شرط فیلتر انجام می‌شود.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::Integer { .. } => "integer",
            Self::Decimal { .. } => "decimal",
            Self::Boolean { .. } => "boolean",
        }
    }

    /// سازنده‌های کوتاه برای استفاده در کد و تست‌ها.
    pub fn text(v: impl Into<String>) -> Self {
        Self::Text { value: v.into() }
    }
    pub fn integer(v: i64) -> Self {
        Self::Integer { value: v }
    }
    pub fn decimal(v: f64) -> Self {
        Self::Decimal { value: v }
    }
    pub fn boolean(v: bool) -> Self {
        Self::Boolean { value: v }
    }
}

/// عملگرهای فیلد سفارشی طبق قرارداد.
///
/// نکته serde: واریانت‌ها ساختاری‌اند (`{"op":"min","value":50.0}`) تا شکل
/// JSON پل فرانت‌اند مستقیم تجزیه شود (همان محدودیت تگ داخلی FilterNode).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CustomFieldOp {
    Equals { value: CustomValue },
    NotEquals { value: CustomValue },
    Contains { value: String },
    Min { value: f64 },
    Max { value: f64 },
    In { values: Vec<CustomValue> },
    NotIn { values: Vec<CustomValue> },
    Exists,
}

/// فیلتر روی یک فیلد سفارشی با کلید فنی.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomFieldFilter {
    pub field_key: String,
    pub op: CustomFieldOp,
}

/// حداکثر عمق مجاز درخت فیلتر — هم‌تراز با نگهبان فرانت‌اند (`extensions.ts`).
pub const MAX_FILTER_DEPTH: usize = 8;

/// گره درخت فیلتر — ترکیب AND/OR با تودرتویی محدود.
///
/// نکته serde: گروه‌ها واریانت **ساختاری** هستند (`{"type":"all","children":[…]}`)
/// تا شکل JSON پل فرانت‌اند (`kernel.ts`) مستقیم تجزیه شود؛ واریانت تاپلی
/// روی Vec با برچسب داخلی تجزیه‌شدنی نیست.
///
/// نکته امنیتی: تجزیه از مسیر `try_from` می‌گذرد تا عمق تودرتویی پیش از
/// ساخت درخت بررسی شود و ورودی خرابکارانه (هزاران لایه) باعث سرریز پشته
/// در خودِ serde نشود.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", try_from = "FilterNodeInput")]
pub enum FilterNode {
    Simple(Box<TradeFilter>),
    Custom(CustomFieldFilter),
    All {
        children: Vec<FilterNode>,
    },
    Any {
        children: Vec<FilterNode>,
    },
}

impl FilterNode {
    /// فیلتر خالی = بدون قید.
    pub fn empty() -> Self {
        Self::All { children: vec![] }
    }
}

/// شکل میانی برای تجزیه با بررسی عمق — آینه ساختار `FilterNode`.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum FilterNodeInput {
    Simple(Box<TradeFilter>),
    Custom(CustomFieldFilter),
    All {
        children: Vec<FilterNodeInput>,
    },
    Any {
        children: Vec<FilterNodeInput>,
    },
}

impl TryFrom<FilterNodeInput> for FilterNode {
    type Error = QueryError;

    fn try_from(input: FilterNodeInput) -> Result<Self, QueryError> {
        convert_node(input, 1)
    }
}

fn convert_node(input: FilterNodeInput, depth: usize) -> Result<FilterNode, QueryError> {
    if depth > MAX_FILTER_DEPTH {
        return Err(QueryError::invalid_query(format!(
            "عمق فیلتر از حد مجاز {MAX_FILTER_DEPTH} فراتر رفت"
        )));
    }
    Ok(match input {
        FilterNodeInput::Simple(f) => FilterNode::Simple(f),
        FilterNodeInput::Custom(c) => FilterNode::Custom(c),
        FilterNodeInput::All { children } => FilterNode::All {
            children: children
                .into_iter()
                .map(|c| convert_node(c, depth + 1))
                .collect::<Result<Vec<_>, _>>()?,
        },
        FilterNodeInput::Any { children } => FilterNode::Any {
            children: children
                .into_iter()
                .map(|c| convert_node(c, depth + 1))
                .collect::<Result<Vec<_>, _>>()?,
        },
    })
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
    // فقط YYYY-MM-DD دقیق (ده نویسه، الگوی عددی، ماه ۰۱-۱۲ و روز ۰۱-۳۱)
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    if !b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) {
        return false;
    }
    let (mm, dd) = (&s[5..7], &s[8..10]);
    (1..=12).contains(&mm.parse::<u8>().unwrap_or(0))
        && (1..=31).contains(&dd.parse::<u8>().unwrap_or(0))
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
            (CustomValue::Text { value }, "text_value" | "datetime_value") => {
                Ok(SqlValue::Text(value.clone()))
            }
            (CustomValue::Integer { value }, "integer_value" | "boolean_value") => {
                Ok(SqlValue::Integer(*value))
            }
            (CustomValue::Boolean { value }, "integer_value" | "boolean_value") => {
                Ok(SqlValue::Integer(*value as i64))
            }
            (CustomValue::Decimal { value }, "decimal_value") => Ok(SqlValue::Real(*value)),
            (CustomValue::Integer { value }, "decimal_value") => Ok(SqlValue::Real(*value as f64)),
            _ => Err(QueryError::invalid_query(
                "نوع مقدار با نوع ذخیره‌سازی فیلد سفارشی سازگار نیست",
            )),
        }
    };

    // ساخت نهایی شرط + پارامترها
    match op {
        CustomFieldOp::Equals { value } => {
            if json_col {
                return Err(QueryError::invalid_query(
                    "برای فیلدهای چندمقداری از contains استفاده کنید",
                ));
            }
            let p = expect_cmp(value)?;
            Ok((format!("{column} = ?"), vec![p]))
        }
        CustomFieldOp::NotEquals { value } => {
            if json_col {
                return Err(QueryError::invalid_query(
                    "برای فیلدهای چندمقداری از contains استفاده کنید",
                ));
            }
            let p = expect_cmp(value)?;
            Ok((format!("({column} IS NOT NULL AND {column} <> ?)"), vec![p]))
        }
        CustomFieldOp::Contains { value } => {
            if numeric_col || column == "boolean_value" {
                return Err(QueryError::invalid_query(
                    "عملگر contains فقط برای فیلدهای متنی/چندمقداری مجاز است",
                ));
            }
            let escaped = like_escape(value);
            if json_col {
                Ok((
                    format!(r#"{column} LIKE ? ESCAPE '\'"#),
                    vec![SqlValue::Text(format!("%\"{escaped}\"%"))],
                ))
            } else {
                Ok((
                    format!(r#"{column} LIKE ? ESCAPE '\'"#),
                    vec![SqlValue::Text(format!("%{escaped}%"))],
                ))
            }
        }
        CustomFieldOp::Min { value } => {
            if !numeric_col {
                return Err(QueryError::invalid_query(
                    "عملگر min فقط برای فیلدهای عددی مجاز است",
                ));
            }
            Ok((format!("{column} >= ?"), vec![SqlValue::Real(*value)]))
        }
        CustomFieldOp::Max { value } => {
            if !numeric_col {
                return Err(QueryError::invalid_query(
                    "عملگر max فقط برای فیلدهای عددی مجاز است",
                ));
            }
            Ok((format!("{column} <= ?"), vec![SqlValue::Real(*value)]))
        }
        CustomFieldOp::In { values } => {
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
        CustomFieldOp::NotIn { values } => {
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
            FilterNode::All { children } => {
                for c in children {
                    build(conn, c, out)?;
                }
                Ok(())
            }
            FilterNode::Any { children } => {
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
        assert!(!is_date_only("2026-99-99")); // الگوی عددی الزامی
        assert!(!is_date_only("2026-1-31"));
        assert!(!is_date_only("2026/01/31"));
    }

    // ---------- رفت‌وبرگشت serde — قرارداد پل فرانت‌اند ----------

    #[test]
    fn serde_roundtrip_all_group_matches_frontend_shape() {
        // دقیقاً شکلی که kernel.ts می‌فرستد
        let json = r#"{"type":"all","children":[]}"#;
        let node: FilterNode = serde_json::from_str(json).unwrap();
        assert!(matches!(node, FilterNode::All { children } if children.is_empty()));
        assert_eq!(serde_json::to_string(&FilterNode::empty()).unwrap(), json);
    }

    #[test]
    fn serde_roundtrip_nested_groups_and_simple() {
        let json = serde_json::json!({
            "type": "any",
            "children": [
                {"type": "all", "children": [
                    {"type": "simple", "strategy": "برک‌اوت", "result": "win"}
                ]},
                {"type": "simple", "symbol_id": "XAUUSD"}
            ]
        });
        let node: FilterNode = serde_json::from_value(json).unwrap();
        let back = serde_json::to_value(&node).unwrap();
        let parsed: FilterNode = serde_json::from_value(back).unwrap();
        // رفت‌وبرگشت کامل بدون خطا
        assert!(matches!(parsed, FilterNode::Any { children } if children.len() == 2));
    }

    #[test]
    fn serde_roundtrip_custom_filter_with_op_tag() {
        let json = serde_json::json!({
            "type": "custom",
            "field_key": "confidence",
            "op": {"op": "min", "value": 50.0}
        });
        let parsed: FilterNode = serde_json::from_value(json).unwrap();
        assert!(matches!(
            parsed,
            FilterNode::Custom(CustomFieldFilter { op: CustomFieldOp::Min { value: 50.0 }, .. })
        ));
    }

    #[test]
    fn filter_depth_at_limit_accepted() {
        // ۸ سطح تودرتویی (حد مجاز) باید پذیرفته شود
        let mut node = serde_json::json!({"type": "all", "children": []});
        for _ in 1..MAX_FILTER_DEPTH {
            node = serde_json::json!({"type": "all", "children": [node]});
        }
        let parsed: FilterNode = serde_json::from_value(node).unwrap();
        assert!(matches!(parsed, FilterNode::All { .. }));
    }

    #[test]
    fn filter_depth_beyond_limit_rejected_1701() {
        // ۹ سطح تودرتویی باید با کد ۱۷۰۱ رد شود — نه سرریز پشته
        let mut node = serde_json::json!({"type": "all", "children": []});
        for _ in 0..MAX_FILTER_DEPTH {
            node = serde_json::json!({"type": "all", "children": [node]});
        }
        let err = serde_json::from_value::<FilterNode>(node).unwrap_err();
        assert!(err.to_string().contains("عمق فیلتر"));
    }
}
