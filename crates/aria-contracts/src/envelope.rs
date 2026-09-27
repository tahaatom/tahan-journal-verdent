//! پاکت‌های دستور و رویداد — قرارداد نسخه ۱.
//!
//! - دستور: درخواست تغییر وضعیت از صادرکننده به کرنل.
//! - رویداد: اعلان وقوع تغییر، پس از commit، idempotent برای مصرف‌کننده.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// نسخه قرارداد پاکت‌ها.
pub const ENVELOPE_VERSION: u32 = 1;

/// پاکت دستور.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CommandEnvelope {
    /// شناسه یکتای دستور
    pub command_id: Uuid,
    /// نوع دستور (مثل "domain.create_trade")
    pub command_type: String,
    /// بار دستور (ساختار مخصوص هر command_type)
    pub payload: serde_json::Value,
    /// صادرکننده دستور ("ui" | "plugin:<id>" | "kernel:<module>")
    pub issuer: String,
    /// زمان صدور (UTC)
    pub timestamp: DateTime<Utc>,
    /// شناسه همبستگی برای ردیابی زنجیره دستور/رویداد
    pub correlation_id: Uuid,
    /// نسخه قرارداد پاکت
    pub envelope_version: u32,
}

impl CommandEnvelope {
    /// ساخت پاکت دستور جدید با زمان و شناسه‌های خودکار.
    pub fn new(command_type: impl Into<String>, payload: serde_json::Value, issuer: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            command_id: Uuid::new_v4(),
            command_type: command_type.into(),
            payload,
            issuer: issuer.into(),
            timestamp: now,
            correlation_id: Uuid::new_v4(),
            envelope_version: ENVELOPE_VERSION,
        }
    }

    /// ساخت پاکت دستور با correlation_id مشخص (برای زنجیره‌های رویداد).
    pub fn with_correlation(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = correlation_id;
        self
    }
}

/// منبع صدور رویداد.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    /// موتور دامنه
    DomainEngine,
    /// موتور پلاگین
    PluginEngine,
    /// موتور ذخیره‌سازی
    StorageEngine,
    /// موتور امنیت
    SecurityEngine,
    /// موتور پرس‌وجو
    QueryEngine,
    /// موتور اجرا
    RuntimeEngine,
    /// منبع دلخواه (مثل "plugin:<id>")
    Other(String),
}

impl EventSource {
    /// نمایش ماشین‌خوان منبع.
    pub fn as_str(&self) -> &str {
        match self {
            Self::DomainEngine => "domain_engine",
            Self::PluginEngine => "plugin_engine",
            Self::StorageEngine => "storage_engine",
            Self::SecurityEngine => "security_engine",
            Self::QueryEngine => "query_engine",
            Self::RuntimeEngine => "runtime_engine",
            Self::Other(s) => s.as_str(),
        }
    }
}

/// پاکت رویداد.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EventEnvelope {
    /// شناسه یکتای رویداد
    pub event_id: Uuid,
    /// نوع رویداد (مثل "domain.trade_created")
    pub event_type: String,
    /// نسخه ساختار بار این رویداد
    pub event_version: u32,
    /// منبع صدور
    pub source: EventSource,
    /// زمان صدور (UTC)
    pub timestamp: DateTime<Utc>,
    /// شناسه همبستگی (پیوند به دستور عامل)
    pub correlation_id: Uuid,
    /// بار رویداد
    pub payload: serde_json::Value,
}

impl EventEnvelope {
    /// ساخت پاکت رویداد جدید.
    pub fn new(
        event_type: impl Into<String>,
        event_version: u32,
        source: EventSource,
        correlation_id: Uuid,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_type: event_type.into(),
            event_version,
            source,
            timestamp: Utc::now(),
            correlation_id,
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_envelope_serde_roundtrip() {
        let cmd = CommandEnvelope::new("domain.create_trade", serde_json::json!({"x": 1}), "ui");
        let json = serde_json::to_string(&cmd).unwrap();
        let back: CommandEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, back);
        assert_eq!(back.envelope_version, ENVELOPE_VERSION);
    }

    #[test]
    fn event_envelope_serde_roundtrip() {
        let ev = EventEnvelope::new(
            "domain.trade_created",
            1,
            EventSource::DomainEngine,
            Uuid::new_v4(),
            serde_json::json!({"trade_id": "abc"}),
        );
        let json = serde_json::to_string(&ev).unwrap();
        let back: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, back);
        assert_eq!(back.source.as_str(), "domain_engine");
    }

    #[test]
    fn correlation_preserved() {
        let c = Uuid::new_v4();
        let cmd = CommandEnvelope::new("t", serde_json::json!({}), "ui").with_correlation(c);
        assert_eq!(cmd.correlation_id, c);
    }
}
