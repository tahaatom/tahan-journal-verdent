//! اعتبارسنجی مقادیر فیلدهای سفارشی — مبنای فرم، فیلتر و آمار.

use crate::error::SchemaError;
use crate::model::{FieldDefinition, FieldOption, StorageType};
use serde::{Deserialize, Serialize};

/// مقدار نوع‌دار برای ستون مقصد در field_values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypedValue {
    Text(String),
    Integer(i64),
    Decimal(f64),
    Boolean(bool),
    Datetime(String),
    Json(serde_json::Value),
}

impl TypedValue {
    /// ستون مقصد.
    pub fn column(&self) -> &'static str {
        match self {
            Self::Text(_) => "text_value",
            Self::Integer(_) => "integer_value",
            Self::Decimal(_) => "decimal_value",
            Self::Boolean(_) => "boolean_value",
            Self::Datetime(_) => "datetime_value",
            Self::Json(_) => "json_value",
        }
    }
}

/// حداقل/حداکثر مجاز امتیاز (rating).
pub const RATING_MIN: i64 = 1;
pub const RATING_MAX_DEFAULT: i64 = 5;

/// اعتبارسنجی و تبدیل مقدار خام (JSON) به مقدار نوع‌دار.
///
/// `options`: گزینه‌های فعال فیلد (فقط برای انواع select).
pub fn validate_value(
    field: &FieldDefinition,
    active_options: &[FieldOption],
    raw: &serde_json::Value,
) -> Result<TypedValue, SchemaError> {
    let field_key = field.technical_key.as_str();
    match field.storage_type {
        StorageType::Text | StorageType::LongText | StorageType::Enum => {
            let s = raw
                .as_str()
                .ok_or_else(|| SchemaError::invalid_value(format!("{field_key}: expected string")))?;
            if field.storage_type == StorageType::Enum {
                let known = active_options.iter().any(|o| o.active && o.value == s);
                if !known {
                    return Err(SchemaError::unknown_option(field_key, s));
                }
            }
            check_text_rules(field, s)?;
            Ok(TypedValue::Text(s.to_string()))
        }
        StorageType::MultiEnum | StorageType::TagSet => {
            let arr = raw
                .as_array()
                .ok_or_else(|| SchemaError::invalid_value(format!("{field_key}: expected array")))?;
            let mut seen = std::collections::HashSet::new();
            for item in arr {
                let s = item
                    .as_str()
                    .ok_or_else(|| SchemaError::invalid_value(format!("{field_key}: array items must be strings")))?;
                let known = active_options.iter().any(|o| o.active && o.value == s);
                if !known {
                    return Err(SchemaError::unknown_option(field_key, s));
                }
                if !seen.insert(s.to_string()) {
                    return Err(SchemaError::invalid_value(format!("{field_key}: duplicate value {s}")));
                }
            }
            Ok(TypedValue::Json(raw.clone()))
        }
        StorageType::Integer => {
            let n = raw
                .as_i64()
                .ok_or_else(|| SchemaError::invalid_value(format!("{field_key}: expected integer")))?;
            check_numeric_rules(field, n as f64)?;
            Ok(TypedValue::Integer(n))
        }
        StorageType::Rating => {
            let n = raw
                .as_i64()
                .ok_or_else(|| SchemaError::invalid_value(format!("{field_key}: rating must be integer")))?;
            if !(RATING_MIN..=RATING_MAX_DEFAULT).contains(&n) {
                return Err(SchemaError::invalid_value(format!(
                    "{field_key}: rating must be in {RATING_MIN}..={RATING_MAX_DEFAULT}"
                )));
            }
            Ok(TypedValue::Integer(n))
        }
        StorageType::Decimal => {
            let n = raw
                .as_f64()
                .ok_or_else(|| SchemaError::invalid_value(format!("{field_key}: expected number")))?;
            if !n.is_finite() {
                return Err(SchemaError::invalid_value(format!("{field_key}: number must be finite")));
            }
            check_numeric_rules(field, n)?;
            Ok(TypedValue::Decimal(n))
        }
        StorageType::Boolean => {
            let b = raw
                .as_bool()
                .ok_or_else(|| SchemaError::invalid_value(format!("{field_key}: expected boolean")))?;
            Ok(TypedValue::Boolean(b))
        }
        StorageType::Datetime => {
            let s = raw
                .as_str()
                .ok_or_else(|| SchemaError::invalid_value(format!("{field_key}: expected ISO datetime string")))?;
            chrono::DateTime::parse_from_rfc3339(s)
                .map_err(|_| SchemaError::invalid_value(format!("{field_key}: invalid ISO 8601 datetime")))?;
            Ok(TypedValue::Datetime(s.to_string()))
        }
    }
}

fn check_text_rules(field: &FieldDefinition, s: &str) -> Result<(), SchemaError> {
    let rules = &field.validation_rules;
    let len = s.chars().count() as u32;
    if let Some(min) = rules.min_length {
        if len < min {
            return Err(SchemaError::invalid_value(format!(
                "{}: length {len} < min_length {min}",
                field.technical_key
            )));
        }
    }
    if let Some(max) = rules.max_length {
        if len > max {
            return Err(SchemaError::invalid_value(format!(
                "{}: length {len} > max_length {max}",
                field.technical_key
            )));
        }
    }
    Ok(())
}

