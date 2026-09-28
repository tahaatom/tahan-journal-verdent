//! خطاهای موتور رابط کاربری — بازه ۱۸۰۰ تا ۱۸۹۹ طبق `docs/contracts/error-codes.md`.

use std::fmt;

/// خطاهای موتور رابط کاربری.
#[derive(Debug)]
pub enum UiError {
    /// ۱۸۰۱ — اسکیمای اعلانی نامعتبر است.
    InvalidSchema(String),
    /// ۱۸۰۲ — شناسه افزونه UI تکراری است (نقطه + شناسه).
    DuplicateExtension(String),
    /// ۱۸۰۳ — افزونه UI یافت نشد.
    ExtensionNotFound(String),
    /// ۱۸۰۴ — تلاش برای تزریق کامپوننت/رندر غیراعلانی — در نسخه ۱ ممنوع.
    InjectionDenied(String),
}

impl UiError {
    pub const INVALID_SCHEMA: i64 = 1801;
    pub const DUPLICATE_EXTENSION: i64 = 1802;
    pub const EXTENSION_NOT_FOUND: i64 = 1803;
    pub const INJECTION_DENIED: i64 = 1804;

    /// کد ماشین‌خوان خطا.
    pub fn code(&self) -> i64 {
        match self {
            Self::InvalidSchema(_) => Self::INVALID_SCHEMA,
            Self::DuplicateExtension(_) => Self::DUPLICATE_EXTENSION,
            Self::ExtensionNotFound(_) => Self::EXTENSION_NOT_FOUND,
            Self::InjectionDenied(_) => Self::INJECTION_DENIED,
        }
    }

    /// کلید پیام برای i18n.
    pub fn message_key(&self) -> &'static str {
        match self {
            Self::InvalidSchema(_) => "error.ui.invalid_schema",
            Self::DuplicateExtension(_) => "error.ui.duplicate_extension",
            Self::ExtensionNotFound(_) => "error.ui.extension_not_found",
            Self::InjectionDenied(_) => "error.ui.injection_denied",
        }
    }
}

impl fmt::Display for UiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchema(m) => write!(f, "اسکیمای اعلانی نامعتبر (۱۸۰۱): {m}"),
            Self::DuplicateExtension(m) => write!(f, "افزونه UI تکراری (۱۸۰۲): {m}"),
            Self::ExtensionNotFound(m) => write!(f, "افزونه UI یافت نشد (۱۸۰۳): {m}"),
            Self::InjectionDenied(m) => write!(
                f,
                "تزریق کامپوننت در نسخه ۱ ممنوع است (۱۸۰۴): {m}"
            ),
        }
    }
}

impl std::error::Error for UiError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(UiError::InvalidSchema("x".into()).code(), 1801);
        assert_eq!(UiError::DuplicateExtension("x".into()).code(), 1802);
        assert_eq!(UiError::ExtensionNotFound("x".into()).code(), 1803);
        assert_eq!(UiError::InjectionDenied("x".into()).code(), 1804);
    }

    #[test]
    fn message_keys_are_stable() {
        assert_eq!(
            UiError::InvalidSchema("x".into()).message_key(),
            "error.ui.invalid_schema"
        );
        assert_eq!(
            UiError::InjectionDenied("x".into()).message_key(),
            "error.ui.injection_denied"
        );
    }
}
