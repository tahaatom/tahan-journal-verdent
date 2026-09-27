//! قرارداد مانیفست پلاگین.
//!
//! مانیفست قرارداد ورودی هر پلاگین است؛ فایل canonical اسکیما در
//! `docs/contracts/plugin-manifest.schema.json` نگه‌داری می‌شود.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// مانیفست پلاگین.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PluginManifest {
    /// شناسه یکتا (مثل "official.mt-import") — الگو: `[a-z0-9]+(\.[a-z0-9-]+)+`
    pub id: String,
    /// نام نمایشی
    pub name: String,
    /// نسخه پلاگین (semver)
    pub version: String,
    /// نسخه API پلاگین (semver)
    pub api_version: String,
    /// نقطه ورود (مسیر اجرایی/اسکریپت نسبت به ریشه پلاگین)
    pub entrypoint: String,
    /// حالت اجرا (فقط out_of_process برای پلاگین‌های نصب‌شدنی)
    pub runtime_mode: RuntimeMode,
    /// بازه سازگاری نسخه کرنل (semver range)
    pub kernel_version_range: String,
    /// بازه سازگاری نسخه API پلاگین (semver range)
    pub api_version_range: String,

    // ---------- فیلدهای اختیاری ----------
    /// قابلیت‌های درخواستی پلاگین
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    /// مجوزهای صریح
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    /// محدودیت منابع
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceLimits>,
    /// وابستگی‌ها به پلاگین‌های دیگر
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    /// زبان‌های پشتیبانی‌شده
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locales: Vec<String>,
    /// نقاط افزونه‌پذیری UI (فقط اعلانی)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ui_extension_points: Vec<UiExtensionPoint>,
    /// توضیحات
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// نویسنده
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// مجوز نشر
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// سطح اعتماد
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_level: Option<TrustLevel>,
    /// سیاست سلامت
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_policy: Option<HealthPolicy>,
}

/// حالت اجرای پلاگین.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    /// فرایند جدا (پیش‌فرض برای همه پلاگین‌های نصب‌شدنی)
    OutOfProcess,
    /// درون‌فرایند — فقط برای ماژول‌های مورد اعتماد داخلی
    InProcessTrusted,
}

/// محدودیت منابع پلاگین.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResourceLimits {
    /// حداکثر حافظه (مگابایت)
    pub max_memory_mb: u64,
    /// حداکثر درصد CPU
    pub max_cpu_percent: u8,
    /// مهلت پیش‌فرض (ثانیه)
    pub timeout_seconds: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self { max_memory_mb: 512, max_cpu_percent: 50, timeout_seconds: 30 }
    }
}

/// نقطه افزونه‌پذیری UI (نسخه ۱ فقط اعلانی).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UiExtensionPoint {
    /// نوع نقطه: dashboard_widget | report_page | command_menu | form_field | plugin_settings
    pub kind: String,
    /// شناسه افزونه
    pub id: String,
    /// اسکیمای اعلانی رندر (بدون کد دلخواه)
    pub schema: BTreeMap<String, serde_json::Value>,
}

/// سطح اعتماد پلاگین.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// رسمی
    Official,
    /// تأییدشده
    Certified,
    /// اجتماع
    Community,
}

/// سیاست سلامت پلاگین.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HealthPolicy {
    /// آستانه قرنطینه: تعداد کرش مجاز (پیش‌فرض ۳)
    #[serde(default = "default_max_crashes")]
    pub max_consecutive_crashes: u32,
    /// فاصله ری‌استارت با backoff (ثانیه)
    #[serde(default = "default_restart_backoff_secs")]
    pub restart_backoff_secs: u64,
}

impl Default for HealthPolicy {
    fn default() -> Self {
        Self {
            max_consecutive_crashes: 3,
            restart_backoff_secs: default_restart_backoff_secs(),
        }
    }
}

fn default_max_crashes() -> u32 {
    3
}

fn default_restart_backoff_secs() -> u64 {
    5
}

