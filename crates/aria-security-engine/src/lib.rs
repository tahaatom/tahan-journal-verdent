//! aria-security-engine — موتور امنیت کرنل آریا
//!
//! ماژول‌ها:
//! - [`error`]: مدل خطای امنیتی با کد پایدار (بازه 1200..=1299)
//! - [`kdf`]: مشتق کلید Argon2id، هش/تأیید گذرواژه و سیاست گذرواژه
//! - [`crypto`]: رمزنگاری AES-256-GCM با AAD (فراداده احرازشده)
//! - [`vault`]: گاوصندوق کلید پروفایل — پوش/بازکردن، تغییر گذرواژه، قفل خودکار
//! - [`audit`]: پایه حسابرسی روی جدول audit_logs (پیاده‌سازی قرارداد AuditProvider)
//!
//! معماری امنیتی نسخه ۱ (باز پوش — envelope encryption):
//! 1. `setup_profile_password`: کلید اصلی گاوصندوق (32 بایت تصادفی) ساخته می‌شود.
//! 2. گذرواژه با Argon2id هش می‌شود (تأییدکننده، بدون راز اصلی).
//! 3. کلید KDF (Argon2id با salt تصادفی) از گذرواژه مشتق و کلید گاوصندوق با AES-GCM پوشیده می‌شود.
//! 4. بازکردن پروفایل = تأیید گذرواژه → مشتق KDF → بازپوشی کلید در حافظه امن (Zeroizing).
//! 5. تغییر گذرواژه فقط بازپوشی کلیدِ همان گاوصندوق است — رمزگذاری مجدد کل داده لازم ندارد.

pub mod audit;
pub mod crypto;
pub mod error;
pub mod kdf;
pub mod providers;
pub mod vault;

pub use crypto::{decrypt, encrypt};
pub use error::SecurityError;
pub use kdf::{derive_key_argon2id, hash_password, validate_password, verify_password};
pub use providers::{VaultCryptoBridge, VaultKeyBridge};
pub use vault::{ProfileVault, VaultRecord, AUTO_LOCK_DEFAULT_SECS};

#[cfg(test)]
mod tests {
    use crate::error;

    #[test]
    fn crate_smoke() {
        assert!(error::SECURITY_ERROR_RANGE.0 >= 1200);
    }
}
