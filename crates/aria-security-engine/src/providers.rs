//! پل‌های قراردادی موتور امنیت — پیاده‌سازی `KeyProvider` و `EncryptionProvider`
//! از `aria_contracts::traits` روی گاوصندوق پروفایل.
//!
//! این پل‌ها نقطه اتصال هسته/پل RPC به موتور امنیت هستند: کلید فعال فقط داخل
//! موتور می‌ماند و بیرون‌دهی مستقیم کلید در قرارداد نیست.

use aria_contracts::traits::{EncryptionProvider, KeyProvider};
use aria_contracts::ContractError;
use aria_storage_engine::Database;

use crate::vault::ProfileVault;

/// تبدیل خطای امنیتی به خطای قرارداد (بدون افشای جزئیات راز).
fn contract_err(e: crate::SecurityError) -> ContractError {
    ContractError::Violation { reason: e.to_string() }
}

/// پل `KeyProvider` روی گاوصندوق پروفایل.
pub struct VaultKeyBridge<'a> {
    pub vault: &'a ProfileVault,
    pub db: &'a Database,
}

impl KeyProvider for VaultKeyBridge<'_> {
    fn unlock_profile(&self, profile_id: &str, password: &str) -> Result<(), ContractError> {
        self.vault
            .unlock_profile(self.db, profile_id, password)
            .map(|_| ())
            .map_err(contract_err)
    }

    fn lock_profile(&self, profile_id: &str) -> Result<(), ContractError> {
        if self.vault.lock_profile(profile_id) {
            Ok(())
        } else {
            Err(ContractError::Violation {
                reason: format!("profile not unlocked: {profile_id}"),
            })
        }
    }

    fn lock_all(&self) {
        self.vault.lock_all();
    }

    fn is_unlocked(&self, profile_id: &str) -> bool {
        self.vault.is_unlocked(profile_id)
    }
}

/// پل `EncryptionProvider` روی کلید فعال گاوصندوق.
///
/// خطای «پروفایل باز نیست» (۱۲۰۶) مستقیم به Violation تبدیل می‌شود؛ کلید هرگز
/// از این پل خارج نمی‌شود.
pub struct VaultCryptoBridge<'a> {
    pub vault: &'a ProfileVault,
}

impl EncryptionProvider for VaultCryptoBridge<'_> {
    fn encrypt(
        &self,
        profile_id: &str,
        plaintext: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, ContractError> {
        let key = self.vault.get_key(profile_id).map_err(contract_err)?;
        crate::crypto::encrypt(&key, plaintext, aad).map_err(contract_err)
    }

    fn decrypt(
        &self,
        profile_id: &str,
        ciphertext: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, ContractError> {
        let key = self.vault.get_key(profile_id).map_err(contract_err)?;
        crate::crypto::decrypt(&key, ciphertext, aad).map_err(contract_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit;
    use crate::vault::ProfileVault;
    use aria_storage_engine::Database;
    fn db() -> Database {
        let db = Database::open_memory(Some("test")).expect("memory db");
        {
            let mut conn = db.lock();
            aria_storage_engine::migrations::run_migrations(&mut conn, |_| Ok(())).unwrap();
        }
        db
    }

    fn setup_vault(db: &Database) -> ProfileVault {
        let vault = ProfileVault::new(crate::vault::AUTO_LOCK_DEFAULT_SECS);
        vault.setup_profile_password(db, "p1", "Correct-Horse-42").unwrap();
        vault
    }

    #[test]
    fn key_bridge_unlock_lock_roundtrip() {
        let db = db();
        let vault = setup_vault(&db);
        let bridge = VaultKeyBridge { vault: &vault, db: &db };
        bridge.lock_profile("p1").unwrap();
        assert!(!bridge.is_unlocked("p1"));
        bridge.unlock_profile("p1", "Correct-Horse-42").unwrap();
        assert!(bridge.is_unlocked("p1"));
        bridge.lock_all();
        assert!(!bridge.is_unlocked("p1"));
    }

    #[test]
    fn crypto_bridge_roundtrip_and_tamper() {
        let db = db();
        let vault = setup_vault(&db);
        let crypto_bridge = VaultCryptoBridge { vault: &vault };
        let ct = crypto_bridge.encrypt("p1", b"secret", b"aad").unwrap();
        let pt = crypto_bridge.decrypt("p1", &ct, b"aad").unwrap();
        assert_eq!(pt, b"secret");
        // دستکاری AAD → شکست رمزگشایی
        assert!(crypto_bridge.decrypt("p1", &ct, b"other").is_err());
    }

    #[test]
    fn crypto_bridge_requires_unlocked_profile() {
        let db = db();
        let vault = setup_vault(&db);
        vault.lock_profile("p1");
        let crypto_bridge = VaultCryptoBridge { vault: &vault };
        let err = crypto_bridge.encrypt("p1", b"x", b"aad").unwrap_err();
        assert!(matches!(err, ContractError::Violation { .. }));
    }

    #[test]
    fn failed_unlocks_are_audited_for_rate_limit() {
        let db = db();
        let vault = setup_vault(&db);
        let bridge = VaultKeyBridge { vault: &vault, db: &db };
        // ۵ تلاش ناموفق → شمارش حسابرسی باید ۵ باشد
        for _ in 0..crate::vault::UNLOCK_RATE_LIMIT_ATTEMPTS {
            assert!(bridge.unlock_profile("p1", "wrong-pass").is_err());
        }
        let window_start = crate::vault::iso_from_epoch(
            crate::vault::now_epoch() - crate::vault::UNLOCK_RATE_LIMIT_WINDOW_SECS,
        );
        assert_eq!(
            audit::count_action_since(&db, "vault.unlock_failed", &window_start).unwrap(),
            crate::vault::UNLOCK_RATE_LIMIT_ATTEMPTS
        );
        // تلاش بعدی باید ۱۲۱۱ بدهد نه wrong_password
        let err = vault.unlock_profile(&db, "p1", "Correct-Horse-42").unwrap_err();
        assert_eq!(err.code(), 1211);
    }
}
