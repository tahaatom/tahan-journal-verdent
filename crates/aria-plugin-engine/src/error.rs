//! خطاهای موتور پلاگین — بازه پایدار ۱۵۰۰ تا ۱۵۹۹.

/// بازه کدهای خطای موتور پلاگین (فقط‌افزودنی).
pub const PLUGIN_ERROR_RANGE: (i64, i64) = (1500, 1599);

/// خطای موتور پلاگین.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PluginError {
    /// ۱۵۰۱ — مانیفست نامعتبر
    #[error("invalid manifest: {reason}")]
    ManifestInvalid { reason: String, code: i64 },

    /// ۱۵۰۲ — پلاگین یافت نشد
    #[error("plugin not found: {id}")]
    PluginNotFound { id: String, code: i64 },

    /// ۱۵۰۳ — پلاگین تکراری (شناسه از قبل ثبت شده)
    #[error("duplicate plugin: {id}")]
    DuplicatePlugin { id: String, code: i64 },

    /// ۱۵۰۴ — ناسازگاری نسخه کرنل یا API
    #[error("version incompatible: {reason}")]
    VersionIncompatible { reason: String, code: i64 },

    /// ۱۵۰۵ — قابلیت ناشناخته
    #[error("unknown capability: {capability}")]
    UnknownCapability { capability: String, code: i64 },

    /// ۱۵۰۶ — فراخوانی بدون قابلیت مجاز
    #[error("permission denied: {reason}")]
    PermissionDenied { reason: String, code: i64 },

    /// ۱۵۰۷ — گذار وضعیت نامعتبر
    #[error("invalid state transition: {reason}")]
    InvalidTransition { reason: String, code: i64 },

    /// ۱۵۰۸ — پلاگین در قرنطینه است؛ عملیات ممنوع
    #[error("plugin quarantined: {id}")]
    Quarantined { id: String, code: i64 },

    /// ۱۵۰۹ — دسترسی شبکه بدون تأیید صریح کاربر ممنوع
    #[error("network access requires explicit user approval: {id}")]
    NetworkAccessNotApproved { id: String, code: i64 },

    /// ۱۵۱۰ — خطای ذخیره‌سازی لایه پلاگین
    #[error("plugin storage error: {reason}")]
    Storage { reason: String, code: i64 },
}

impl PluginError {
    /// کد ماشین‌خوان خطا.
    pub fn code(&self) -> i64 {
        match self {
            Self::ManifestInvalid { code, .. }
            | Self::PluginNotFound { code, .. }
            | Self::DuplicatePlugin { code, .. }
            | Self::VersionIncompatible { code, .. }
            | Self::UnknownCapability { code, .. }
            | Self::PermissionDenied { code, .. }
            | Self::InvalidTransition { code, .. }
            | Self::Quarantined { code, .. }
            | Self::NetworkAccessNotApproved { code, .. }
            | Self::Storage { code, .. } => *code,
        }
    }

    pub(crate) fn manifest_invalid(reason: impl Into<String>) -> Self {
        Self::ManifestInvalid { reason: reason.into(), code: 1501 }
    }

    pub(crate) fn plugin_not_found(id: &str) -> Self {
        Self::PluginNotFound { id: id.to_string(), code: 1502 }
    }

    pub(crate) fn duplicate(id: &str) -> Self {
        Self::DuplicatePlugin { id: id.to_string(), code: 1503 }
    }

    pub(crate) fn incompatible(reason: impl Into<String>) -> Self {
        Self::VersionIncompatible { reason: reason.into(), code: 1504 }
    }

    pub(crate) fn unknown_capability(cap: &str) -> Self {
        Self::UnknownCapability { capability: cap.to_string(), code: 1505 }
    }

    pub(crate) fn permission_denied(reason: impl Into<String>) -> Self {
        Self::PermissionDenied { reason: reason.into(), code: 1506 }
    }

    pub(crate) fn invalid_transition(reason: impl Into<String>) -> Self {
        Self::InvalidTransition { reason: reason.into(), code: 1507 }
    }

    pub(crate) fn quarantined(id: &str) -> Self {
        Self::Quarantined { id: id.to_string(), code: 1508 }
    }

    pub(crate) fn network_not_approved(id: &str) -> Self {
        Self::NetworkAccessNotApproved { id: id.to_string(), code: 1509 }
    }

    pub(crate) fn storage(reason: impl Into<String>) -> Self {
        Self::Storage { reason: reason.into(), code: 1510 }
    }
}

impl From<rusqlite::Error> for PluginError {
    fn from(e: rusqlite::Error) -> Self {
        Self::storage(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable_and_in_range() {
        assert_eq!(PLUGIN_ERROR_RANGE, (1500, 1599));
        assert_eq!(PluginError::manifest_invalid("x").code(), 1501);
        assert_eq!(PluginError::plugin_not_found("p").code(), 1502);
        assert_eq!(PluginError::duplicate("p").code(), 1503);
        assert_eq!(PluginError::incompatible("x").code(), 1504);
        assert_eq!(PluginError::unknown_capability("c").code(), 1505);
        assert_eq!(PluginError::permission_denied("x").code(), 1506);
        assert_eq!(PluginError::invalid_transition("x").code(), 1507);
        assert_eq!(PluginError::quarantined("p").code(), 1508);
        assert_eq!(PluginError::network_not_approved("p").code(), 1509);
        assert_eq!(PluginError::storage("x").code(), 1510);
    }

    #[test]
    fn from_rusqlite_maps_to_storage() {
        let e: PluginError =
            rusqlite::Error::InvalidParameterName("bad".into()).into();
        assert_eq!(e.code(), 1510);
    }
}
