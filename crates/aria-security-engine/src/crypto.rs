//! رمزنگاری AES-256-GCM با فراداده احرازشده (AAD).
//!
//! قالب بایت خروجی: `nonce(12) || ciphertext+tag`
//! — دستکاری هر بایتی (شامل AAD) با شکست رمزگشایی تشخیص داده می‌شود.

use crate::error::SecurityError;
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use aes_gcm::aead::OsRng;
use aes_gcm::aead::rand_core::RngCore;

/// طول nonce به بایت (استاندارد GCM).
pub const NONCE_LEN: usize = 12;
/// طول کلید به بایت (AES-256).
pub const KEY_LEN: usize = 32;

/// رمزنگاری با کلید 32 بایتی و AAD اختیاری.
pub fn encrypt(key: &[u8; KEY_LEN], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, SecurityError> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| SecurityError::encryption(e.to_string()))?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let payload = Payload { msg: plaintext, aad };
    let ct = cipher
        .encrypt(nonce, payload)
        .map_err(|_| SecurityError::encryption("aes-gcm encrypt failed"))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// رمزگشایی — دستکاری داده یا AAD یا کلید نادرست → خطای 1201.
pub fn decrypt(key: &[u8; KEY_LEN], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>, SecurityError> {
    if ciphertext.len() <= NONCE_LEN {
        return Err(SecurityError::decryption("ciphertext too short"));
    }
    let (nonce_bytes, body) = ciphertext.split_at(NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| SecurityError::decryption(e.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let payload = Payload { msg: body, aad };
    cipher
        .decrypt(nonce, payload)
        .map_err(|_| SecurityError::decryption("aes-gcm authentication failed"))
}

/// رمزنگاری رشته (کمک ساده) — خروجی nonce+ct به‌صورت بایت.
pub fn encrypt_string(key: &[u8; KEY_LEN], plaintext: &str, aad: &[u8]) -> Result<Vec<u8>, SecurityError> {
    encrypt(key, plaintext.as_bytes(), aad)
}

/// رمزگشایی رشته UTF-8 (کمک ساده).
pub fn decrypt_string(key: &[u8; KEY_LEN], ciphertext: &[u8], aad: &[u8]) -> Result<String, SecurityError> {
    let bytes = decrypt(key, ciphertext, aad)?;
    String::from_utf8(bytes).map_err(|_| SecurityError::decryption("decrypted data is not utf-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; KEY_LEN] {
        let mut k = [0u8; KEY_LEN];
        OsRng.fill_bytes(&mut k);
        k
    }

    #[test]
    fn roundtrip_with_aad() {
        let key = test_key();
        let plaintext = b"secret data 1234567890";
        let aad = b"profile:tahan";

        let ct = encrypt(&key, plaintext, aad).unwrap();
        assert!(ct.len() > NONCE_LEN);
        assert_ne!(&ct[..], plaintext);
        assert_ne!(&ct[NONCE_LEN..NONCE_LEN + 5], &plaintext[..5]); // پیکربندی رمز

        let pt = decrypt(&key, &ct, aad).unwrap();
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn tampered_ciphertext_rejected() {
        let key = test_key();
        let ct = encrypt(&key, b"hello secret 123", b"aad-1").unwrap();
        let mut tampered = ct.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(matches!(
            decrypt(&key, &tampered, b"aad-1"),
            Err(SecurityError::DecryptionFailed { .. })
        ));
    }

    #[test]
    fn tampered_nonce_rejected() {
        let key = test_key();
        let ct = encrypt(&key, b"hello secret 123", b"aad-1").unwrap();
        let mut tampered = ct.clone();
        tampered[3] ^= 0xFF;
        assert!(matches!(
            decrypt(&key, &tampered, b"aad-1"),
            Err(SecurityError::DecryptionFailed { .. })
        ));
    }

    #[test]
    fn wrong_aad_rejected() {
        let key = test_key();
        let ct = encrypt(&key, b"payload-123456789", b"aad-A").unwrap();
        assert!(matches!(
            decrypt(&key, &ct, b"aad-B"),
            Err(SecurityError::DecryptionFailed { .. })
        ));
    }

    #[test]
    fn wrong_key_rejected() {
        let key1 = test_key();
        let key2 = test_key();
        let ct = encrypt(&key1, b"payload-123456789", b"aad").unwrap();
        assert!(matches!(
            decrypt(&key2, &ct, b"aad"),
            Err(SecurityError::DecryptionFailed { .. })
        ));
    }

    #[test]
    fn empty_plaintext_and_empty_aad_ok() {
        let key = test_key();
        let ct = encrypt(&key, b"", b"").unwrap();
        assert_eq!(ct.len(), NONCE_LEN + 16); // tag GCM همیشه 16 بایت
        assert_eq!(decrypt(&key, &ct, b"").unwrap(), b"");
    }

    #[test]
    fn too_short_ciphertext_rejected() {
        let key = test_key();
        assert!(matches!(
            decrypt(&key, &[1u8, 2, 3], b""),
            Err(SecurityError::DecryptionFailed { .. })
        ));
    }

    #[test]
    fn string_helpers_roundtrip() {
        let key = test_key();
        let ct = encrypt_string(&key, "سلام ژورنال ۱۲۳", b"aad").unwrap();
        let pt = decrypt_string(&key, &ct, b"aad").unwrap();
        assert_eq!(pt, "سلام ژورنال ۱۲۳");
    }

    #[test]
    fn unique_nonce_per_call() {
        let key = test_key();
        let c1 = encrypt(&key, b"same", b"").unwrap();
        let c2 = encrypt(&key, b"same", b"").unwrap();
        assert_ne!(c1, c2, "fresh nonce per encryption is mandatory");
    }
}
