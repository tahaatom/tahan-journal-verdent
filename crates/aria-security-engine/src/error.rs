//! مدل خطای موتور امنیت — بازه کد 1200..=1299.

pub const SECURITY_ERROR_RANGE: (u32, u32) = (1200, 1299);

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SecurityError {
    #[error("decryption failed: {reason}")]
    DecryptionFailed { reason: String, code: u32 },
    #[error("encryption failed: {reason}")]
    EncryptionFailed { reason: String, code: u32 },
    #[error("weak password: {reason}")]
    WeakPassword { reason: String, code: u32 },
    #[error("wrong password")]
    WrongPassword { code: u32 },
    #[error("profile is locked: {profile}")]
    ProfileLocked { profile: String, code: u32 },
    #[error("vault record not found for profile: {profile}")]
    VaultNotFound { profile: String, code: u32 },
    #[error("vault already initialized for profile: {profile}")]
    VaultAlreadyInitialized { profile: String, code: u32 },
    #[error("kdf failed: {reason}")]
    KdfFailed { reason: String, code: u32 },
    #[error("audit error: {reason}")]
    Audit { reason: String, code: u32 },
    /// ۱۲۱۰ — رکورد گاوصندوق خراب
    #[error("malformed vault record: {reason}")]
    MalformedVaultRecord { reason: String, code: u32 },
    /// ۱۲۱۱ — تلاش ورود بیش از حد مجاز؛ قفل موقت (محدودسازی نرخ)
    #[error("too many failed unlock attempts for profile: {profile}")]
    RateLimited { profile: String, code: u32 },
}

impl SecurityError {
    pub fn code(&self) -> u32 {
        match self {
            Self::DecryptionFailed { code, .. }
            | Self::EncryptionFailed { code, .. }
            | Self::WeakPassword { code, .. }
            | Self::WrongPassword { code }
            | Self::ProfileLocked { code, .. }
            | Self::VaultNotFound { code, .. }
            | Self::VaultAlreadyInitialized { code, .. }
            | Self::KdfFailed { code, .. }
            | Self::Audit { code, .. }
            | Self::MalformedVaultRecord { code, .. }
            | Self::RateLimited { code, .. } => *code,
        }
    }

    pub fn variant(&self) -> &'static str {
        match self {
            Self::DecryptionFailed { .. } => "decryption_failed",
            Self::EncryptionFailed { .. } => "encryption_failed",
            Self::WeakPassword { .. } => "weak_password",
            Self::WrongPassword { .. } => "wrong_password",
            Self::ProfileLocked { .. } => "profile_locked",
            Self::VaultNotFound { .. } => "vault_not_found",
            Self::VaultAlreadyInitialized { .. } => "vault_already_initialized",
            Self::KdfFailed { .. } => "kdf_failed",
            Self::Audit { .. } => "audit_error",
            Self::MalformedVaultRecord { .. } => "malformed_vault_record",
            Self::RateLimited { .. } => "rate_limited",
        }
    }

    pub fn message_key(&self) -> &'static str {
        match self {
            Self::DecryptionFailed { .. } => "error.security.decryption_failed",
            Self::EncryptionFailed { .. } => "error.security.encryption_failed",
            Self::WeakPassword { .. } => "error.security.weak_password",
            Self::WrongPassword { .. } => "error.security.wrong_password",
            Self::ProfileLocked { .. } => "error.security.profile_locked",
            Self::VaultNotFound { .. } => "error.security.vault_not_found",
            Self::VaultAlreadyInitialized { .. } => "error.security.vault_already_initialized",
            Self::KdfFailed { .. } => "error.security.kdf_failed",
            Self::Audit { .. } => "error.security.audit_error",
            Self::MalformedVaultRecord { .. } => "error.security.malformed_vault_record",
            Self::RateLimited { .. } => "error.security.rate_limited",
        }
    }

    // سازنده‌های استاندارد با کد پایدار
    pub fn decryption(reason: impl Into<String>) -> Self {
        Self::DecryptionFailed { reason: reason.into(), code: 1201 }
    }
    pub fn encryption(reason: impl Into<String>) -> Self {
        Self::EncryptionFailed { reason: reason.into(), code: 1202 }
    }
    pub fn weak_password(reason: impl Into<String>) -> Self {
        Self::WeakPassword { reason: reason.into(), code: 1203 }
    }
    pub fn wrong_password() -> Self {
        Self::WrongPassword { code: 1204 }
    }
    pub fn profile_locked(profile: impl Into<String>) -> Self {
        Self::ProfileLocked { profile: profile.into(), code: 1205 }
    }
    pub fn vault_not_found(profile: impl Into<String>) -> Self {
        Self::VaultNotFound { profile: profile.into(), code: 1206 }
    }
    pub fn vault_already_initialized(profile: impl Into<String>) -> Self {
        Self::VaultAlreadyInitialized { profile: profile.into(), code: 1207 }
    }
    pub fn kdf(reason: impl Into<String>) -> Self {
        Self::KdfFailed { reason: reason.into(), code: 1208 }
    }
    pub fn audit(reason: impl Into<String>) -> Self {
        Self::Audit { reason: reason.into(), code: 1209 }
    }
    pub fn malformed_vault_record(reason: impl Into<String>) -> Self {
        Self::MalformedVaultRecord { reason: reason.into(), code: 1210 }
    }
    pub fn rate_limited(profile: impl Into<String>) -> Self {
        Self::RateLimited { profile: profile.into(), code: 1211 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn error_codes_unique_and_in_range() {
        let errs = vec![
            SecurityError::decryption("a"),
            SecurityError::encryption("b"),
            SecurityError::weak_password("c"),
            SecurityError::wrong_password(),
            SecurityError::profile_locked("p"),
            SecurityError::vault_not_found("p"),
            SecurityError::vault_already_initialized("p"),
            SecurityError::kdf("d"),
            SecurityError::audit("e"),
            SecurityError::malformed_vault_record("f"),
            SecurityError::rate_limited("p"),
        ];
        let mut seen = HashSet::new();
        for e in errs {
            let c = e.code();
            assert!(seen.insert(c), "duplicate code {c}");
            assert!(c >= SECURITY_ERROR_RANGE.0 && c <= SECURITY_ERROR_RANGE.1);
            assert!(!e.variant().is_empty());
            assert!(e.message_key().starts_with("error.security."));
        }
    }

    #[test]
    fn wrong_password_message_is_generic() {
        // نباید شاهدی قابل انکار درباره وجود/عدم وجود پروفایل بدهد
        let e = SecurityError::wrong_password();
        assert_eq!(e.to_string(), "wrong password");
        assert_eq!(e.code(), 1204);
    }
}
