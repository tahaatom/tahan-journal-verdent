//! رویدادهای دامنه — انتشار پس از commit، idempotent برای مصرف‌کننده.
//!
//! قواعد قرارداد (`docs/contracts/event-contract.md`):
//! - رویدادهای حیاتی در outbox (`system_events`) داخل همان تراکنش نوشته می‌شوند
//!   تا اتمیک بودن دستور تضمین شود؛ «انتشار» (broadcast) پس از commit انجام می‌گیرد.
//! - مصرف‌کننده‌ها با `event_id` تشخیص تکرار می‌دهند.
//! - `StatsInvalidated` رویداد اجباری موتور پرس‌وجو برای ابطال کش است.

use aria_contracts::{EventEnvelope, EventSource};
use uuid::Uuid;

/// نام‌های event_type نسخه ۱ (ثابت‌های پایدار).
pub mod event_type {
    pub const TRADE_CREATED: &str = "domain.trade_created";
    pub const TRADE_UPDATED: &str = "domain.trade_updated";
    pub const TRADE_DELETED: &str = "domain.trade_deleted";
    pub const ENTRY_LEG_ADDED: &str = "domain.entry_leg_added";
    pub const EXIT_LEG_ADDED: &str = "domain.exit_leg_added";
    pub const EXECUTION_ASSIGNED: &str = "domain.execution_assigned";
    pub const OVERRIDE_ADDED: &str = "domain.override_added";
    pub const OVERRIDE_REVERTED: &str = "domain.override_reverted";
    pub const STATS_INVALIDATED: &str = "domain.stats_invalidated";
}

/// نسخه ساختار بار همه رویدادهای دامنه نسخه ۱.
pub const DOMAIN_EVENT_VERSION: u32 = 1;

/// سازنده رویدادهای دامنه — همه با source=domain_engine و correlation_id دستور عامل.
pub struct DomainEventFactory {
    correlation_id: Uuid,
}

impl DomainEventFactory {
    pub fn new(correlation_id: Uuid) -> Self {
        Self { correlation_id }
    }

    fn envelope(&self, event_type: &str, payload: serde_json::Value) -> EventEnvelope {
        EventEnvelope::new(
            event_type,
            DOMAIN_EVENT_VERSION,
            EventSource::DomainEngine,
            self.correlation_id,
            payload,
        )
    }

    pub fn trade_created(&self, trade_id: &str) -> EventEnvelope {
        self.envelope(event_type::TRADE_CREATED, serde_json::json!({ "trade_id": trade_id }))
    }

    pub fn trade_updated(&self, trade_id: &str) -> EventEnvelope {
        self.envelope(event_type::TRADE_UPDATED, serde_json::json!({ "trade_id": trade_id }))
    }

    pub fn trade_deleted(&self, trade_id: &str) -> EventEnvelope {
        self.envelope(
            event_type::TRADE_DELETED,
            serde_json::json!({ "trade_id": trade_id, "soft": true }),
        )
    }

    pub fn entry_leg_added(&self, trade_id: &str, leg_id: &str) -> EventEnvelope {
        self.envelope(
            event_type::ENTRY_LEG_ADDED,
            serde_json::json!({ "trade_id": trade_id, "leg_id": leg_id }),
        )
    }

    pub fn exit_leg_added(&self, trade_id: &str, leg_id: &str) -> EventEnvelope {
        self.envelope(
            event_type::EXIT_LEG_ADDED,
            serde_json::json!({ "trade_id": trade_id, "leg_id": leg_id }),
        )
    }

    pub fn execution_assigned(&self, execution_id: &str, leg_id: &str, trade_id: &str) -> EventEnvelope {
        self.envelope(
            event_type::EXECUTION_ASSIGNED,
            serde_json::json!({ "execution_id": execution_id, "leg_id": leg_id, "trade_id": trade_id }),
        )
    }

    pub fn override_added(&self, override_id: &str, entity_type: &str, entity_id: &str, field_name: &str) -> EventEnvelope {
        self.envelope(
            event_type::OVERRIDE_ADDED,
            serde_json::json!({
                "override_id": override_id,
                "entity_type": entity_type,
                "entity_id": entity_id,
                "field_name": field_name
            }),
        )
    }

    pub fn override_reverted(&self, override_id: &str) -> EventEnvelope {
        self.envelope(event_type::OVERRIDE_REVERTED, serde_json::json!({ "override_id": override_id }))
    }

    /// رویداد ابطال آمار — مصرف‌کننده اجباری: موتور پرس‌وجو.
    pub fn stats_invalidated(&self, trade_id: Option<&str>) -> EventEnvelope {
        self.envelope(
            event_type::STATS_INVALIDATED,
            serde_json::json!({ "trade_id": trade_id, "scope": "trade" }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_types_are_versioned_and_stable() {
        assert!(event_type::TRADE_CREATED.starts_with("domain."));
        assert_eq!(event_type::STATS_INVALIDATED, "domain.stats_invalidated");
        assert_eq!(DOMAIN_EVENT_VERSION, 1);
    }

    #[test]
    fn factory_preserves_correlation_and_source() {
        let c = Uuid::new_v4();
        let f = DomainEventFactory::new(c);
        let ev = f.execution_assigned("e1", "l1", "t1");
        assert_eq!(ev.correlation_id, c);
        assert_eq!(ev.source, EventSource::DomainEngine);
        assert_eq!(ev.event_type, event_type::EXECUTION_ASSIGNED);
        assert_eq!(ev.payload["trade_id"], "t1");
    }

    #[test]
    fn events_are_idempotent_by_unique_event_id() {
        let f = DomainEventFactory::new(Uuid::new_v4());
        let e1 = f.trade_created("t1");
        let e2 = f.trade_created("t1");
        // دو نمونه رویداد برای یک تغییر تکراری id متفاوت دارند؛ مصرف‌کننده با event_id تشخیص می‌دهد
        assert_ne!(e1.event_id, e2.event_id);
        assert_eq!(e1.payload, e2.payload);
    }
}
