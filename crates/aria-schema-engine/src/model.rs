//! مدل فیلد سفارشی — تعریف اعلانی بدون کد.

use serde::{Deserialize, Serialize};

/// نوع ذخیره‌سازی فیلد (ستون مقصد در field_values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageType {
    Text,
    LongText,
    Integer,
    Decimal,
    Boolean,
    Datetime,
    Enum,
    MultiEnum,
    Rating,
    TagSet,
}

impl StorageType {
    /// برچسب ستون مقصد در جدول field_values.
    pub fn value_column(&self) -> &'static str {
        match self {
            Self::Text | Self::LongText | Self::Enum => "text_value",
            Self::MultiEnum | Self::TagSet => "json_value",
            Self::Integer | Self::Rating => "integer_value",
            Self::Decimal => "decimal_value",
            Self::Boolean => "boolean_value",
            Self::Datetime => "datetime_value",
        }
    }

    /// آیا فیلد به گزینه‌های enum نیاز دارد.
    pub fn needs_options(&self) -> bool {
        matches!(self, Self::Enum | Self::MultiEnum | Self::TagSet)
    }

    /// نمای رشته‌ای در پایگاه‌داده (بدون گیومه JSON).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::LongText => "long_text",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::Boolean => "boolean",
            Self::Datetime => "datetime",
            Self::Enum => "enum",
            Self::MultiEnum => "multi_enum",
            Self::Rating => "rating",
            Self::TagSet => "tag_set",
        }
    }

    /// خواندن از نمای رشته‌ای پایگاه‌داده.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "text" => Self::Text,
            "long_text" => Self::LongText,
            "integer" => Self::Integer,
            "decimal" => Self::Decimal,
            "boolean" => Self::Boolean,
            "datetime" => Self::Datetime,
            "enum" => Self::Enum,
            "multi_enum" => Self::MultiEnum,
            "rating" => Self::Rating,
            "tag_set" => Self::TagSet,
            _ => return None,
        })
    }
}

/// نوع معنایی فیلد (کنترل ویجت فرم و رفتار تحلیل).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticType {
    ShortText,
    LongText,
    Number,
    Price,
    Percent,
    Money,
    Datetime,
    Boolean,
    SingleSelect,
    MultiSelect,
    Rating,
    Tag,
}

impl SemanticType {
    /// نمای رشته‌ای در پایگاه‌داده (بدون گیومه JSON).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ShortText => "short_text",
            Self::LongText => "long_text",
            Self::Number => "number",
            Self::Price => "price",
            Self::Percent => "percent",
            Self::Money => "money",
            Self::Datetime => "datetime",
            Self::Boolean => "boolean",
            Self::SingleSelect => "single_select",
            Self::MultiSelect => "multi_select",
            Self::Rating => "rating",
            Self::Tag => "tag",
        }
    }

    /// خواندن از نمای رشته‌ای پایگاه‌داده.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "short_text" => Self::ShortText,
            "long_text" => Self::LongText,
            "number" => Self::Number,
            "price" => Self::Price,
            "percent" => Self::Percent,
            "money" => Self::Money,
            "datetime" => Self::Datetime,
            "boolean" => Self::Boolean,
            "single_select" => Self::SingleSelect,
            "multi_select" => Self::MultiSelect,
            "rating" => Self::Rating,
            "tag" => Self::Tag,
            _ => return None,
        })
    }
}

/// قواعد اعتبارسنجی اعلانی (بدون کد).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case", deny_unknown_fields)]
pub struct ValidationRules {
    /// کمینه مقدار عددی
    pub min: Option<f64>,
    /// بیشینه مقدار عددی
    pub max: Option<f64>,
    /// کمینه طول متن
    pub min_length: Option<u32>,
    /// بیشینه طول متن
    pub max_length: Option<u32>,
}

impl ValidationRules {
    /// آیا هیچ قاعده‌ای تنظیم نشده است.
    pub fn is_empty(&self) -> bool {
        self.min.is_none() && self.max.is_none() && self.min_length.is_none() && self.max_length.is_none()
    }
}

/// تعریف کامل یک فیلد سفارشی.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FieldDefinition {
    /// شناسه یکتا (uuid)
    pub id: String,
    /// کلید فنی یکتا — الگو: snake_case
    pub technical_key: String,
    /// برچسب نمایشی
    pub display_label: String,
    /// توضیح
    pub description: Option<String>,
    /// نوع ذخیره‌سازی
    pub storage_type: StorageType,
    /// نوع معنایی
    pub semantic_type: SemanticType,
    /// واحد (مثل «پیپ»، «دلار»، «٪»)
    pub unit: Option<String>,
    /// مقدار پیش‌فرض (JSON)
    pub default_value: Option<serde_json::Value>,
    /// اجباری بودن در فرم معامله
    pub required: bool,
    /// فعال بودن
    pub active: bool,
    /// قابل فیلتر (فیلترها و لیست‌ها)
    pub filterable: bool,
    /// فعال در آمار
    pub stat_enabled: bool,
    /// فعال در تحلیل پیشرفته
    pub analysis_enabled: bool,
    /// ترتیب نمایش در فرم
    pub display_order: i64,
    /// گروه فرم
    pub form_group: Option<String>,
    /// قواعد اعتبارسنجی
    pub validation_rules: ValidationRules,
    /// نسخه تعریف فیلد
    pub schema_version: u32,
}

