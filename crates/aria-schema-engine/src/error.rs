//! مدل خطای موتور اسکیما — بازه کد 1300..=1399.

pub const SCHEMA_ERROR_RANGE: (u32, u32) = (1300, 1399);

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SchemaError {
    #[error("field not found: {id}")]
    FieldNotFound { id: String, code: u32 },
    #[error("duplicate technical key: {key}")]
    DuplicateTechnicalKey { key: String, code: u32 },
    #[error("invalid field definition: {reason}")]
    InvalidDefinition { reason: String, code: u32 },
    #[error("invalid field value: {reason}")]
    InvalidValue { reason: String, code: u32 },
    #[error("storage type change forbidden for field with data: {id}")]
    TypeChangeForbidden { id: String, code: u32 },
    #[error("unknown option value {value} for field {field}")]
    UnknownOption { field: String, value: String, code: u32 },
    #[error("field has data, destructive operation forbidden: {id}")]
    FieldHasData { id: String, code: u32 },
    #[error("schema storage error: {reason}")]
    Storage { reason: String, code: u32 },
}

impl SchemaError {
    pub fn code(&self) -> u32 {
        match self {
            Self::FieldNotFound { code, .. }
            | Self::DuplicateTechnicalKey { code, .. }
            | Self::InvalidDefinition { code, .. }
            | Self::InvalidValue { code, .. }
            | Self::TypeChangeForbidden { code, .. }
            | Self::UnknownOption { code, .. }
            | Self::FieldHasData { code, .. }
            | Self::Storage { code, .. } => *code,
        }
    }

    pub fn variant(&self) -> &'static str {
        match self {
            Self::FieldNotFound { .. } => "field_not_found",
            Self::DuplicateTechnicalKey { .. } => "duplicate_technical_key",
            Self::InvalidDefinition { .. } => "invalid_field_definition",
            Self::InvalidValue { .. } => "invalid_field_value",
            Self::TypeChangeForbidden { .. } => "field_type_change_forbidden",
            Self::UnknownOption { .. } => "unknown_option_value",
            Self::FieldHasData { .. } => "field_has_data",
            Self::Storage { .. } => "schema_storage_error",
        }
    }

    pub fn message_key(&self) -> &'static str {
        match self {
            Self::FieldNotFound { .. } => "error.schema.field_not_found",
            Self::DuplicateTechnicalKey { .. } => "error.schema.duplicate_technical_key",
            Self::InvalidDefinition { .. } => "error.schema.invalid_definition",
            Self::InvalidValue { .. } => "error.schema.invalid_value",
            Self::TypeChangeForbidden { .. } => "error.schema.type_change_forbidden",
            Self::UnknownOption { .. } => "error.schema.unknown_option",
            Self::FieldHasData { .. } => "error.schema.field_has_data",
            Self::Storage { .. } => "error.schema.storage_error",
        }
    }

    // سازنده‌های استاندارد با کد پایدار
    pub fn field_not_found(id: impl Into<String>) -> Self {
        Self::FieldNotFound { id: id.into(), code: 1301 }
    }
    pub fn duplicate_key(key: impl Into<String>) -> Self {
        Self::DuplicateTechnicalKey { key: key.into(), code: 1302 }
    }
    pub fn invalid_definition(reason: impl Into<String>) -> Self {
        Self::InvalidDefinition { reason: reason.into(), code: 1303 }
    }
    pub fn invalid_value(reason: impl Into<String>) -> Self {
        Self::InvalidValue { reason: reason.into(), code: 1304 }
    }
    pub fn type_change_forbidden(id: impl Into<String>) -> Self {
        Self::TypeChangeForbidden { id: id.into(), code: 1305 }
    }
    pub fn unknown_option(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self::UnknownOption { field: field.into(), value: value.into(), code: 1306 }
    }
    pub fn field_has_data(id: impl Into<String>) -> Self {
        Self::FieldHasData { id: id.into(), code: 1307 }
    }
    pub fn storage(reason: impl Into<String>) -> Self {
        Self::Storage { reason: reason.into(), code: 1308 }
    }
}

impl From<rusqlite::Error> for SchemaError {
    fn from(e: rusqlite::Error) -> Self {
        SchemaError::storage(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn error_codes_unique_and_in_range() {
        let errs = vec![
            SchemaError::field_not_found("f1"),
            SchemaError::duplicate_key("k"),
            SchemaError::invalid_definition("r"),
            SchemaError::invalid_value("v"),
            SchemaError::type_change_forbidden("f2"),
            SchemaError::unknown_option("f3", "opt"),
            SchemaError::field_has_data("f4"),
            SchemaError::storage("s"),
        ];
        let mut seen = HashSet::new();
        for e in errs {
            let c = e.code();
            assert!(seen.insert(c), "duplicate code {c}");
            assert!(c >= SCHEMA_ERROR_RANGE.0 && c <= SCHEMA_ERROR_RANGE.1);
            assert!(!e.variant().is_empty());
            assert!(e.message_key().starts_with("error.schema."));
        }
    }
}
