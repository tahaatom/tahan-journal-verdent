//! قابلیت‌ها و مجوزها — فهرست بسته نسخه ۱ با اجرای سخت‌گیرانه.
//!
//! `network.access` به‌صورت پیش‌فرض خاموش است و فقط با تأیید صریح کاربر فعال می‌شود
//! (قرارداد: `docs/contracts/plugin-api.md`).

use serde::{Deserialize, Serialize};

/// هر ۱۷ قابلیت نسخه ۱ (ثابت‌های پایدار).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    TradesRead,
    TradesWrite,
    TradesDelete,
    FieldsRead,
    FieldsDefine,
    AttachmentsRead,
    AttachmentsWrite,
    StatsRead,
    UiWidget,
    UiPage,
    NotificationsShow,
    BackupCreate,
    BackupRestore,
    MtImport,
    MtLive,
    InsightsWrite,
    NetworkAccess,
}

impl Capability {
    /// نمایش رشته‌ای مطابق مانیفست.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TradesRead => "trades.read",
            Self::TradesWrite => "trades.write",
            Self::TradesDelete => "trades.delete",
            Self::FieldsRead => "fields.read",
            Self::FieldsDefine => "fields.define",
            Self::AttachmentsRead => "attachments.read",
            Self::AttachmentsWrite => "attachments.write",
            Self::StatsRead => "stats.read",
            Self::UiWidget => "ui.widget",
            Self::UiPage => "ui.page",
            Self::NotificationsShow => "notifications.show",
            Self::BackupCreate => "backup.create",
            Self::BackupRestore => "backup.restore",
            Self::MtImport => "mt.import",
            Self::MtLive => "mt.live",
            Self::InsightsWrite => "insights.write",
            Self::NetworkAccess => "network.access",
        }
    }

    /// تجزیه از رشته مانیفست — فقط مقادیر فهرست بسته.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "trades.read" => Self::TradesRead,
            "trades.write" => Self::TradesWrite,
            "trades.delete" => Self::TradesDelete,
            "fields.read" => Self::FieldsRead,
            "fields.define" => Self::FieldsDefine,
            "attachments.read" => Self::AttachmentsRead,
            "attachments.write" => Self::AttachmentsWrite,
            "stats.read" => Self::StatsRead,
            "ui.widget" => Self::UiWidget,
            "ui.page" => Self::UiPage,
            "notifications.show" => Self::NotificationsShow,
            "backup.create" => Self::BackupCreate,
            "backup.restore" => Self::BackupRestore,
            "mt.import" => Self::MtImport,
            "mt.live" => Self::MtLive,
            "insights.write" => Self::InsightsWrite,
            "network.access" => Self::NetworkAccess,
            _ => return None,
        })
    }

    /// آیا این قابلیت نیازمند تأیید صریح کاربر است؟
    pub fn needs_user_approval(&self) -> bool {
        matches!(self, Self::NetworkAccess)
    }
}

/// مجموعه قابلیت‌های اعطاشده به یک پلاگین.
#[derive(Debug, Clone, Default)]
pub struct PermissionSet {
    granted: Vec<Capability>,
}

impl PermissionSet {
    /// ساخت از قابلیت‌های مانیفست (قبلاً اعتبارسنجی‌شده).
    pub fn from_capabilities(caps: &[Capability]) -> Self {
        let mut granted = caps.to_vec();
        granted.sort_by_key(|c| c.as_str());
        granted.dedup_by_key(|c| c.as_str());
        Self { granted }
    }

    /// آیا قابلیت اعطا شده؟
    pub fn has(&self, cap: Capability) -> bool {
        self.granted.contains(&cap)
    }

    /// فهرست قابلیت‌ها (برای نمایش به کاربر).
    pub fn list(&self) -> &[Capability] {
        &self.granted
    }

    /// اجرای مجوز — بدون قابلیت، خطای ۱۵۰۶.
    pub fn require(&self, cap: Capability) -> Result<(), crate::error::PluginError> {
        if self.has(cap) {
            Ok(())
        } else {
            Err(crate::error::PluginError::permission_denied(format!(
                "capability not granted: {}",
                cap.as_str()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_roundtrip_all_17() {
        let all = [
            Capability::TradesRead,
            Capability::TradesWrite,
            Capability::TradesDelete,
            Capability::FieldsRead,
            Capability::FieldsDefine,
            Capability::AttachmentsRead,
            Capability::AttachmentsWrite,
            Capability::StatsRead,
            Capability::UiWidget,
            Capability::UiPage,
            Capability::NotificationsShow,
            Capability::BackupCreate,
            Capability::BackupRestore,
            Capability::MtImport,
            Capability::MtLive,
            Capability::InsightsWrite,
            Capability::NetworkAccess,
        ];
        assert_eq!(all.len(), 17);
        for cap in all {
            assert_eq!(Capability::parse(cap.as_str()), Some(cap));
        }
        assert_eq!(Capability::parse("root.access"), None);
    }

    #[test]
    fn require_enforces_granted_and_denied() {
        let perms = PermissionSet::from_capabilities(&[Capability::TradesRead, Capability::StatsRead]);
        assert!(perms.require(Capability::TradesRead).is_ok());
        assert!(perms.require(Capability::StatsRead).is_ok());
        let err = perms.require(Capability::TradesDelete).unwrap_err();
        assert_eq!(err.code(), 1506);
        assert_eq!(perms.list().len(), 2);
    }

    #[test]
    fn network_access_is_the_only_approval_required_capability() {
        for cap in [
            Capability::TradesRead,
            Capability::BackupRestore,
            Capability::MtLive,
        ] {
            assert!(!cap.needs_user_approval());
        }
        assert!(Capability::NetworkAccess.needs_user_approval());
    }

    #[test]
    fn duplicates_deduped() {
        let perms =
            PermissionSet::from_capabilities(&[Capability::StatsRead, Capability::StatsRead]);
        assert_eq!(perms.list().len(), 1);
    }
}
