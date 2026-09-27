//! مدل خطای موتور پایه.
//!
//! قوانین:
//! - هر خطا «کد عددی یکتا»، «واریانت ماشین‌خوان» و «کلید پیام فارسی» دارد.
//! - کد بازه‌ی موتور پایه: 1000..=1099.
//! - `anyhow` فقط در مرزهای اپلیکیشن (نه داخل API عمومی موتورها) مجاز است.

use std::fmt;

/// بازه کدهای خطای موتور پایه — باید در docs/contracts/error-codes.md منعکس شود.
pub const FOUNDATION_ERROR_RANGE: (u32, u32) = (1000, 1099);

/// خطای موتور پایه کرنل آریا.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FoundationError {
    /// بارگذاری فایل پیکربندی ممکن نشد.
    #[error("config load failed: {path}")]
    ConfigLoadFailed { path: String, code: u32 },

    /// پارس پیکربندی شکست خورد.
    #[error("config parse failed: {reason}")]
    ConfigParseFailed { reason: String, code: u32 },

    /// ذخیره پیکربندی شکست خورد.
    #[error("config save failed: {path}")]
    ConfigSaveFailed { path: String, code: u32 },

    /// خطای I/O عمومی.
    #[error("io error: {reason}")]
    Io { reason: String, code: u32 },

    /// تاریخ/زمان نامعتبر.
    #[error("invalid date/time: {reason}")]
    InvalidDateTime { reason: String, code: u32 },

    /// مقدار پیکربندی خارج از محدوده مجاز.
    #[error("invalid config value: {field}")]
    InvalidConfigValue { field: String, code: u32 },
}

impl FoundationError {
    /// کد عددی یکتای خطا (پایدار و مستقل از پیام).
    pub fn code(&self) -> u32 {
        match self {
            Self::ConfigLoadFailed { code, .. }
            | Self::ConfigParseFailed { code, .. }
            | Self::ConfigSaveFailed { code, .. }
            | Self::Io { code, .. }
            | Self::InvalidDateTime { code, .. }
            | Self::InvalidConfigValue { code, .. } => *code,
        }
    }

    /// واریانت ماشین‌خوان (برای RPC و لاگ).
    pub fn variant(&self) -> &'static str {
        match self {
            Self::ConfigLoadFailed { .. } => "config_load_failed",
            Self::ConfigParseFailed { .. } => "config_parse_failed",
            Self::ConfigSaveFailed { .. } => "config_save_failed",
            Self::Io { .. } => "io",
            Self::InvalidDateTime { .. } => "invalid_date_time",
            Self::InvalidConfigValue { .. } => "invalid_config_value",
        }
    }

    /// کلید پیام قابل نمایش فارسی (ترجمه در i18n متمرکز نگه داشته می‌شود).
    pub fn message_key(&self) -> &'static str {
        match self {
            Self::ConfigLoadFailed { .. } => "error.foundation.config_load_failed",
            Self::ConfigParseFailed { .. } => "error.foundation.config_parse_failed",
            Self::ConfigSaveFailed { .. } => "error.foundation.config_save_failed",
            Self::Io { .. } => "error.foundation.io",
            Self::InvalidDateTime { .. } => "error.foundation.invalid_date_time",
            Self::InvalidConfigValue { .. } => "error.foundation.invalid_config_value",
        }
    }
}

/// نوع خطای عمومی کرنل. موتورهای سطح بالا می‌توانند خطای خود را با کد متمایز داشته باشند؛
/// این تریت قرارداد مشترک همه خطاهای کرنل است.
pub trait KernelError: fmt::Debug + Send + Sync + 'static {
    /// کد عددی یکتا.
    fn code(&self) -> u32;
    /// واریانت ماشین‌خوان.
    fn variant(&self) -> &'static str;
    /// کلید پیام فارسی.
    fn message_key(&self) -> &'static str;
}

impl KernelError for FoundationError {
    fn code(&self) -> u32 {
        FoundationError::code(self)
    }
    fn variant(&self) -> &'static str {
        FoundationError::variant(self)
    }
    fn message_key(&self) -> &'static str {
        FoundationError::message_key(self)
    }
}

impl FoundationError {
    /// سازنده‌های کمکی با کد استاندارد.
    pub fn config_load(path: impl Into<String>) -> Self {
        Self::ConfigLoadFailed { path: path.into(), code: 1001 }
    }
    pub fn config_parse(reason: impl Into<String>) -> Self {
        Self::ConfigParseFailed { reason: reason.into(), code: 1002 }
    }
    pub fn config_save(path: impl Into<String>) -> Self {
        Self::ConfigSaveFailed { path: path.into(), code: 1003 }
    }
    pub fn io(reason: impl Into<String>) -> Self {
        Self::Io { reason: reason.into(), code: 1004 }
    }
    pub fn invalid_datetime(reason: impl Into<String>) -> Self {
        Self::InvalidDateTime { reason: reason.into(), code: 1005 }
    }
    pub fn invalid_config_value(field: impl Into<String>) -> Self {
        Self::InvalidConfigValue { field: field.into(), code: 1006 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn error_codes_are_unique() {
        let errs = vec![
            FoundationError::config_load("a"),
            FoundationError::config_parse("b"),
            FoundationError::config_save("c"),
            FoundationError::io("d"),
            FoundationError::invalid_datetime("e"),
            FoundationError::invalid_config_value("f"),
        ];
        let mut seen = HashSet::new();
        for e in errs {
            assert!(seen.insert(e.code()), "duplicate code: {}", e.code());
            assert!(!e.variant().is_empty());
            assert!(e.message_key().starts_with("error.foundation."));
        }
    }

    #[test]
    fn error_codes_within_engine_range() {
        let errs = vec![
            FoundationError::config_load("a"),
            FoundationError::config_parse("b"),
            FoundationError::config_save("c"),
            FoundationError::io("d"),
            FoundationError::invalid_datetime("e"),
            FoundationError::invalid_config_value("f"),
        ];
        for e in errs {
            assert!(e.code() >= FOUNDATION_ERROR_RANGE.0 && e.code() <= FOUNDATION_ERROR_RANGE.1);
        }
    }

    #[test]
    fn display_is_stable() {
        let e = FoundationError::config_load("x.json");
        assert_eq!(e.to_string(), "config load failed: x.json");
        assert_eq!(e.code(), 1001);
        assert_eq!(e.variant(), "config_load_failed");
    }
}
