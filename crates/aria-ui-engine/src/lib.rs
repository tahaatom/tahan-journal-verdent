//! aria-ui-engine — موتور رابط کاربری کرنل آریا
//!
//! رجیستری افزونه‌پذیری UI به‌صورت کاملاً اعلانی (نسخه ۱):
//! پلاگین‌ها فقط اسکیمای اعلانی ثبت می‌کنند و کرنل آن‌ها را رندر می‌کند.
//! تزریق کامپوننت React یا هر کد اجرایی ساختاراً غیرممکن است
//! (نگهبان رندر: `render.rs`، کلیدهای ممنوع + فهرست سفید انواع).

pub mod error;
pub mod registry;
pub mod render;

pub use error::UiError;
pub use registry::{ExtensionKind, UiExtension, UiExtensionRegistry};
pub use render::{allowed_types, validate_schema};

/// نسخه ساختارهای اعلانی UI.
pub const UI_ENGINE_DECL_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    #[test]
    fn decl_version_is_stable() {
        assert_eq!(super::UI_ENGINE_DECL_VERSION, 1);
        // پنج نقطه اعلانی نسخه ۱
        assert_eq!(super::ExtensionKind::all().len(), 5);
    }
}
