//! سیاست نسخه‌گذاری کرنل آریا — قرارداد نسخه ۱.
//!
//! - نسخه قرارداد کرنل (KERNEL_CONTRACT_VERSION): semver؛ تغییرهای ناسازگار فقط با major جدید و سیاست منسوخ‌سازی.
//! - نسخه API پلاگین (PLUGIN_API_VERSION): مستقل از کرنل؛ پلاگین‌ها بازه سازگاری اعلام می‌کنند.
//! - نسخه اسکیمای پایگاه‌داده (DATABASE_SCHEMA_VERSION): عدد صحیح افزایشی؛ فقط مهاجرت رو به جلو.
//! - نسخه فرمت بکاپ (BACKUP_FORMAT_VERSION): عدد صحیح افزایشی؛ بکاپ‌های قدیمی باید قابل بازیابی بمانند.
//! - رویدادها هرکدام event_version مستقل دارند؛ تغییر ناسازگار بار رویداد یعنی نسخه جدید همان event_type.

use semver::Version;

/// نسخه قراردادهای عمومی کرنل (سنگ بنای سازگاری).
pub const KERNEL_CONTRACT_VERSION: &str = "1.0.0";

/// نسخه API پلاگین.
pub const PLUGIN_API_VERSION: &str = "1.0.0";

/// نسخه اسکیمای فیزیکی پایگاه‌داده.
/// نسخه ۲: ایندکس یکتا روی `attachments.blake3_hash` (مهاجرت ۲).
pub const DATABASE_SCHEMA_VERSION: u32 = 2;

/// نسخه فرمت بکاپ.
pub const BACKUP_FORMAT_VERSION: u32 = 1;

/// بررسی سازگاری نسخه کرنل با بازه موردنیاز پلاگین.
pub fn is_kernel_compatible(required_range: &str) -> bool {
    let actual = Version::parse(KERNEL_CONTRACT_VERSION).expect("kernel version is valid semver");
    crate::manifest::parse_semver_req(required_range)
        .map(|req| req.matches(&actual))
        .unwrap_or(false)
}

/// بررسی سازگاری نسخه API پلاگین با بازه موردنیاز.
pub fn is_api_compatible(required_range: &str) -> bool {
    let actual = Version::parse(PLUGIN_API_VERSION).expect("plugin api version is valid semver");
    crate::manifest::parse_semver_req(required_range)
        .map(|req| req.matches(&actual))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_and_plugin_versions_are_valid_semver() {
        assert!(Version::parse(KERNEL_CONTRACT_VERSION).is_ok());
        assert!(Version::parse(PLUGIN_API_VERSION).is_ok());
    }

    #[test]
    fn schema_version_matches_migration_registry() {
        assert_eq!(DATABASE_SCHEMA_VERSION, 2);
        assert_eq!(BACKUP_FORMAT_VERSION, 1);
    }

    #[test]
    fn compatibility_checks() {
        assert!(is_kernel_compatible("^1.0.0"));
        assert!(is_kernel_compatible("^1.0"));
        assert!(!is_kernel_compatible("^2.0.0"));
        assert!(!is_kernel_compatible("garbage"));
        assert!(is_api_compatible(">=1.0.0, <2.0.0"));
        assert!(!is_api_compatible("0.9.x"));
    }
}