fn check_numeric_rules(field: &FieldDefinition, n: f64) -> Result<(), SchemaError> {
    if let Some(min) = field.validation_rules.min {
        if n < min {
            return Err(SchemaError::invalid_value(format!(
                "{}: {n} < min {min}",
                field.technical_key
            )));
        }
    }
    if let Some(max) = field.validation_rules.max {
        if n > max {
            return Err(SchemaError::invalid_value(format!(
                "{}: {n} > max {max}",
                field.technical_key
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FieldDefinition, SemanticType, StorageType};

    fn option(field_id: &str, value: &str) -> FieldOption {
        FieldOption {
            id: uuid::Uuid::new_v4().to_string(),
            field_id: field_id.to_string(),
            value: value.to_string(),
            label: value.to_string(),
            sort_order: 0,
            active: true,
        }
    }

    fn field(t: StorageType, s: SemanticType) -> FieldDefinition {
        FieldDefinition::new("test_field", "تست", t, s)
    }

    #[test]
    fn text_roundtrip_and_rules() {
        let mut f = field(StorageType::Text, SemanticType::ShortText);
        f.validation_rules.min_length = Some(2);
        f.validation_rules.max_length = Some(4);
        assert!(validate_value(&f, &[], &serde_json::json!("ab")).is_ok());
        assert!(validate_value(&f, &[], &serde_json::json!("abcd")).is_ok());
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!("a")),
            Err(SchemaError::InvalidValue { .. })
        ));
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!("abcde")),
            Err(SchemaError::InvalidValue { .. })
        ));
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!(5)),
            Err(SchemaError::InvalidValue { .. })
        ));
    }

    #[test]
    fn enum_requires_known_active_option() {
        let f = field(StorageType::Enum, SemanticType::SingleSelect);
        let opts = vec![option(&f.id, "good"), option(&f.id, "bad")];
        assert!(validate_value(&f, &opts, &serde_json::json!("good")).is_ok());
        assert!(matches!(
            validate_value(&f, &opts, &serde_json::json!("ugly")),
            Err(SchemaError::UnknownOption { .. })
        ));
        // گزینه غیرفعال دیگر معتبر نیست
        let mut opts2 = opts.clone();
        opts2[1].active = false;
        assert!(matches!(
            validate_value(&f, &opts2, &serde_json::json!("bad")),
            Err(SchemaError::UnknownOption { .. })
        ));
    }

    #[test]
    fn multi_enum_rejects_unknown_and_duplicates() {
        let f = field(StorageType::MultiEnum, SemanticType::MultiSelect);
        let opts = vec![option(&f.id, "a"), option(&f.id, "b")];
        assert!(validate_value(&f, &opts, &serde_json::json!(["a", "b"])).is_ok());
        assert!(matches!(
            validate_value(&f, &opts, &serde_json::json!(["a", "x"])),
            Err(SchemaError::UnknownOption { .. })
        ));
        assert!(matches!(
            validate_value(&f, &opts, &serde_json::json!(["a", "a"])),
            Err(SchemaError::InvalidValue { .. })
        ));
        assert!(matches!(
            validate_value(&f, &opts, &serde_json::json!("a")),
            Err(SchemaError::InvalidValue { .. })
        ));
    }

    #[test]
    fn integer_with_bounds() {
        let mut f = field(StorageType::Integer, SemanticType::Number);
        f.validation_rules.min = Some(0.0);
        f.validation_rules.max = Some(100.0);
        assert!(validate_value(&f, &[], &serde_json::json!(50)).is_ok());
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!(-1)),
            Err(SchemaError::InvalidValue { .. })
        ));
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!(101)),
            Err(SchemaError::InvalidValue { .. })
        ));
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!(1.5)),
            Err(SchemaError::InvalidValue { .. })
        ));
    }

    #[test]
    fn decimal_bounds_and_finite() {
        let mut f = field(StorageType::Decimal, SemanticType::Percent);
        f.validation_rules.max = Some(100.0);
        assert!(validate_value(&f, &[], &serde_json::json!(99.5)).is_ok());
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!(100.5)),
            Err(SchemaError::InvalidValue { .. })
        ));
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!("x")),
            Err(SchemaError::InvalidValue { .. })
        ));
    }

    #[test]
    fn rating_range() {
        let f = field(StorageType::Rating, SemanticType::Rating);
        assert!(validate_value(&f, &[], &serde_json::json!(1)).is_ok());
        assert!(validate_value(&f, &[], &serde_json::json!(5)).is_ok());
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!(0)),
            Err(SchemaError::InvalidValue { .. })
        ));
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!(6)),
            Err(SchemaError::InvalidValue { .. })
        ));
    }

    #[test]
    fn boolean_strict() {
        let f = field(StorageType::Boolean, SemanticType::Boolean);
        assert!(validate_value(&f, &[], &serde_json::json!(true)).is_ok());
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!(1)),
            Err(SchemaError::InvalidValue { .. })
        ));
    }

    #[test]
    fn datetime_iso_required() {
        let f = field(StorageType::Datetime, SemanticType::Datetime);
        assert!(validate_value(&f, &[], &serde_json::json!("2026-09-28T10:00:00Z")).is_ok());
        assert!(matches!(
            validate_value(&f, &[], &serde_json::json!("1405/07/06")),
            Err(SchemaError::InvalidValue { .. })
        ));
    }

    #[test]
    fn typed_value_columns_match_storage() {
        assert_eq!(TypedValue::Text("x".into()).column(), "text_value");
        assert_eq!(TypedValue::Integer(1).column(), "integer_value");
        assert_eq!(TypedValue::Decimal(1.5).column(), "decimal_value");
        assert_eq!(TypedValue::Boolean(true).column(), "boolean_value");
        assert_eq!(TypedValue::Datetime("t".into()).column(), "datetime_value");
        assert_eq!(TypedValue::Json(serde_json::json!([])).column(), "json_value");
    }
}