/// خطاهای اعتبارسنجی مانیفست.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ManifestError {
    #[error("missing required field: {0}")]
    MissingField(String),
    #[error("invalid field {field}: {reason}")]
    InvalidField { field: String, reason: String },
    #[error("invalid plugin id format: {0}")]
    InvalidId(String),
    #[error("invalid semver in {field}: {value}")]
    InvalidSemver { field: String, value: String },
    #[error("invalid semver range in {field}: {value}")]
    InvalidSemverRange { field: String, value: String },
    #[error("invalid capability: {0}")]
    InvalidCapability(String),
    #[error("invalid runtime mode: {0}")]
    InvalidRuntimeMode(String),
}

/// قابلیت‌های مجاز (سند نسخه ۵).
pub const VALID_CAPABILITIES: &[&str] = &[
    "trades.read",
    "trades.write",
    "trades.delete",
    "fields.read",
    "fields.define",
    "attachments.read",
    "attachments.write",
    "stats.read",
    "ui.widget",
    "ui.page",
    "notifications.show",
    "backup.create",
    "backup.restore",
    "mt.import",
    "mt.live",
    "insights.write",
    "network.access",
];

/// اعتبارسنجی مانیفست (ساختاری + semver + قابلیت‌ها).
/// اعتبارسنجی کامل JSON-Schema در موتور پلاگین (فاز ۱.۷) انجام می‌شود.
pub fn validate_manifest(m: &PluginManifest) -> Result<(), ManifestError> {
    if m.id.is_empty() {
        return Err(ManifestError::MissingField("id".into()));
    }
    let ok = m.id.split('.').all(|p| {
        !p.is_empty()
            && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    });
    if !ok || m.id.starts_with('.') || m.id.ends_with('.') {
        return Err(ManifestError::InvalidId(m.id.clone()));
    }
    if m.name.trim().is_empty() {
        return Err(ManifestError::MissingField("name".into()));
    }
    for (field, value) in [("version", &m.version), ("api_version", &m.api_version)] {
        semver::Version::parse(value).map_err(|_| ManifestError::InvalidSemver {
            field: field.to_string(),
            value: value.clone(),
        })?;
    }
    if m.entrypoint.trim().is_empty() {
        return Err(ManifestError::MissingField("entrypoint".into()));
    }
    if m.runtime_mode == RuntimeMode::InProcessTrusted {
        // درون‌فرایند فقط برای ماژول‌های داخلی — از طریق مانیفست نصب‌شدنی ممنوع
        return Err(ManifestError::InvalidRuntimeMode("in_process_trusted not allowed for installable plugins".into()));
    }
    for (field, value) in [
        ("kernel_version_range", &m.kernel_version_range),
        ("api_version_range", &m.api_version_range),
    ] {
        parse_semver_req(value).map_err(|_| ManifestError::InvalidSemverRange {
            field: field.to_string(),
            value: value.clone(),
        })?;
    }
    for cap in &m.capabilities {
        if !VALID_CAPABILITIES.contains(&cap.as_str()) {
            return Err(ManifestError::InvalidCapability(cap.clone()));
        }
    }
    Ok(())
}

/// پارس ایمن بازه semver (پشتیبانی از قالب‌های "^1.0", ">=1.0, <2.0", "1.2.3").
pub fn parse_semver_req(req: &str) -> Result<semver::VersionReq, String> {
    let normalized = req.trim();
    if normalized.is_empty() {
        return Err("empty range".to_string());
    }
    // قالب کوتاه "^1.0" را به "^1.0.0" تبدیل می‌کنیم
    let expanded = if normalized.starts_with('^')
        && normalized[1..].split('.').count() < 3
    {
        format!("{normalized}.0")
    } else {
        normalized.to_string()
    };
    semver::VersionReq::parse(&expanded).map_err(|e| e.to_string())
}

