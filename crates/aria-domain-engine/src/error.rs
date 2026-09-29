//! خطاهای موتور دامنه — بازه پایدار ۱۴۰۰ تا ۱۴۹۹.

/// بازه کدهای خطای موتور دامنه (فقط‌افزودنی).
pub const DOMAIN_ERROR_RANGE: (i64, i64) = (1400, 1499);

/// خطای دامنه معاملاتی.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DomainError {
    /// ۱۴۰۱ — معامله یافت نشد (یا حذف نرم شده)
    #[error("trade not found: {id}")]
    TradeNotFound { id: String, code: i64 },

    /// ۱۴۰۲ — حساب یافت نشد
    #[error("trading account not found: {id}")]
    AccountNotFound { id: String, code: i64 },

    /// ۱۴۰۳ — نماد یافت نشد
    #[error("symbol not found: {id}")]
    SymbolNotFound { id: String, code: i64 },

    /// ۱۴۰۴ — داده معامله نامعتبر
    #[error("invalid trade data: {reason}")]
    InvalidTradeData { reason: String, code: i64 },

    /// ۱۴۰۵ — پا یافت نشد
    #[error("leg not found: {id}")]
    LegNotFound { id: String, code: i64 },

    /// ۱۴۰۶ — اجرا یافت نشد
    #[error("execution not found: {id}")]
    ExecutionNotFound { id: String, code: i64 },

    /// ۱۴۰۷ — تضاد تخصیص (اجرا به معامله دیگری تعلق دارد یا نوع پا ناهمخوان است)
    #[error("assignment conflict: {reason}")]
    AssignmentConflict { reason: String, code: i64 },

    /// ۱۴۰۸ — بازنویسی یافت نشد یا بازگشت‌پذیر نیست
    #[error("override error: {reason}")]
    OverrideError { reason: String, code: i64 },

    /// ۱۴۰۹ — پیوست یافت نشد
    #[error("attachment not found: {id}")]
    AttachmentNotFound { id: String, code: i64 },

    /// ۱۴۱۰ — خطای ذخیره‌سازی لایه دامنه
    #[error("domain storage error: {reason}")]
    Storage { reason: String, code: i64 },
}

impl DomainError {
    /// کد ماشین‌خوان خطا.
    pub fn code(&self) -> i64 {
        match self {
            Self::TradeNotFound { code, .. }
            | Self::AccountNotFound { code, .. }
            | Self::SymbolNotFound { code, .. }
            | Self::InvalidTradeData { code, .. }
            | Self::LegNotFound { code, .. }
            | Self::ExecutionNotFound { code, .. }
            | Self::AssignmentConflict { code, .. }
            | Self::OverrideError { code, .. }
            | Self::AttachmentNotFound { code, .. }
            | Self::Storage { code, .. } => *code,
        }
    }

    /// واریانت ماشین‌خوان برای بار خطای RPC.
    pub fn variant(&self) -> &'static str {
        match self {
            Self::TradeNotFound { .. } => "trade_not_found",
            Self::AccountNotFound { .. } => "account_not_found",
            Self::SymbolNotFound { .. } => "symbol_not_found",
            Self::InvalidTradeData { .. } => "invalid_trade_data",
            Self::LegNotFound { .. } => "leg_not_found",
            Self::ExecutionNotFound { .. } => "execution_not_found",
            Self::AssignmentConflict { .. } => "assignment_conflict",
            Self::OverrideError { .. } => "override_error",
            Self::AttachmentNotFound { .. } => "attachment_not_found",
            Self::Storage { .. } => "storage",
        }
    }

    /// کلید پیام فارسی برای i18n متمرکز.
    pub fn message_key(&self) -> String {
        format!("error.domain.{}", self.variant())
    }

    /// ساخت بار خطای RPC طبق قرارداد `aria_contracts::RpcErrorPayload`.
    pub fn rpc_payload(&self) -> aria_contracts::RpcErrorPayload {
        aria_contracts::RpcErrorPayload::new(self.code() as u32, self.variant(), self.message_key())
    }

    pub(crate) fn trade_not_found(id: &str) -> Self {
        Self::TradeNotFound { id: id.to_string(), code: 1401 }
    }

    pub(crate) fn account_not_found(id: &str) -> Self {
        Self::AccountNotFound { id: id.to_string(), code: 1402 }
    }

    pub(crate) fn symbol_not_found(id: &str) -> Self {
        Self::SymbolNotFound { id: id.to_string(), code: 1403 }
    }

    pub(crate) fn invalid(reason: impl Into<String>) -> Self {
        Self::InvalidTradeData { reason: reason.into(), code: 1404 }
    }

    pub(crate) fn leg_not_found(id: &str) -> Self {
        Self::LegNotFound { id: id.to_string(), code: 1405 }
    }

    pub(crate) fn execution_not_found(id: &str) -> Self {
        Self::ExecutionNotFound { id: id.to_string(), code: 1406 }
    }

    pub(crate) fn assignment_conflict(reason: impl Into<String>) -> Self {
        Self::AssignmentConflict { reason: reason.into(), code: 1407 }
    }

    pub(crate) fn override_error(reason: impl Into<String>) -> Self {
        Self::OverrideError { reason: reason.into(), code: 1408 }
    }

    pub(crate) fn attachment_not_found(id: &str) -> Self {
        Self::AttachmentNotFound { id: id.to_string(), code: 1409 }
    }

    pub(crate) fn storage(reason: impl Into<String>) -> Self {
        Self::Storage { reason: reason.into(), code: 1410 }
    }
}

impl From<rusqlite::Error> for DomainError {
    fn from(e: rusqlite::Error) -> Self {
        Self::storage(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable_and_in_range() {
        assert_eq!(DOMAIN_ERROR_RANGE, (1400, 1499));
        assert_eq!(DomainError::trade_not_found("t1").code(), 1401);
        assert_eq!(DomainError::account_not_found("a1").code(), 1402);
        assert_eq!(DomainError::symbol_not_found("s1").code(), 1403);
        assert_eq!(DomainError::invalid("x").code(), 1404);
        assert_eq!(DomainError::leg_not_found("l1").code(), 1405);
        assert_eq!(DomainError::execution_not_found("e1").code(), 1406);
        assert_eq!(DomainError::assignment_conflict("x").code(), 1407);
        assert_eq!(DomainError::override_error("x").code(), 1408);
        assert_eq!(DomainError::attachment_not_found("f1").code(), 1409);
        assert_eq!(DomainError::storage("x").code(), 1410);
    }

    #[test]
    fn from_rusqlite_maps_to_storage() {
        let e: DomainError =
            rusqlite::Error::InvalidParameterName("bad".into()).into();
        assert_eq!(e.code(), 1410);
    }

    #[test]
    fn rpc_payload_matches_contract_shape() {
        let e = DomainError::trade_not_found("t1");
        let p = e.rpc_payload();
        assert_eq!(p.code, 1401);
        assert_eq!(p.variant, "trade_not_found");
        assert_eq!(p.message_key, "error.domain.trade_not_found");
    }
}
