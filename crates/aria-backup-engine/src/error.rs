//! خطاهای موتور بکاپ و بازیابی — بازه کد ۱۶۰۰ (فاز ۱.۱۶).

use thiserror::Error;

/// خطای موتور بکاپ/بازیابی.
#[derive(Debug, Error)]
pub enum BackupError {
    /// بسته ناسازگار یا خراب (ساختار، جادوگر، کوتاهی) — کد ۱۶۰۰
    #[error("{0}")]
    InvalidPackage(String),
    /// رمزگشایی ناموفق: گذرواژه نادرست یا دستکاری داده — کد ۱۶۰۱
    #[error("رمزگشایی بسته ناموفق بود: گذرواژه نادرست است یا بسته دستکاری شده")]
    WrongPassword,
    /// عدم تطابق چک‌سام blake3 — کد ۱۶۰۲
    #[error("صحت بسته تأیید نشد: {0}")]
    IntegrityFailed(String),
    /// نسخه فرمت پشتیبانی نمی‌شود — کد ۱۶۰۳
    #[error("نسخه فرمت بکاپ {found} پشتیبانی نمی‌شود (حداکثر پشتیبانی‌شده: {supported})")]
    UnsupportedFormat { found: u32, supported: u32 },
    /// گذرواژه ضعیف (سیاست امنیتی) — کد ۱۶۰۴
    #[error("{0}")]
    WeakPassword(String),
    /// خطای ورودی/خروجی فایل یا پوشه — کد ۱۶۰۵
    #[error("خطای فایل‌سیستم: {0}")]
    Io(String),
    /// خطای پایگاه‌داده هنگام snapshot/بازیابی — کد ۱۶۰۶
    #[error("خطای پایگاه‌داده: {0}")]
    Database(String),
    /// بازیابی شکست خورد و وضعیت پیشین بازگردانده شد — کد ۱۶۰۷
    #[error("بازیابی ناموفق بود و وضعیت پیشین بازگردانده شد: {reason}")]
    RestoreRolledBack { reason: String },
}

impl BackupError {
    /// ساخت خطای بسته ناسازگار.
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::InvalidPackage(msg.into())
    }

    /// ساخت خطای صحت.
    pub fn integrity(msg: impl Into<String>) -> Self {
        Self::IntegrityFailed(msg.into())
    }

    /// کد عددی خطا برای قرارداد IPC.
    pub fn code(&self) -> i64 {
        match self {
            Self::InvalidPackage(_) => 1600,
            Self::WrongPassword => 1601,
            Self::IntegrityFailed(_) => 1602,
            Self::UnsupportedFormat { .. } => 1603,
            Self::WeakPassword(_) => 1604,
            Self::Io(_) => 1605,
            Self::Database(_) => 1606,
            Self::RestoreRolledBack { .. } => 1607,
        }
    }
}

impl From<aria_security_engine::SecurityError> for BackupError {
    fn from(e: aria_security_engine::SecurityError) -> Self {
        use aria_security_engine::SecurityError as S;
        match e {
            S::WeakPassword { .. } => Self::WeakPassword(e.to_string()),
            // گذرواژه نادرست یا دستکاری داده — یکسان گزارش می‌شوند (بدون افشای تمایز)
            S::WrongPassword { .. } | S::DecryptionFailed { .. } => Self::WrongPassword,
            _ => Self::InvalidPackage(e.to_string()),
        }
    }
}

impl From<aria_storage_engine::StorageError> for BackupError {
    fn from(e: aria_storage_engine::StorageError) -> Self {
        Self::Database(e.to_string())
    }
}

impl From<std::io::Error> for BackupError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable_and_distinct() {
        assert_eq!(BackupError::invalid("x").code(), 1600);
        assert_eq!(BackupError::WrongPassword.code(), 1601);
        assert_eq!(BackupError::integrity("x").code(), 1602);
        assert_eq!(
            BackupError::UnsupportedFormat { found: 9, supported: 1 }.code(),
            1603
        );
        assert_eq!(BackupError::WeakPassword("x".into()).code(), 1604);
        assert_eq!(BackupError::Io("x".into()).code(), 1605);
        assert_eq!(BackupError::Database("x".into()).code(), 1606);
        assert_eq!(
            BackupError::RestoreRolledBack { reason: "x".into() }.code(),
            1607
        );
    }

    #[test]
    fn security_errors_map_to_backup_codes() {
        // گذرواژه ضعیف → ۱۶۰۴
        let weak = aria_security_engine::validate_password("short").unwrap_err();
        assert_eq!(BackupError::from(weak).code(), 1604);
        // شکست رمزگشایی → ۱۶۰۱
        let dec = aria_security_engine::crypto::decrypt(
            &[0u8; 32],
            &[0u8; 40],
            b"aad",
        )
        .unwrap_err();
        assert_eq!(BackupError::from(dec).code(), 1601);
    }
}
