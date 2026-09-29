//! گاوصندوق کلید پروفایل — قلب موتور امنیت.
//!
//! - کلید گاوصندوق هر پروفایل: 32 بایت تصادفی (کلید اصلی داده).
//! - گذرواژه فقط برای «پوش/بازپوشی» کلید استفاده می‌شود (envelope encryption).
//! - کلید بازشده فقط در حافظه امن (`Zeroizing`) نگه داشته می‌شود.
//! - قفل خودکار: پس از مهلت بی‌فعالیتی همه پروفایل‌ها قفل می‌شوند.

use crate::crypto;
use crate::error::SecurityError;
use crate::kdf;
use aes_gcm::aead::OsRng;
use aes_gcm::aead::rand_core::RngCore;
use aria_storage_engine::Database;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Mutex;
use zeroize::Zeroizing;

/// پیش‌فرض قفل خودکار: ۹۰۰ ثانیه (۱۵ دقیقه) — همگام با پیکربندی پایه.
pub const AUTO_LOCK_DEFAULT_SECS: u64 = 15 * 60;

/// حداکثر تلاش ناموفق بازکردن پروفایل در پنجره زمانی (محدودسازی نرخ ورود).
pub const UNLOCK_RATE_LIMIT_ATTEMPTS: i64 = 5;
/// پنجره محدودسازی نرخ (ثانیه) — پس از آن شمارش شکست‌ها منقضی می‌شود.
pub const UNLOCK_RATE_LIMIT_WINDOW_SECS: i64 = 15 * 60;

/// زمان ISO از epoch برای مرز پنجره نرخ.
pub fn iso_from_epoch(secs: i64) -> String {
    chrono::DateTime::from_timestamp(secs, 0)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).expect("epoch"))
        .to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

/// رکورد گاوصندوق ذخیره‌شده در جدول settings.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VaultRecord {
    /// هش PHC تأییدکننده گذرواژه (argon2id)
    pub verifier: String,
    /// salt hex برای مشتق کلید KDF از گذرواژه
    pub kdf_salt_hex: String,
    /// کلید گاوصندوق پوشیده‌شده (nonce+ct، قالب base64)
    pub wrapped_key_b64: String,
    /// نسخه رکورد گاوصندوق
    pub record_version: u32,
}

/// نسخه رکورد گاوصندوق.
pub const VAULT_RECORD_VERSION: u32 = 1;

/// کلید گاوصندوق در حافظه امن.
pub type VaultKey = Zeroizing<[u8; crypto::KEY_LEN]>;

fn settings_key(profile_id: &str) -> String {
    format!("vault_record:{profile_id}")
}

fn random_vault_key() -> VaultKey {
    let mut k = [0u8; crypto::KEY_LEN];
    OsRng.fill_bytes(&mut k);
    Zeroizing::new(k)
}

fn b64_encode(data: &[u8]) -> String {
    // base64 استاندارد بدون وابستگی جدید (کدگذاری دستی)
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18 & 63) as usize] as char);
        out.push(TABLE[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn b64_decode(s: &str) -> Result<Vec<u8>, SecurityError> {
    fn val(c: u8) -> Result<u32, SecurityError> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(SecurityError::malformed_vault_record("invalid base64 character")),
        }
    }
    let bytes = s.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(SecurityError::malformed_vault_record("base64 length not multiple of 4"));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let pad = chunk.iter().filter(|&&c| c == b'=').count();
        if pad > 2 {
            return Err(SecurityError::malformed_vault_record("invalid base64 padding"));
        }
        // '=' فقط در انتها مجاز است
        for (i, &c) in chunk.iter().enumerate() {
            if c == b'=' && i < 4 - pad {
                return Err(SecurityError::malformed_vault_record("invalid base64 padding position"));
            }
        }
        let v = |c: u8| -> Result<u32, SecurityError> {
            if c == b'=' { Ok(0) } else { val(c) }
        };
        let n = (v(chunk[0])? << 18)
            | (v(chunk[1])? << 12)
            | (v(chunk[2])? << 6)
            | v(chunk[3])?;
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

/// زمان فعلی به ثانیه epoch.
pub(crate) fn now_epoch() -> i64 {
    chrono::Utc::now().timestamp()
}

fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, SecurityError> {
    if !hex.len().is_multiple_of(2) {
        return Err(SecurityError::malformed_vault_record("hex length not even"));
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| SecurityError::malformed_vault_record("invalid hex"))
        })
        .collect()
}

