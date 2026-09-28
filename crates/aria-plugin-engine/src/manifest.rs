//! مانیفست پلاگین — اعتبارسنجی کامل مطابق `plugin-manifest.schema.json`.
//!
//! قواعد (انعکاس یک‌به‌یک اسکیمای قرارداد):
//! - فیلدهای الزامی: id, name, version, api_version, entrypoint, runtime_mode,
//!   kernel_version_range, api_version_range
//! - فیلد ناشناخته ممنوع (additionalProperties: false)
//! - الگوی شناسه: `^[a-z0-9]+(\.[a-z0-9-]+)+$`
//! - version و api_version باید semver معتبر باشند
//! - بازه‌های سازگاری باید نحو معتبر داشته باشند
//! - capabilities فقط از فهرست بسته
//! - resources با محدوده‌های تعریف‌شده

use crate::error::PluginError;
use crate::semver::{SemVer, VersionRange};
use serde::{Deserialize, Serialize};

/// الگوی شناسه پلاگین — معادل الگوی اسکیما.
fn valid_plugin_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
                && p.chars().next().map(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).unwrap_or(false)
        })
}

/// حالت اجرا.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    OutOfProcess,
    InProcessTrusted,
}

impl RuntimeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OutOfProcess => "out_of_process",
            Self::InProcessTrusted => "in_process_trusted",
        }
    }
}

/// سطح اعتماد.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Official,
    Certified,
    Community,
}

/// سیاست منابع — مقادیر پیش‌فرض در غیاب مانیفست.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResourcePolicy {
    pub max_memory_mb: u32,
    pub max_cpu_percent: u32,
    pub timeout_seconds: u32,
}

impl Default for ResourcePolicy {
    fn default() -> Self {
        Self { max_memory_mb: 512, max_cpu_percent: 50, timeout_seconds: 30 }
    }
}

/// سیاست سلامت — آستانه قرنطینه و فاصله راه‌اندازی مجدد.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HealthPolicy {
    pub max_consecutive_crashes: u32,
    pub restart_backoff_secs: u32,
}

impl Default for HealthPolicy {
    fn default() -> Self {
        Self { max_consecutive_crashes: 3, restart_backoff_secs: 5 }
    }
}

/// مانیفست پلاگین — نسخه ۱.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub entrypoint: String,
    pub runtime_mode: String,
    pub kernel_version_range: String,
    pub api_version_range: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub resources: ResourceOptions,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub locales: Vec<String>,
    #[serde(default)]
    pub ui_extension_points: Vec<UiExtensionPoint>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub trust_level: String,
    #[serde(default)]
    pub health_policy: HealthPolicyOptions,
}

/// گزینه‌های منابع خام مانیفست (فیلدهای اختیاری).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ResourceOptions {
    pub max_memory_mb: Option<u32>,
    pub max_cpu_percent: Option<u32>,
    pub timeout_seconds: Option<u32>,
}

/// گزینه‌های سلامت خام مانیفست.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct HealthPolicyOptions {
    pub max_consecutive_crashes: Option<u32>,
    pub restart_backoff_secs: Option<u32>,
}

/// نقطه اعلانی UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct UiExtensionPoint {
    pub kind: String,
    pub id: String,
    pub schema: serde_json::Value,
}

/// انواع مجاز نقطه اعلانی UI.
pub const UI_EXTENSION_KINDS: [&str; 5] =
    ["dashboard_widget", "report_page", "command_menu", "form_field", "plugin_settings"];

/// سطح اعتماد مجاز.
pub const TRUST_LEVELS: [&str; 3] = ["official", "certified", "community"];

