//! مدل خطای موتور ذخیره‌سازی — بازه کد 1100..=1199.

pub const STORAGE_ERROR_RANGE: (u32, u32) = (1100, 1199);

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum StorageError {
    #[error("database open failed: {reason}")]
    OpenFailed { reason: String, code: u32 },
    #[error("migration failed at version {version}: {reason}")]
    MigrationFailed { version: i64, reason: String, code: u32 },
    #[error("query failed: {reason}")]
    QueryFailed { reason: String, code: u32 },
    #[error("integrity check failed: {reason}")]
    IntegrityFailed { reason: String, code: u32 },
    #[error("entity not found: {entity}:{id}")]
    NotFound { entity: String, id: String, code: u32 },
    #[error("constraint violation: {reason}")]
    Constraint { reason: String, code: u32 },
    #[error("transaction failed: {reason}")]
    TransactionFailed { reason: String, code: u32 },
    #[error("backup error: {reason}")]
    Backup { reason: String, code: u32 },
    #[error("attachment error: {reason}")]
    Attachment { reason: String, code: u32 },
}

impl StorageError {
    pub fn code(&self) -> u32 {
        match self {
            Self::OpenFailed { code, .. }
            | Self::MigrationFailed { code, .. }
            | Self::QueryFailed { code, .. }
            | Self::IntegrityFailed { code, .. }
            | Self::NotFound { code, .. }
            | Self::Constraint { code, .. }
            | Self::TransactionFailed { code, .. }
            | Self::Backup { code, .. }
            | Self::Attachment { code, .. } => *code,
        }
    }

    pub fn variant(&self) -> &'static str {
        match self {
            Self::OpenFailed { .. } => "db_open_failed",
            Self::MigrationFailed { .. } => "db_migration_failed",
            Self::QueryFailed { .. } => "db_query_failed",
            Self::IntegrityFailed { .. } => "db_integrity_failed",
            Self::NotFound { .. } => "db_not_found",
            Self::Constraint { .. } => "db_constraint_violation",
            Self::TransactionFailed { .. } => "db_transaction_failed",
            Self::Backup { .. } => "db_backup_error",
            Self::Attachment { .. } => "db_attachment_error",
        }
    }

    pub fn message_key(&self) -> &'static str {
        match self {
            Self::OpenFailed { .. } => "error.storage.open_failed",
            Self::MigrationFailed { .. } => "error.storage.migration_failed",
            Self::QueryFailed { .. } => "error.storage.query_failed",
            Self::IntegrityFailed { .. } => "error.storage.integrity_failed",
            Self::NotFound { .. } => "error.storage.not_found",
            Self::Constraint { .. } => "error.storage.constraint_violation",
            Self::TransactionFailed { .. } => "error.storage.transaction_failed",
            Self::Backup { .. } => "error.storage.backup_error",
            Self::Attachment { .. } => "error.storage.attachment_error",
        }
    }

    // سازنده‌های استاندارد با کد پایدار
    pub fn open(reason: impl Into<String>) -> Self {
        Self::OpenFailed { reason: reason.into(), code: 1101 }
    }
    pub fn migration(version: i64, reason: impl Into<String>) -> Self {
        Self::MigrationFailed { version, reason: reason.into(), code: 1102 }
    }
    pub fn query(reason: impl Into<String>) -> Self {
        Self::QueryFailed { reason: reason.into(), code: 1103 }
    }
    pub fn integrity(reason: impl Into<String>) -> Self {
        Self::IntegrityFailed { reason: reason.into(), code: 1104 }
    }
    pub fn not_found(entity: impl Into<String>, id: impl Into<String>) -> Self {
        Self::NotFound { entity: entity.into(), id: id.into(), code: 1105 }
    }
    pub fn constraint(reason: impl Into<String>) -> Self {
        Self::Constraint { reason: reason.into(), code: 1106 }
    }
    pub fn transaction(reason: impl Into<String>) -> Self {
        Self::TransactionFailed { reason: reason.into(), code: 1107 }
    }
    pub fn backup(reason: impl Into<String>) -> Self {
        Self::Backup { reason: reason.into(), code: 1108 }
    }
    pub fn attachment(reason: impl Into<String>) -> Self {
        Self::Attachment { reason: reason.into(), code: 1109 }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        match &e {
            rusqlite::Error::SqliteFailure(err, msg)
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                StorageError::constraint(msg.clone().unwrap_or_else(|| err.to_string()))
            }
            rusqlite::Error::QueryReturnedNoRows => StorageError::not_found("entity", ""),
            _ => StorageError::query(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn error_codes_unique_and_in_range() {
        let errs = vec![
            StorageError::open("x"),
            StorageError::migration(1, "y"),
            StorageError::query("z"),
            StorageError::integrity("i"),
            StorageError::not_found("t", "1"),
            StorageError::constraint("c"),
            StorageError::transaction("tr"),
            StorageError::backup("b"),
            StorageError::attachment("a"),
        ];
        let mut seen = HashSet::new();
        for e in errs {
            let c = e.code();
            assert!(seen.insert(c), "duplicate code {c}");
            assert!(c >= STORAGE_ERROR_RANGE.0 && c <= STORAGE_ERROR_RANGE.1);
            assert!(!e.variant().is_empty());
            assert!(e.message_key().starts_with("error.storage."));
        }
    }
}
