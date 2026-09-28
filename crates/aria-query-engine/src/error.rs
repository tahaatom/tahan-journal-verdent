//! خطاهای موتور پرس‌وجو — بازه ۱۷۰۰ تا ۱۷۹۹ طبق `docs/contracts/error-codes.md`.

use std::fmt;

/// خطاهای موتور پرس‌وجو.
#[derive(Debug)]
pub enum QueryError {
    /// ۱۷۰۱ — فیلتر، صفحه یا پارامتر پرس‌وجو نامعتبر است.
    InvalidQuery(String),
    /// ۱۷۰۲ — بودجه پرس‌وجو (اندازه صفحه / گام‌های اجرا) فراتر رفت.
    BudgetExceeded(String),
    /// ۱۷۰۳ — فیلد سفارشی برای پرس‌وجو/گروه‌بندی مجاز نیست یا وجود ندارد.
    FieldNotQueryable(String),
    /// ۱۷۰۴ — خطای ذخیره‌سازی زیرین.
    Storage(rusqlite::Error),
}

impl QueryError {
    pub const INVALID_QUERY: i64 = 1701;
    pub const BUDGET_EXCEEDED: i64 = 1702;
    pub const FIELD_NOT_QUERYABLE: i64 = 1703;
    pub const STORAGE: i64 = 1704;

    /// کد ماشین‌خوان خطا.
    pub fn code(&self) -> i64 {
        match self {
            Self::InvalidQuery(_) => Self::INVALID_QUERY,
            Self::BudgetExceeded(_) => Self::BUDGET_EXCEEDED,
            Self::FieldNotQueryable(_) => Self::FIELD_NOT_QUERYABLE,
            Self::Storage(_) => Self::STORAGE,
        }
    }

    /// کلید پیام برای i18n.
    pub fn message_key(&self) -> &'static str {
        match self {
            Self::InvalidQuery(_) => "error.query.invalid_query",
            Self::BudgetExceeded(_) => "error.query.budget_exceeded",
            Self::FieldNotQueryable(_) => "error.query.field_not_queryable",
            Self::Storage(_) => "error.query.storage_error",
        }
    }

    pub fn invalid_query(message: impl Into<String>) -> Self {
        Self::InvalidQuery(message.into())
    }

    pub fn budget_exceeded(message: impl Into<String>) -> Self {
        Self::BudgetExceeded(message.into())
    }

    pub fn field_not_queryable(message: impl Into<String>) -> Self {
        Self::FieldNotQueryable(message.into())
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery(m) => write!(f, "پرس‌وجوی نامعتبر (۱۷۰۱): {m}"),
            Self::BudgetExceeded(m) => write!(f, "بودجه پرس‌وجو فراتر رفت (۱۷۰۲): {m}"),
            Self::FieldNotQueryable(m) => {
                write!(f, "فیلد سفارشی برای پرس‌وجو مجاز نیست (۱۷۰۳): {m}")
            }
            Self::Storage(e) => write!(f, "خطای ذخیره‌سازی پرس‌وجو (۱۷۰۴): {e}"),
        }
    }
}

impl std::error::Error for QueryError {}

impl From<rusqlite::Error> for QueryError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(QueryError::invalid_query("x").code(), 1701);
        assert_eq!(QueryError::budget_exceeded("x").code(), 1702);
        assert_eq!(QueryError::field_not_queryable("x").code(), 1703);
        assert_eq!(
            QueryError::Storage(rusqlite::Error::InvalidQuery).code(),
            1704
        );
    }

    #[test]
    fn message_keys_are_stable() {
        assert_eq!(
            QueryError::invalid_query("x").message_key(),
            "error.query.invalid_query"
        );
        assert_eq!(
            QueryError::Storage(rusqlite::Error::InvalidQuery).message_key(),
            "error.query.storage_error"
        );
    }
}
