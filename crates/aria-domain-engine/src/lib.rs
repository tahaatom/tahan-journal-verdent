//! aria-domain-engine — موتور دامنه معاملاتی کرنل آریا
//!
//! - مدل دامنه (Trade، پاها، اجراها، SourceRecord، ManualOverride)
//! - ۱۱ دستور اتمیک نسخه ۱ با outbox رویداد
//! - سرویس محاسبه R — قطعی و تست‌شده
//! - داده مؤثر (canonical + بازنویسی‌ها)

pub mod commands;
pub mod error;
pub mod events;
pub mod model;
pub mod risk;
pub mod service;

pub use error::{DomainError, DOMAIN_ERROR_RANGE};
pub use events::DOMAIN_EVENT_VERSION;
pub use model::{Direction, Trade, TradeStatus};
pub use risk::{RiskBasis, RiskCalculationStatus, RiskComputation};
pub use service::DomainService;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_smoke() {
        assert_eq!(crate::DOMAIN_ERROR_RANGE, (1400, 1499));
        assert_eq!(crate::DOMAIN_EVENT_VERSION, 1);
    }
}