/// گاوصندوق پروفایل‌ها + قفل خودکار.
pub struct ProfileVault {
    unlocked: Mutex<HashMap<String, VaultKey>>,
    last_activity: AtomicI64,
    auto_lock_timeout_secs: AtomicU64,
}

impl Default for ProfileVault {
    fn default() -> Self {
        Self::new(AUTO_LOCK_DEFAULT_SECS)
    }
}

impl ProfileVault {
    /// ساخت گاوصندوق با مهلت قفل خودکار مشخص (ثانیه).
    pub fn new(auto_lock_timeout_secs: u64) -> Self {
        Self {
            unlocked: Mutex::new(HashMap::new()),
            last_activity: AtomicI64::new(now_epoch()),
            auto_lock_timeout_secs: AtomicU64::new(auto_lock_timeout_secs.max(1)),
        }
    }

    // ---------------- ذخیره‌سازی رکورد گاوصندوق ----------------

    /// خواندن رکورد گاوصندوق پروفایل از پایگاه‌داده.
    pub fn read_record(db: &Database, profile_id: &str) -> Result<VaultRecord, SecurityError> {
        let conn = db.lock();
        let value: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                [settings_key(profile_id)],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })
            .map_err(|e| SecurityError::audit(e.to_string()))?;
        let raw = value.ok_or_else(|| SecurityError::vault_not_found(profile_id))?;
        serde_json::from_str(&raw)
            .map_err(|e| SecurityError::malformed_vault_record(e.to_string()))
    }

    fn write_record(db: &Database, profile_id: &str, record: &VaultRecord) -> Result<(), SecurityError> {
        let json = serde_json::to_string(record)
            .map_err(|e| SecurityError::audit(e.to_string()))?;
        db.lock()
            .execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
                rusqlite::params![settings_key(profile_id), json, crate::audit::now_iso()],
            )
            .map_err(|e| SecurityError::audit(e.to_string()))?;
        Ok(())
    }

    // ---------------- چرخه حیات گذرواژه ----------------

    /// ثبت رویداد حسابرسی — شکست حسابرسی هرگز عمل اصلی را متوقف نمی‌کند.
    fn audit(
        db: &Database,
        action: &str,
        target: &str,
        detail: Option<&serde_json::Value>,
    ) {
        if let Err(e) = crate::audit::record_event(db, action, "kernel:security_engine", Some(target), detail) {
            tracing::warn!(action = action, error = %e, "audit write failed");
        }
    }

    /// راه‌اندازی اولیه گاوصندوق پروفایل با گذرواژه جدید.
    ///
    /// کلید گاوصندوق تصادفی ساخته، با KDF گذرواژه پوشیده و ذخیره می‌شود.
    /// کلید بازگشتی برای رمزگذاری/بازکردن پایگاه‌داده استفاده می‌شود.
    pub fn setup_profile_password(
        &self,
        db: &Database,
        profile_id: &str,
        password: &str,
    ) -> Result<VaultKey, SecurityError> {
        kdf::validate_password(password)?;
        if Self::read_record(db, profile_id).is_ok() {
            return Err(SecurityError::vault_already_initialized(profile_id));
        }
        let key = random_vault_key();
        self.persist_wrapped(db, profile_id, password, &key)?;
        self.unlocked
            .lock()
            .expect("vault mutex")
            .insert(profile_id.to_string(), key.clone());
        self.touch();
        Self::audit(db, "vault.setup", profile_id, None);
        tracing::info!(profile = profile_id, "vault initialized");
        Ok(key)
    }

    /// بازکردن پروفایل با گذرواژه — کلید در حافظه امن نگه داشته می‌شود.
    ///
    /// محدودسازی نرخ: اگر در پنجره زمانی مجاز، تعداد تلاش‌های ناموفق از حد
    /// بگذرد، بازکردن تا انقضای پنجره با خطای ۱۲۱۱ رد می‌شود.
    pub fn unlock_profile(
        &self,
        db: &Database,
        profile_id: &str,
        password: &str,
    ) -> Result<VaultKey, SecurityError> {
        // محدودسازی نرخ تلاش ورود (ذخیره‌شده محلی در حسابرسی)
        let window_start = iso_from_epoch(now_epoch() - UNLOCK_RATE_LIMIT_WINDOW_SECS);
        let failures = crate::audit::count_action_since(db, "vault.unlock_failed", &window_start)?;
        if failures >= UNLOCK_RATE_LIMIT_ATTEMPTS {
            tracing::warn!(profile = profile_id, failures = failures, "unlock rate limited");
            return Err(SecurityError::rate_limited(profile_id));
        }

        let record = Self::read_record(db, profile_id)?;
        let ok = kdf::verify_password(&record.verifier, password)?;
        if !ok {
            Self::audit(db, "vault.unlock_failed", profile_id, None);
            return Err(SecurityError::wrong_password());
        }
        let key = self.unwrap_key(profile_id, &record, password)?;
        self.unlocked
            .lock()
            .expect("vault mutex")
            .insert(profile_id.to_string(), key.clone());
        self.touch();
        Self::audit(db, "vault.unlock", profile_id, None);
        tracing::info!(profile = profile_id, "profile unlocked");
        Ok(key)
    }

    /// قفل پروفایل — حذف کلید از حافظه (Zeroizing هنگام drop صفر می‌شود).
    pub fn lock_profile(&self, profile_id: &str) -> bool {
        let removed = self
            .unlocked
            .lock()
            .expect("vault mutex")
            .remove(profile_id)
            .is_some();
        if removed {
            tracing::info!(profile = profile_id, "profile locked");
        }
        removed
    }

    /// قفل همه پروفایل‌ها.
    pub fn lock_all(&self) -> usize {
        let mut map = self.unlocked.lock().expect("vault mutex");
        let n = map.len();
        map.clear();
        if n > 0 {
            tracing::info!(count = n, "all profiles locked");
        }
        n
    }

    /// آیا پروفایل باز است.
    pub fn is_unlocked(&self, profile_id: &str) -> bool {
        self.unlocked.lock().expect("vault mutex").contains_key(profile_id)
    }

    /// دریافت کلید بازشده (کلون) — خطای 1205 اگر قفل باشد.
    pub fn get_key(&self, profile_id: &str) -> Result<VaultKey, SecurityError> {
        self.unlocked
            .lock()
            .expect("vault mutex")
            .get(profile_id)
            .cloned()
            .ok_or_else(|| SecurityError::profile_locked(profile_id))
    }

    /// تغییر گذرواژه: تأیید گذرواژه فعلی → بازپوشی همان کلید با گذرواژه جدید.
    pub fn change_password(
        &self,
        db: &Database,
        profile_id: &str,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), SecurityError> {
        kdf::validate_password(new_password)?;
        let record = Self::read_record(db, profile_id)?;
        if !kdf::verify_password(&record.verifier, current_password)? {
            Self::audit(db, "vault.change_failed", profile_id, None);
            return Err(SecurityError::wrong_password());
        }
        let key = self.unwrap_key(profile_id, &record, current_password)?;
        self.persist_wrapped(db, profile_id, new_password, &key)?;
        self.touch();
        Self::audit(db, "vault.password_changed", profile_id, None);
        tracing::info!(profile = profile_id, "password changed");
        Ok(())
    }

    // ---------------- قفل خودکار ----------------

    /// ثبت فعالیت کاربر (هر تعامل UI باید این را صدا بزند).
    pub fn touch(&self) {
        self.last_activity.store(now_epoch(), Ordering::SeqCst);
    }

    /// تنظیم مهلت قفل خودکار (ثانیه، حداقل ۱).
    pub fn set_auto_lock_timeout(&self, secs: u64) {
        self.auto_lock_timeout_secs.store(secs.max(1), Ordering::SeqCst);
    }

    /// مهلت فعلی قفل خودکار.
    pub fn auto_lock_timeout(&self) -> u64 {
        self.auto_lock_timeout_secs.load(Ordering::SeqCst)
    }

    /// بررسی قفل خودکار — اگر مهلت بی‌فعالیتی گذشته و پروفایلی باز باشد،
    /// همه را قفل می‌کند و true برمی‌گرداند.
    pub fn check_auto_lock(&self, now_epoch_secs: i64) -> bool {
        let last = self.last_activity.load(Ordering::SeqCst);
        let timeout = self.auto_lock_timeout_secs.load(Ordering::SeqCst) as i64;
        let any_unlocked = !self.unlocked.lock().expect("vault mutex").is_empty();
        if any_unlocked && now_epoch_secs.saturating_sub(last) >= timeout {
            self.lock_all();
            return true;
        }
        false
    }

    // ---------------- کمک‌های داخلی ----------------

    fn persist_wrapped(
        &self,
        db: &Database,
        profile_id: &str,
        password: &str,
        key: &[u8; crypto::KEY_LEN],
    ) -> Result<(), SecurityError> {
        let salt_hex = kdf::random_salt_hex();
        let salt = hex_to_bytes(&salt_hex)?;
        let kdf_key = Zeroizing::new(kdf::derive_key_argon2id(password, &salt)?);
        let aad = profile_id.as_bytes();
        let wrapped = crypto::encrypt(&kdf_key, key, aad)?;
        let record = VaultRecord {
            verifier: kdf::hash_password(password)?,
            kdf_salt_hex: salt_hex,
            wrapped_key_b64: b64_encode(&wrapped),
            record_version: VAULT_RECORD_VERSION,
        };
        Self::write_record(db, profile_id, &record)
    }

    fn unwrap_key(
        &self,
        profile_id: &str,
        record: &VaultRecord,
        password: &str,
    ) -> Result<VaultKey, SecurityError> {
        if record.record_version != VAULT_RECORD_VERSION {
            return Err(SecurityError::malformed_vault_record(format!(
                "unsupported vault record version: {}",
                record.record_version
            )));
        }
        let salt = hex_to_bytes(&record.kdf_salt_hex)?;
        let kdf_key = Zeroizing::new(kdf::derive_key_argon2id(password, &salt)?);
        let wrapped = b64_decode(&record.wrapped_key_b64)?;
        let aad = profile_id.as_bytes();
        let plain = Zeroizing::new(crypto::decrypt(&kdf_key, &wrapped, aad)?);
        if plain.len() != crypto::KEY_LEN {
            return Err(SecurityError::malformed_vault_record(
                "unwrapped key has wrong length",
            ));
        }
        let mut key = [0u8; crypto::KEY_LEN];
        key.copy_from_slice(&plain);
        Ok(Zeroizing::new(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aria_storage_engine::migrations::run_migrations;

    fn setup_db() -> Database {
        let db = Database::open_memory(None).unwrap();
        {
            let mut conn = db.lock();
            run_migrations(&mut conn, |_| Ok(())).unwrap();
        }
        db
    }

    #[test]
    fn base64_roundtrip_known_vectors() {
        assert_eq!(b64_encode(b""), "");
        assert_eq!(b64_encode(b"f"), "Zg==");
        assert_eq!(b64_encode(b"fo"), "Zm8=");
        assert_eq!(b64_encode(b"foo"), "Zm9v");
        assert_eq!(b64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(b64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(b64_encode(b"foobar"), "Zm9vYmFy");
        for s in ["Zg==", "Zm8=", "Zm9v", "Zm9vYg==", "Zm9vYmE=", "Zm9vYmFy"] {
            assert!(b64_decode(s).is_ok());
        }
        assert!(matches!(
            b64_decode("Zg=Z"),
            Err(SecurityError::MalformedVaultRecord { .. })
        ));
        assert!(matches!(
            b64_decode("abc"),
            Err(SecurityError::MalformedVaultRecord { .. })
        ));
    }

    #[test]
    fn hex_helpers() {
        assert_eq!(hex_to_bytes("00ff10").unwrap(), vec![0, 255, 16]);
        assert!(hex_to_bytes("0f").is_ok());
        assert!(hex_to_bytes("0").is_err());
        assert!(hex_to_bytes("zz").is_err());
    }

    #[test]
    fn setup_then_unlock_roundtrip() {
        let vault = ProfileVault::new(60);
        let db = setup_db();
        let k1 = vault.setup_profile_password(&db, "p1", "strong-pass-1").unwrap();
        assert!(vault.is_unlocked("p1"));

        vault.lock_profile("p1");
        assert!(!vault.is_unlocked("p1"));

        let k2 = vault.unlock_profile(&db, "p1", "strong-pass-1").unwrap();
        assert_eq!(*k1, *k2, "unwrapped key must equal original key");
    }

    #[test]
    fn wrong_password_rejected_on_setup_flow() {
        let vault = ProfileVault::new(60);
        let db = setup_db();
        vault.setup_profile_password(&db, "p1", "strong-pass-1").unwrap();
        vault.lock_profile("p1");
        let err = vault.unlock_profile(&db, "p1", "wrong-pass-9").unwrap_err();
        assert_eq!(err.code(), 1204);
        assert!(!vault.is_unlocked("p1"));
    }

    #[test]
    fn weak_password_rejected_at_setup_and_change() {
        let vault = ProfileVault::new(60);
        let db = setup_db();
        assert!(matches!(
            vault.setup_profile_password(&db, "p1", "short1"),
            Err(SecurityError::WeakPassword { .. })
        ));
        vault.setup_profile_password(&db, "p1", "strong-pass-1").unwrap();
        assert!(matches!(
            vault.change_password(&db, "p1", "strong-pass-1", "weak"),
            Err(SecurityError::WeakPassword { .. })
        ));
    }

    #[test]
    fn double_setup_rejected() {
        let vault = ProfileVault::new(60);
        let db = setup_db();
        vault.setup_profile_password(&db, "p1", "strong-pass-1").unwrap();
        let err = vault.setup_profile_password(&db, "p1", "another-pass-2").unwrap_err();
        assert_eq!(err.code(), 1207);
    }

    #[test]
    fn change_password_keeps_same_key() {
        let vault = ProfileVault::new(60);
        let db = setup_db();
        let k1 = vault.setup_profile_password(&db, "p1", "old-pass-11").unwrap();
        vault.change_password(&db, "p1", "old-pass-11", "new-pass-22").unwrap();

        // گذرواژه قدیمی دیگر کار نمی‌کند
        vault.lock_profile("p1");
        assert_eq!(vault.unlock_profile(&db, "p1", "old-pass-11").unwrap_err().code(), 1204);

        // گذرواژه جدید همان کلید اصلی را باز می‌گرداند
        let k2 = vault.unlock_profile(&db, "p1", "new-pass-22").unwrap();
        assert_eq!(*k1, *k2);
    }

    #[test]
    fn unlock_requires_existing_vault() {
        let vault = ProfileVault::new(60);
        let db = setup_db();
        let err = vault.unlock_profile(&db, "ghost", "whatever-1").unwrap_err();
        assert_eq!(err.code(), 1206);
    }

    #[test]
    fn get_key_on_locked_profile_errors() {
        let vault = ProfileVault::new(60);
        let err = vault.get_key("p1").unwrap_err();
        assert_eq!(err.code(), 1205);
    }

    #[test]
    fn auto_lock_after_timeout() {
        let vault = ProfileVault::new(60);
        let db = setup_db();
        vault.setup_profile_password(&db, "p1", "strong-pass-1").unwrap();
        assert!(vault.is_unlocked("p1"));

        // فعالیت اخیر → قفل نمی‌شود
        assert!(!vault.check_auto_lock(now_epoch()));
        assert!(vault.is_unlocked("p1"));

        // گذشته از مهلت → قفل می‌شود
        assert!(vault.check_auto_lock(now_epoch() + 61));
        assert!(!vault.is_unlocked("p1"));

        // بعد از قفل، دوباره قفل نمی‌کند
        assert!(!vault.check_auto_lock(now_epoch() + 61));
    }

    #[test]
    fn touch_resets_auto_lock_window() {
        let vault = ProfileVault::new(60);
        let db = setup_db();
        vault.setup_profile_password(&db, "p1", "strong-pass-1").unwrap();
        // مرز زمانی مستقل از ساعت واقعی: last_activity >= `before`
        let before = now_epoch();
        vault.touch();
        let t = vault.auto_lock_timeout() as i64;
        // یک ثانیه پیش از مهلت → قفل نمی‌شود
        assert!(!vault.check_auto_lock(before + t - 1));
        vault.touch();
        // یک ثانیه پس از مهلت → قفل می‌شود
        assert!(vault.check_auto_lock(before + t + 1));
        assert!(!vault.is_unlocked("p1"));
    }

    #[test]
    fn auto_lock_disabled_when_nothing_unlocked() {
        let vault = ProfileVault::new(1);
        assert!(!vault.check_auto_lock(now_epoch() + 9999));
    }

    #[test]
    fn set_timeout_min_one() {
        let vault = ProfileVault::new(60);
        vault.set_auto_lock_timeout(0);
        assert_eq!(vault.auto_lock_timeout(), 1);
        vault.set_auto_lock_timeout(300);
        assert_eq!(vault.auto_lock_timeout(), 300);
    }

    #[test]
    fn lock_all_counts_and_clears() {
        let vault = ProfileVault::new(60);
        let db = setup_db();
        vault.setup_profile_password(&db, "p1", "strong-pass-1").unwrap();
        vault.setup_profile_password(&db, "p2", "strong-pass-2").unwrap();
        assert_eq!(vault.lock_all(), 2);
        assert_eq!(vault.lock_all(), 0);
    }

    #[test]
    fn tampered_vault_record_detected() {
        let vault = ProfileVault::new(60);
        let db = setup_db();
        vault.setup_profile_password(&db, "p1", "strong-pass-1").unwrap();
        vault.lock_profile("p1");

        // دستکاری کلید پوشیده‌شده → شکست رمزگشایی
        let mut record = ProfileVault::read_record(&db, "p1").unwrap();
        let mut wrapped = b64_decode(&record.wrapped_key_b64).unwrap();
        let last = wrapped.len() - 1;
        wrapped[last] ^= 0x01;
        record.wrapped_key_b64 = b64_encode(&wrapped);
        let err = vault
            .unwrap_key("p1", &record, "strong-pass-1")
            .unwrap_err();
        assert_eq!(err.code(), 1201);
    }

    #[test]
    fn vault_record_persists_across_vault_instances() {
        let db = setup_db();
        let k1 = {
            let vault = ProfileVault::new(60);
            vault.setup_profile_password(&db, "p1", "strong-pass-1").unwrap()
        };
        {
            let vault2 = ProfileVault::new(60);
            let k2 = vault2.unlock_profile(&db, "p1", "strong-pass-1").unwrap();
            assert_eq!(*k1, *k2, "key must survive restart via persisted record");
        }
    }
}