/// قالب نمونه مانیفست (برای مستندات و تست).
pub fn sample_manifest_json() -> String {
    r#"{
  "id": "official.mt-import",
  "name": "MetaTrader Import",
  "version": "1.0.0",
  "api_version": "1.0.0",
  "entrypoint": "bin/mt-import",
  "runtime_mode": "out_of_process",
  "kernel_version_range": "^1.0.0",
  "api_version_range": "^1.0.0",
  "capabilities": ["mt.import", "trades.read"],
  "permissions": ["mt.import"],
  "resources": { "max_memory_mb": 256, "max_cpu_percent": 30, "timeout_seconds": 60 },
  "trust_level": "official",
  "health_policy": { "max_consecutive_crashes": 3, "restart_backoff_secs": 5 }
}"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> PluginManifest {
        serde_json::from_str(&sample_manifest_json()).unwrap()
    }

    #[test]
    fn sample_manifest_is_valid() {
        let m = valid_manifest();
        validate_manifest(&m).unwrap();
    }

    #[test]
    fn sample_manifest_json_is_parseable() {
        let parsed: serde_json::Value = serde_json::from_str(&sample_manifest_json()).unwrap();
        assert_eq!(parsed["id"], "official.mt-import");
        assert!(parsed["runtime_mode"].is_string());
    }

    #[test]
    fn manifest_serde_roundtrip() {
        let m = valid_manifest();
        let json = serde_json::to_string(&m).unwrap();
        let back: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn missing_required_field_rejected() {
        let json = r#"{
            "id": "official.bad",
            "name": "",
            "version": "1.0.0",
            "api_version": "1.0.0",
            "entrypoint": "bin/x",
            "runtime_mode": "out_of_process",
            "kernel_version_range": "^1.0.0",
            "api_version_range": "^1.0.0"
        }"#;
        let m: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(matches!(validate_manifest(&m), Err(ManifestError::MissingField(_))));
    }

    #[test]
    fn bad_id_rejected() {
        let mut m = valid_manifest();
        m.id = "Bad_ID".to_string();
        assert!(matches!(validate_manifest(&m), Err(ManifestError::InvalidId(_))));
        m.id = "official..bad".to_string();
        assert!(matches!(validate_manifest(&m), Err(ManifestError::InvalidId(_))));
    }

    #[test]
    fn bad_semver_rejected() {
        let mut m = valid_manifest();
        m.version = "one".to_string();
        assert!(matches!(validate_manifest(&m), Err(ManifestError::InvalidSemver { .. })));
        m.version = "1.0.0".to_string();
        m.kernel_version_range = "not-a-range".to_string();
        assert!(matches!(validate_manifest(&m), Err(ManifestError::InvalidSemverRange { .. })));
    }

    #[test]
    fn in_process_mode_rejected_for_installable() {
        let mut m = valid_manifest();
        m.runtime_mode = RuntimeMode::InProcessTrusted;
        assert!(matches!(validate_manifest(&m), Err(ManifestError::InvalidRuntimeMode(_))));
    }

    #[test]
    fn unknown_capability_rejected() {
        let mut m = valid_manifest();
        m.capabilities = vec!["arbitrary.access".to_string()];
        assert!(matches!(validate_manifest(&m), Err(ManifestError::InvalidCapability(_))));
    }

    #[test]
    fn semver_range_short_forms_parse() {
        assert!(parse_semver_req("^1.0").is_ok());
        assert!(parse_semver_req(">=1.0, <2.0").is_ok());
        assert!(parse_semver_req("1.2.3").is_ok());
        assert!(parse_semver_req("").is_err());
        assert!(parse_semver_req("abc").is_err());
    }

    #[test]
    fn missing_optional_fields_use_defaults() {
        let json = r#"{
            "id": "official.min",
            "name": "Minimal",
            "version": "0.1.0",
            "api_version": "1.0.0",
            "entrypoint": "bin/m",
            "runtime_mode": "out_of_process",
            "kernel_version_range": "^1.0.0",
            "api_version_range": "^1.0.0"
        }"#;
        let m: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(m.capabilities.is_empty());
        assert!(m.resources.is_none());
        assert!(m.trust_level.is_none());
        validate_manifest(&m).unwrap();
    }
}