impl PluginManifest {
    /// تجزیه و اعتبارسنجی کامل مانیفست از JSON.
    /// هر ایراد با کد ۱۵۰۱ و توضیح دقیق رد می‌شود — هرگز مانیفست ناقص نمی‌پذیرد.
    pub fn parse(json: &str) -> Result<Self, PluginError> {
        let manifest: PluginManifest = serde_json::from_str(json)
            .map_err(|e| PluginError::manifest_invalid(format!("manifest JSON: {e}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// اعتبارسنجی مانیفست تجزیه‌شده.
    pub fn validate(&self) -> Result<(), PluginError> {
        if !valid_plugin_id(&self.id) {
            return Err(PluginError::manifest_invalid(format!(
                "invalid plugin id (expected lowercase.dotted.format): {}",
                self.id
            )));
        }
        if self.name.trim().is_empty() {
            return Err(PluginError::manifest_invalid("name must not be empty"));
        }
        if self.entrypoint.trim().is_empty() {
            return Err(PluginError::manifest_invalid("entrypoint must not be empty"));
        }
        match self.runtime_mode.as_str() {
            "out_of_process" | "in_process_trusted" => {}
            other => {
                return Err(PluginError::manifest_invalid(format!(
                    "invalid runtime_mode: {other}"
                )))
            }
        }
        // اجرای درون‌فرایند فقط برای ماژول‌های مورد اعتماد داخلی (سطح رسمی) مجاز است
        if self.runtime_mode == "in_process_trusted"
            && self.trust_level != "official"
        {
            return Err(PluginError::manifest_invalid(
                "in_process_trusted requires trust_level 'official' (internal trusted modules only)",
            ));
        }
        SemVer::parse(&self.version)
            .map_err(|e| PluginError::manifest_invalid(format!("version: {e}")))?;
        SemVer::parse(&self.api_version)
            .map_err(|e| PluginError::manifest_invalid(format!("api_version: {e}")))?;
        VersionRange::parse(&self.kernel_version_range)
            .map_err(|e| PluginError::manifest_invalid(format!("kernel_version_range: {e}")))?;
        VersionRange::parse(&self.api_version_range)
            .map_err(|e| PluginError::manifest_invalid(format!("api_version_range: {e}")))?;
        for cap in &self.capabilities {
            crate::permissions::Capability::parse(cap)
                .ok_or_else(|| PluginError::unknown_capability(cap))?;
        }
        if let Some(mem) = self.resources.max_memory_mb {
            if !(32..=8192).contains(&mem) {
                return Err(PluginError::manifest_invalid(format!(
                    "max_memory_mb out of range 32..=8192: {mem}"
                )));
            }
        }
        if let Some(cpu) = self.resources.max_cpu_percent {
            if !(1..=100).contains(&cpu) {
                return Err(PluginError::manifest_invalid(format!(
                    "max_cpu_percent out of range 1..=100: {cpu}"
                )));
            }
        }
        if let Some(t) = self.resources.timeout_seconds {
            if !(1..=3600).contains(&t) {
                return Err(PluginError::manifest_invalid(format!(
                    "timeout_seconds out of range 1..=3600: {t}"
                )));
            }
        }
        if let Some(n) = self.health_policy.max_consecutive_crashes {
            if !(1..=10).contains(&n) {
                return Err(PluginError::manifest_invalid(format!(
                    "max_consecutive_crashes out of range 1..=10: {n}"
                )));
            }
        }
        if let Some(b) = self.health_policy.restart_backoff_secs {
            if !(1..=300).contains(&b) {
                return Err(PluginError::manifest_invalid(format!(
                    "restart_backoff_secs out of range 1..=300: {b}"
                )));
            }
        }
        if !self.trust_level.is_empty() && !TRUST_LEVELS.contains(&self.trust_level.as_str()) {
            return Err(PluginError::manifest_invalid(format!(
                "invalid trust_level: {}",
                self.trust_level
            )));
        }
        for point in &self.ui_extension_points {
            if !UI_EXTENSION_KINDS.contains(&point.kind.as_str()) {
                return Err(PluginError::manifest_invalid(format!(
                    "invalid ui_extension_point kind: {}",
                    point.kind
                )));
            }
            if point.id.trim().is_empty() {
                return Err(PluginError::manifest_invalid(
                    "ui_extension_point.id must not be empty",
                ));
            }
            if !point.schema.is_object() {
                return Err(PluginError::manifest_invalid(
                    "ui_extension_point.schema must be an object (declarative render only)",
                ));
            }
        }
        Ok(())
    }

    /// نسخه مانیفست به‌صورت SemVer.
    pub fn version_parsed(&self) -> Result<SemVer, PluginError> {
        SemVer::parse(&self.version).map_err(PluginError::manifest_invalid)
    }

    /// سیاست منابع مؤثر — مقادیر مانیفست روی پیش‌فرض‌ها.
    pub fn effective_resources(&self) -> ResourcePolicy {
        ResourcePolicy {
            max_memory_mb: self.resources.max_memory_mb.unwrap_or(512),
            max_cpu_percent: self.resources.max_cpu_percent.unwrap_or(50),
            timeout_seconds: self.resources.timeout_seconds.unwrap_or(30),
        }
    }

    /// سیاست سلامت مؤثر — آستانه قرنطینه (پیش‌فرض: ۳).
    pub fn effective_health(&self) -> HealthPolicy {
        HealthPolicy {
            max_consecutive_crashes: self.health_policy.max_consecutive_crashes.unwrap_or(3),
            restart_backoff_secs: self.health_policy.restart_backoff_secs.unwrap_or(5),
        }
    }

    /// آیا پلاگین به دسترسی شبکه نیاز دارد؟
    pub fn needs_network(&self) -> bool {
        self.capabilities.iter().any(|c| c == "network.access")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_manifest() -> String {
        r#"{
            "id": "com.example.myplugin",
            "name": "پلاگین نمونه",
            "version": "1.0.0",
            "api_version": "1.0.0",
            "entrypoint": "main.py",
            "runtime_mode": "out_of_process",
            "kernel_version_range": ">=1.0.0, <2.0.0",
            "api_version_range": "^1.0.0",
            "capabilities": ["trades.read", "stats.read"]
        }"#
        .to_string()
    }

    #[test]
    fn valid_manifest_parses() {
        let m = PluginManifest::parse(&base_manifest()).unwrap();
        assert_eq!(m.id, "com.example.myplugin");
        assert_eq!(m.capabilities.len(), 2);
        assert_eq!(m.effective_resources().timeout_seconds, 30);
        assert_eq!(m.effective_health().max_consecutive_crashes, 3);
        assert!(!m.needs_network());
    }

    #[test]
    fn custom_resources_and_health() {
        let json = base_manifest().replace(
            "\"capabilities\": [\"trades.read\", \"stats.read\"]",
            "\"capabilities\": [\"trades.read\"],\n\"resources\": {\"max_memory_mb\": 256, \"max_cpu_percent\": 25, \"timeout_seconds\": 60},\n\"health_policy\": {\"max_consecutive_crashes\": 5, \"restart_backoff_secs\": 10}",
        );
        let m = PluginManifest::parse(&json).unwrap();
        let r = m.effective_resources();
        assert_eq!((r.max_memory_mb, r.max_cpu_percent, r.timeout_seconds), (256, 25, 60));
        assert_eq!(m.effective_health().max_consecutive_crashes, 5);
    }

    #[test]
    fn missing_required_field_rejected() {
        let json = base_manifest().replace("\"entrypoint\": \"main.py\",", "");
        let err = PluginManifest::parse(&json).unwrap_err();
        assert_eq!(err.code(), 1501);
    }

    #[test]
    fn unknown_field_rejected() {
        let json = base_manifest().replace(
            "\"capabilities\": [\"trades.read\", \"stats.read\"]",
            "\"capabilities\": [], \"hacker_field\": true",
        );
        let err = PluginManifest::parse(&json).unwrap_err();
        assert_eq!(err.code(), 1501);
    }

    #[test]
    fn invalid_id_rejected() {
        for bad in ["myplugin", "com.example..x", "COM.example.x", "com.example.-x", "com.exa mple.x"] {
            let json = base_manifest().replace("com.example.myplugin", bad);
            let err = PluginManifest::parse(&json).unwrap_err();
            assert_eq!(err.code(), 1501, "should reject id: {bad}");
        }
        // تک‌بخشی مجاز نیست ولی یک نقطه با دو بخش مجاز است
        let ok = base_manifest().replace("com.example.myplugin", "a.b");
        assert!(PluginManifest::parse(&ok).is_ok());
    }

    #[test]
    fn invalid_semver_rejected() {
        let json = base_manifest().replace("\"version\": \"1.0.0\"", "\"version\": \"1.0\"");
        assert_eq!(PluginManifest::parse(&json).unwrap_err().code(), 1501);
        let json = base_manifest().replace("\"api_version\": \"1.0.0\"", "\"api_version\": \"beta\"");
        assert_eq!(PluginManifest::parse(&json).unwrap_err().code(), 1501);
    }

    #[test]
    fn invalid_version_range_rejected() {
        let json =
            base_manifest().replace("\"api_version_range\": \"^1.0.0\"", "\"api_version_range\": \"junk\"");
        assert_eq!(PluginManifest::parse(&json).unwrap_err().code(), 1501);
    }

    #[test]
    fn unknown_capability_rejected() {
        let json = base_manifest().replace(
            "\"capabilities\": [\"trades.read\", \"stats.read\"]",
            "\"capabilities\": [\"trades.read\", \"database.direct\"]",
        );
        let err = PluginManifest::parse(&json).unwrap_err();
        assert_eq!(err.code(), 1505);
        assert_eq!(
            match err {
                PluginError::UnknownCapability { capability, .. } => capability,
                _ => String::new(),
            },
            "database.direct"
        );
    }

    #[test]
    fn resource_bounds_enforced() {
        let json = base_manifest().replace(
            "\"capabilities\": [\"trades.read\", \"stats.read\"]",
            "\"resources\": {\"max_memory_mb\": 16}",
        );
        assert_eq!(PluginManifest::parse(&json).unwrap_err().code(), 1501);
        let json = base_manifest().replace(
            "\"capabilities\": [\"trades.read\", \"stats.read\"]",
            "\"resources\": {\"max_cpu_percent\": 0}",
        );
        assert_eq!(PluginManifest::parse(&json).unwrap_err().code(), 1501);
        let json = base_manifest().replace(
            "\"capabilities\": [\"trades.read\", \"stats.read\"]",
            "\"resources\": {\"timeout_seconds\": 4000}",
        );
        assert_eq!(PluginManifest::parse(&json).unwrap_err().code(), 1501);
    }

    #[test]
    fn invalid_runtime_mode_and_trust_level_rejected() {
        let json = base_manifest().replace("out_of_process", "inline");
        assert_eq!(PluginManifest::parse(&json).unwrap_err().code(), 1501);
        let json = base_manifest().replace(
            "\"capabilities\": [\"trades.read\", \"stats.read\"]",
            "\"trust_level\": \"suspect\", \"capabilities\": []",
        );
        assert_eq!(PluginManifest::parse(&json).unwrap_err().code(), 1501);
    }

    #[test]
    fn ui_extension_points_validated() {
        let ok = base_manifest().replace(
            "\"capabilities\": [\"trades.read\", \"stats.read\"]",
            "\"ui_extension_points\": [{\"kind\": \"dashboard_widget\", \"id\": \"w1\", \"schema\": {\"type\": \"object\"}}], \"capabilities\": []",
        );
        assert!(PluginManifest::parse(&ok).is_ok());
        let bad_kind = base_manifest().replace(
            "\"capabilities\": [\"trades.read\", \"stats.read\"]",
            "\"ui_extension_points\": [{\"kind\": \"react_component\", \"id\": \"w1\", \"schema\": {}}], \"capabilities\": []",
        );
        assert_eq!(PluginManifest::parse(&bad_kind).unwrap_err().code(), 1501);
        let bad_schema = base_manifest().replace(
            "\"capabilities\": [\"trades.read\", \"stats.read\"]",
            "\"ui_extension_points\": [{\"kind\": \"report_page\", \"id\": \"r1\", \"schema\": \"raw\"}], \"capabilities\": []",
        );
        assert_eq!(PluginManifest::parse(&bad_schema).unwrap_err().code(), 1501);
    }

    #[test]
    fn in_process_trusted_requires_official_trust() {
        // community + درون‌فرایند → رد
        let json = base_manifest()
            .replace("out_of_process", "in_process_trusted")
            .replace(
                "\"capabilities\": [\"trades.read\", \"stats.read\"]",
                "\"trust_level\": \"community\", \"capabilities\": []",
            );
        let err = PluginManifest::parse(&json).unwrap_err();
        assert_eq!(err.code(), 1501);
        // official + درون‌فرایند → مجاز
        let json = base_manifest()
            .replace("out_of_process", "in_process_trusted")
            .replace(
                "\"capabilities\": [\"trades.read\", \"stats.read\"]",
                "\"trust_level\": \"official\", \"capabilities\": []",
            );
        assert!(PluginManifest::parse(&json).is_ok());
        // پیش‌فرض (بدون trust_level) با out_of_process مشکلی ندارد
        assert!(PluginManifest::parse(&base_manifest()).is_ok());
    }

    #[test]
    fn network_capability_detected() {
        let json = base_manifest().replace(
            "\"capabilities\": [\"trades.read\", \"stats.read\"]",
            "\"capabilities\": [\"network.access\"]",
        );
        let m = PluginManifest::parse(&json).unwrap();
        assert!(m.needs_network());
    }
}
