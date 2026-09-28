//! aria-plugin-engine — موتور پلاگین کرنل آریا
//!
//! - اعتبارسنجی مانیفست (`manifest.rs`) مطابق `plugin-manifest.schema.json`
//! - نسخه‌گذاری معنایی و بازه‌های سازگاری (`semver.rs`)
//! - قابلیت‌ها و مجوزها با network.access پیش‌فرض-خاموش (`permissions.rs`)
//! - رجیستری، چرخه حیات، قرنطینه و سلامت (`service.rs`)
//! - رویدادهای پلاگین با outbox تراکنشی (`events.rs`)
//!
//! محدوده نسخه ۱: هیچ پلاگینی دسترسی مستقیم به پایگاه‌داده ندارد؛
//! اجرای فرایند جانبی (sidecar) در فاز ۱.۸ (موتور اجرا) پیاده‌سازی می‌شود.

pub mod error;
pub mod events;
pub mod manifest;
pub mod permissions;
pub mod semver;
pub mod service;

pub use error::{PluginError, PLUGIN_ERROR_RANGE};
pub use manifest::{PluginManifest, ResourcePolicy};
pub use permissions::Capability;
pub use service::{PluginHealth, PluginService, PluginStatus};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_smoke() {
        assert_eq!(crate::PLUGIN_ERROR_RANGE, (1500, 1599));
    }
}
