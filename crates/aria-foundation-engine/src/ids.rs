//! شناسه‌های نوع‌دار موجودیت‌ها.
//!
//! همه شناسه‌ها uuid v4 هستند اما به‌صورت نوع‌دار تا اشتباه‌کردن شناسه‌های
//! موجودیت‌های مختلف در زمان کامپایل ناممکن شود.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_typed_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// ساخت شناسه جدید (uuid v4).
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// ساخت از یک uuid موجود (برای خواندن از پایگاه‌داده).
            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// uuid زیربنایی.
            pub fn as_uuid(&self) -> Uuid {
                self.0
            }

            /// صفر (nil uuid) — فقط برای مقایسه/مقدار پیش‌فرض ساختاری.
            pub fn nil() -> Self {
                Self(Uuid::nil())
            }

            /// آیا شناسه صفر است.
            pub fn is_nil(&self) -> bool {
                self.0.is_nil()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(u: Uuid) -> Self {
                Self(u)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

define_typed_id!(
    /// شناسه پروفایل کاربری
    ProfileId
);
define_typed_id!(
    /// شناسه حساب معاملاتی
    AccountId
);
define_typed_id!(
    /// شناسه نماد معاملاتی
    SymbolId
);
define_typed_id!(
    /// شناسه ژورنال معامله
    TradeId
);
define_typed_id!(
    /// شناسه پا (Entry Leg / Exit Leg)
    LegId
);
define_typed_id!(
    /// شناسه اجرا (فیل واقعی بروکر یا دستی)
    ExecutionId
);
define_typed_id!(
    /// شناسه رکورد خام منبع (داده واردشده)
    SourceRecordId
);
define_typed_id!(
    /// شناسه فیلد سفارشی
    FieldId
);
define_typed_id!(
    /// شناسه پیوست
    AttachmentId
);
define_typed_id!(
    /// شناسه پلاگین
    PluginId
);
define_typed_id!(
    /// شناسه رویداد
    EventId
);
define_typed_id!(
    /// شناسه دستور
    CommandId
);
define_typed_id!(
    /// شناسه گروه پوزیشن (رزرو نسخه ۱)
    PositionGroupId
);
define_typed_id!(
    /// شناسه رکورد بازنویسی دستی
    OverrideId
);

/// شناسه پای ورود (نام قراردادی سند نسخه ۵).
pub type EntryLegId = LegId;
/// شناسه پای خروج (نام قراردادی سند نسخه ۵).
pub type ExitLegId = LegId;

/// انواع پا برای تمایز Entry از Exit در سطح نوع.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegKind {
    /// پای ورود
    Entry,
    /// پای خروج
    Exit,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn id_generation_uniqueness() {
        let mut seen = HashSet::new();
        for _ in 0..10_000 {
            let id = TradeId::new();
            assert!(seen.insert(id));
        }
    }

    #[test]
    fn different_id_types_are_not_confusable() {
        // نوع‌داری: مقایسه مستقیم بین دو نوع شناسه کامپایل نمی‌شود؛
        // این تست فقط وجود سازنده‌ها و تفکیک نمایش را تضمین می‌کند.
        let t = TradeId::new();
        let a = AccountId::new();
        assert_ne!(t.as_uuid(), a.as_uuid());
    }

    #[test]
    fn serde_roundtrip() {
        let id = TradeId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: TradeId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn nil_detection() {
        let nil = TradeId::nil();
        assert!(nil.is_nil());
        assert!(!TradeId::new().is_nil());
    }

    #[test]
    fn uuid_conversion_roundtrip() {
        let u = Uuid::new_v4();
        let id: TradeId = u.into();
        assert_eq!(Uuid::from(id), u);
    }
}
