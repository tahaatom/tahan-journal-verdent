//! مشتق کلید و مدیریت گذرواژه با Argon2id.
//!
//! پارامترها (سطح امنیتی نسخه ۱): m=64 MiB، t=3، p=1، خروجی 32 بایت.
//! هش تأییدکننده گذرواژه با قالب PHC رشته ذخیره می‌شود.

use crate::error::SecurityError;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params,
};

/// حافظه (KiB) — 64 MiB.
pub const KDF_M_COST_KIB: u32 = 64 * 1024;
/// تعداد گذرها.
pub const KDF_T_COST: u32 = 3;
/// موازی‌سازی.
pub const KDF_P_COST: u32 = 1;
/// طول خروجی کلید (بایت).
pub const KDF_OUTPUT_LEN: usize = 32;

/// سیاست گذرواژه نسخه ۱: حداقل ۸ کاراکتر، حداقل یک حرف و یک رقم.
pub const MIN_PASSWORD_LEN: usize = 8;

fn argon2_instance() -> Argon2<'static> {
    Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        Params::new(KDF_M_COST_KIB, KDF_T_COST, KDF_P_COST, Some(KDF_OUTPUT_LEN))
            .expect("kdf params are valid"),
    )
}

/// سیاست گذرواژه — خطای 1203 در صورت نقض.
pub fn validate_password(password: &str) -> Result<(), SecurityError> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(SecurityError::weak_password(format!(
            "at least {MIN_PASSWORD_LEN} characters required"
        )));
    }
    if !password.chars().any(char::is_alphabetic) {
        return Err(SecurityError::weak_password("at least one letter required"));
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(SecurityError::weak_password("at least one digit required"));
    }
    Ok(())
}

/// مشتق کلید 32 بایتی با Argon2id از گذرواژه و salt (برای پوش کلید گاوصندوق).
pub fn derive_key_argon2id(
    password: &str,
    salt: &[u8],
) -> Result<[u8; KDF_OUTPUT_LEN], SecurityError> {
    if salt.len() < 8 {
        return Err(SecurityError::kdf("salt too short (min 8 bytes)"));
    }
    let mut out = [0u8; KDF_OUTPUT_LEN];
    argon2_instance()
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| SecurityError::kdf(e.to_string()))?;
    Ok(out)
}

/// ساخت هش تأییدکننده گذرواژه (قالب PHC) — برای ذخیره‌سازی.
pub fn hash_password(password: &str) -> Result<String, SecurityError> {
    let salt = SaltString::generate(&mut OsRng);
    argon2_instance()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| SecurityError::kdf(e.to_string()))
}

/// تأیید گذرواژه در برابر هش PHC (مقایسه زمان-ثابت داخلی کتابخانه).
pub fn verify_password(phc: &str, password: &str) -> Result<bool, SecurityError> {
    let parsed = PasswordHash::new(phc)
        .map_err(|e| SecurityError::kdf(format!("invalid verifier format: {e}")))?;
    Ok(argon2_instance()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// ساخت salt تصادفی (16 بایت) به‌صورت hex.
pub fn random_salt_hex() -> String {
    use argon2::password_hash::rand_core::RngCore;
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    salt.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_key_is_deterministic_and_salt_sensitive() {
        let k1a = derive_key_argon2id("password1", b"0123456789abcdef").unwrap();
        let k1b = derive_key_argon2id("password1", b"0123456789abcdef").unwrap();
        let k2 = derive_key_argon2id("password1", b"fedcba9876543210").unwrap();
        let k3 = derive_key_argon2id("password2", b"0123456789abcdef").unwrap();
        assert_eq!(k1a, k1b, "same input must derive same key");
        assert_ne!(k1a, k2, "different salt must change key");
        assert_ne!(k1a, k3, "different password must change key");
        assert_eq!(k1a.len(), 32);
    }

    #[test]
    fn short_salt_rejected() {
        assert!(matches!(
            derive_key_argon2id("password1", b"short"),
            Err(SecurityError::KdfFailed { .. })
        ));
    }

    #[test]
    fn password_hash_verify_roundtrip() {
        let phc = hash_password("correct-horse-9").unwrap();
        assert!(phc.starts_with("$argon2id$"));
        assert!(verify_password(&phc, "correct-horse-9").unwrap());
        assert!(!verify_password(&phc, "wrong-password-1").unwrap());
    }

    #[test]
    fn verifier_is_salted_each_time() {
        let a = hash_password("same-password-1").unwrap();
        let b = hash_password("same-password-1").unwrap();
        assert_ne!(a, b, "two hashes of the same password must differ (salt)");
        assert!(verify_password(&a, "same-password-1").unwrap());
        assert!(verify_password(&b, "same-password-1").unwrap());
    }

    #[test]
    fn invalid_verifier_format_detected() {
        assert!(matches!(
            verify_password("not-a-phc-string", "password123"),
            Err(SecurityError::KdfFailed { .. })
        ));
    }

    #[test]
    fn password_policy_enforced() {
        assert!(matches!(validate_password("sh1"), Err(SecurityError::WeakPassword { .. }))); // کوتاه
        assert!(matches!(
            validate_password("abcdefgh"),
            Err(SecurityError::WeakPassword { .. })
        )); // بدون رقم
        assert!(matches!(
            validate_password("12345678"),
            Err(SecurityError::WeakPassword { .. })
        )); // بدون حرف
        assert!(validate_password("good-pass-1").is_ok());
        assert!(validate_password("عبور-قوی-1x").is_ok()); // یونیکد پذیرفته است
    }

    #[test]
    fn random_salt_hex_is_32_chars_and_unique() {
        let a = random_salt_hex();
        let b = random_salt_hex();
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }
}
