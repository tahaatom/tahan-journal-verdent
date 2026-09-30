//! خطاهای موتور ایمپورت — بازه کد ۱۵۰۰ (فاز ۱.۱۵).

use thiserror::Error;

/// خطای موتور ایمپورت متاتریدر.
#[derive(Debug, Error)]
pub enum ImportError {
    /// خطای عمومی پیام‌دار (پیام فارسی برای نمایش به کاربر).
    #[error("{0}")]
    Message(String),
    /// خطای ذخیره‌سازی زیربنایی.
    #[error("storage: {0}")]
    Storage(String),
}

impl ImportError {
    /// ساخت خطای پیام‌دار.
    pub fn msg(text: impl Into<String>) -> Self {
        Self::Message(text.into())
    }

    /// کد عددی خطا برای قرارداد IPC.
    pub fn code(&self) -> i64 {
        match self {
            // خطاهای پیام‌دار — بازه ۱۵۰۰
            Self::Message(_) => 1500,
            // خطای ذخیره‌سازی — هم‌تراز موتور ذخیره‌سازی (۱۱۰۰)
            Self::Storage(_) => 1100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(ImportError::msg("x").code(), 1500);
        assert_eq!(ImportError::Storage("x".into()).code(), 1100);
    }
}
