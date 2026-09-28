//! رویدادهای موتور پلاگین — انتشار از outbox پس از commit.

use aria_contracts::{EventEnvelope, EventSource};
use uuid::Uuid;

/// نام‌های event_type نسخه ۱ (ثابت‌های پایدار).
pub mod event_type {
    pub const PLUGIN_INSTALLED: &str = "plugin.installed";
    pub const PLUGIN_ENABLED: &str = "plugin.enabled";
    pub const PLUGIN_STARTED: &str = "plugin.started";
    pub const PLUGIN_STOPPED: &str = "plugin.stopped";
    pub const PLUGIN_CRASHED: &str = "plugin.crashed";
    pub const PLUGIN_QUARANTINED: &str = "plugin.quarantined";
    pub const PLUGIN_DISABLED: &str = "plugin.disabled";
    pub const PLUGIN_RELEASED: &str = "plugin.released_from_quarantine";
}

/// نسخه ساختار بار رویدادهای پلاگین.
pub const PLUGIN_EVENT_VERSION: u32 = 1;

/// سازنده رویدادهای پلاگین.
pub struct PluginEventFactory {
    correlation_id: Uuid,
}

impl PluginEventFactory {
    pub fn new(correlation_id: Uuid) -> Self {
        Self { correlation_id }
    }

    fn envelope(&self, event_type: &str, payload: serde_json::Value) -> EventEnvelope {
        EventEnvelope::new(
            event_type,
            PLUGIN_EVENT_VERSION,
            EventSource::PluginEngine,
            self.correlation_id,
            payload,
        )
    }

    pub fn installed(&self, plugin_id: &str) -> EventEnvelope {
        self.envelope(event_type::PLUGIN_INSTALLED, serde_json::json!({ "plugin_id": plugin_id }))
    }

    pub fn enabled(&self, plugin_id: &str) -> EventEnvelope {
        self.envelope(event_type::PLUGIN_ENABLED, serde_json::json!({ "plugin_id": plugin_id }))
    }

    pub fn started(&self, plugin_id: &str) -> EventEnvelope {
        self.envelope(event_type::PLUGIN_STARTED, serde_json::json!({ "plugin_id": plugin_id }))
    }

    pub fn stopped(&self, plugin_id: &str) -> EventEnvelope {
        self.envelope(event_type::PLUGIN_STOPPED, serde_json::json!({ "plugin_id": plugin_id }))
    }

    pub fn crashed(&self, plugin_id: &str, crash_count: i64, error: &str) -> EventEnvelope {
        self.envelope(
            event_type::PLUGIN_CRASHED,
            serde_json::json!({
                "plugin_id": plugin_id,
                "crash_count": crash_count,
                "error": error
            }),
        )
    }

    pub fn quarantined(&self, plugin_id: &str, crash_count: i64) -> EventEnvelope {
        self.envelope(
            event_type::PLUGIN_QUARANTINED,
            serde_json::json!({ "plugin_id": plugin_id, "crash_count": crash_count }),
        )
    }

    pub fn disabled(&self, plugin_id: &str) -> EventEnvelope {
        self.envelope(event_type::PLUGIN_DISABLED, serde_json::json!({ "plugin_id": plugin_id }))
    }

    pub fn released(&self, plugin_id: &str) -> EventEnvelope {
        self.envelope(
            event_type::PLUGIN_RELEASED,
            serde_json::json!({ "plugin_id": plugin_id, "by": "user" }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_types_are_versioned_and_stable() {
        assert!(event_type::PLUGIN_INSTALLED.starts_with("plugin."));
        assert_eq!(event_type::PLUGIN_QUARANTINED, "plugin.quarantined");
        assert_eq!(PLUGIN_EVENT_VERSION, 1);
    }

    #[test]
    fn factory_uses_plugin_engine_source() {
        let c = Uuid::new_v4();
        let f = PluginEventFactory::new(c);
        let ev = f.crashed("com.x.y", 2, "panic: boom");
        assert_eq!(ev.source, EventSource::PluginEngine);
        assert_eq!(ev.correlation_id, c);
        assert_eq!(ev.payload["crash_count"], 2);
    }
}