impl FieldDefinition {
    /// تعریف جدید با شناسه و مقادیر پیش‌فرض.
    pub fn new(technical_key: &str, display_label: &str, storage_type: StorageType, semantic_type: SemanticType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            technical_key: technical_key.to_string(),
            display_label: display_label.to_string(),
            description: None,
            storage_type,
            semantic_type,
            unit: None,
            default_value: None,
            required: false,
            active: true,
            filterable: false,
            stat_enabled: false,
            analysis_enabled: false,
            display_order: 0,
            form_group: None,
            validation_rules: ValidationRules::default(),
            schema_version: 1,
        }
    }
}

/// گزینه فیلدهای enum/multi_enum/tag_set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FieldOption {
    pub id: String,
    pub field_id: String,
    /// مقدار ماشین‌خوان
    pub value: String,
    /// برچسب نمایشی
    pub label: String,
    pub sort_order: i64,
    pub active: bool,
}

/// اعتبارسنجی ساختاری تعریف فیلد.
pub fn validate_definition(def: &FieldDefinition) -> Result<(), crate::error::SchemaError> {
    use crate::error::SchemaError;
    if def.technical_key.is_empty() {
        return Err(SchemaError::invalid_definition("technical_key is empty"));
    }
    let ok = def.technical_key.chars().enumerate().all(|(i, c)| {
        c.is_ascii_lowercase() || c == '_' || (i > 0 && c.is_ascii_digit())
    });
    if !ok || def.technical_key.ends_with('_') {
        return Err(SchemaError::invalid_definition(format!(
            "technical_key must be snake_case: {}",
            def.technical_key
        )));
    }
    if def.display_label.trim().is_empty() {
        return Err(SchemaError::invalid_definition("display_label is empty"));
    }
    if def.schema_version == 0 {
        return Err(SchemaError::invalid_definition("schema_version must be >= 1"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_column_mapping_is_complete() {
        let all = [
            (StorageType::Text, "text_value"),
            (StorageType::LongText, "text_value"),
            (StorageType::Enum, "text_value"),
            (StorageType::MultiEnum, "json_value"),
            (StorageType::TagSet, "json_value"),
            (StorageType::Integer, "integer_value"),
            (StorageType::Rating, "integer_value"),
            (StorageType::Decimal, "decimal_value"),
            (StorageType::Boolean, "boolean_value"),
            (StorageType::Datetime, "datetime_value"),
        ];
        for (t, col) in all {
            assert_eq!(t.value_column(), col);
        }
    }

    #[test]
    fn options_required_for_select_types() {
        assert!(StorageType::Enum.needs_options());
        assert!(StorageType::MultiEnum.needs_options());
        assert!(StorageType::TagSet.needs_options());
        assert!(!StorageType::Text.needs_options());
        assert!(!StorageType::Integer.needs_options());
    }

    #[test]
    fn definition_serde_roundtrip() {
        let mut def = FieldDefinition::new("setup_quality", "کیفیت ستاپ", StorageType::Enum, SemanticType::SingleSelect);
        def.required = true;
        def.filterable = true;
        def.stat_enabled = true;
        def.form_group = Some("تحلیل".to_string());
        def.validation_rules.min_length = Some(1);
        let json = serde_json::to_string(&def).unwrap();
        let back: FieldDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(def, back);
    }

    #[test]
    fn technical_key_validation() {
        let mut d = FieldDefinition::new("ok_key_1", "x", StorageType::Text, SemanticType::ShortText);
        assert!(validate_definition(&d).is_ok());

        d.technical_key = "Bad-Key".to_string();
        assert!(matches!(validate_definition(&d), Err(crate::error::SchemaError::InvalidDefinition { .. })));

        d.technical_key = "1start_digit".to_string();
        assert!(validate_definition(&d).is_err());

        d.technical_key = "trailing_".to_string();
        assert!(validate_definition(&d).is_err());

        d.technical_key = "".to_string();
        assert!(validate_definition(&d).is_err());

        d.technical_key = "good".to_string();
        d.display_label = " ".to_string();
        assert!(validate_definition(&d).is_err());
    }

    #[test]
    fn validation_rules_empty_detection() {
        assert!(ValidationRules::default().is_empty());
        let r = ValidationRules { min: Some(0.0), ..Default::default() };
        assert!(!r.is_empty());
    }
}
