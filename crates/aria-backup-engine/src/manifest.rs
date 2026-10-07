//! مانیفست بسته بکاپ — فراداده احرازشده (AAD) و فهرست چک‌سام‌ها (فاز ۱.۱۶).
//!
//! مانیفست به‌صورت JSON خام در بسته ذخیره و در رمزگشایی به‌عنوان AAD به
//! AES-GCM داده می‌شود؛ بنابراین دستکاری هر فیلد (نسخه، چک‌سام، نمک) با
//! شکست احراز هویت تشخیص داده می‌شود.

use serde::{Deserialize, Serialize};

/// نسخه فرمت بسته — همگام با `aria_contracts::BACKUP_FORMAT_VERSION`.
pub const PACKAGE_FORMAT_VERSION: u32 = aria_contracts::BACKUP_FORMAT_VERSION;

/// الگوریتم مشتق کلید (تنها مقدار نسخه ۱).
pub const KDF_ALGORITHM: &str = "argon2id";

/// الگوریتم رمز (تنها مقدار نسخه ۱).
pub const CIPHER_ALGORITHM: &str = "aes-256-gcm";

/// سطح پیش‌فرض فشرده‌سازی zstd.
pub const DEFAULT_ZSTD_LEVEL: i32 = 3;

/// فراداده یک فایل درون بسته.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EntryMeta {
    /// مسیر نسبی درون بسته (با `/`)
    pub path: String,
    /// اندازه بایت پیش از فشرده‌سازی
    pub size: u64,
    /// هش blake3 محتوا (hex)
    pub blake3: String,
}

/// پارامترهای KDF ذخیره‌شده در مانیفست — بدون هیچ رازی.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct KdfParams {
    pub algorithm: String,
    /// نمک hex (۱۶ بایت)
    pub salt_hex: String,
    /// هزینه حافظه (KiB)
    pub m_cost_kib: u32,
    /// تعداد گذرها
    pub t_cost: u32,
    /// موازی‌سازی
    pub p_cost: u32,
    /// طول کلید مشتق‌شده (بایت)
    pub key_len: u32,
}

/// مانیفست بسته بکاپ نسخه ۱.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BackupManifest {
    /// نسخه فرمت بسته
    pub format_version: u32,
    /// زمان ساخت (RFC3339 UTC)
    pub created_at: String,
    /// نسخه برنامه سازنده
    pub app_version: String,
    /// نسخه اسکیمای پایگاه‌داده snapshot
    pub schema_version: i64,
    /// الگوریتم رمز
    pub cipher: String,
    /// پارامترهای مشتق کلید
    pub kdf: KdfParams,
    /// سطح فشرده‌سازی zstd
    pub zstd_level: i32,
    /// اندازه بار خام (پیش از فشرده‌سازی) — همه فایل‌ها روی هم
    pub raw_bytes: u64,
    /// هش blake3 بار خام (پیش از فشرده‌سازی)
    pub raw_blake3: String,
    /// تعداد فایل‌های پیوست
    pub attachments_count: u64,
    /// آیا snapshot تنظیمات در بسته هست؟
    pub includes_settings: bool,
    /// فهرست فایل‌های بسته با چک‌سام
    pub entries: Vec<EntryMeta>,
    /// خلاصه مدل رمزنگاری برای نمایش به کاربر (بدون راز)
    pub encryption_summary: String,
}

impl BackupManifest {
    /// مسیر فایل snapshot پایگاه‌داده درون بسته.
    pub const DATABASE_ENTRY: &'static str = "database/tahan.db";
    /// مسیر snapshot تنظیمات درون بسته.
    pub const SETTINGS_ENTRY: &'static str = "settings/settings.json";
    /// پیشوند مسیر پیوست‌ها درون بسته.
    pub const ATTACHMENTS_PREFIX: &'static str = "attachments/";

    /// بررسی سازگاری نسخه فرمت.
    pub fn is_supported(&self) -> bool {
        is_format_supported(self.format_version)
    }

    /// آیا فهرست چک‌سام‌ها فایل تکراری دارد؟
    pub fn has_duplicate_paths(&self) -> bool {
        let mut seen = std::collections::HashSet::new();
        self.entries.iter().any(|e| !seen.insert(e.path.as_str()))
    }

    /// بررسی سازگاری ساختاری مانیفست (پیش از هر عملیات رمزگشایی).
    pub fn validate(&self) -> Result<(), crate::error::BackupError> {
        use crate::error::BackupError;
        if !self.is_supported() {
            return Err(BackupError::UnsupportedFormat {
                found: self.format_version,
                supported: PACKAGE_FORMAT_VERSION,
            });
        }
        if self.entries.is_empty() {
            return Err(BackupError::invalid("فهرست فایل‌های بسته خالی است"));
        }
        if self.has_duplicate_paths() {
            return Err(BackupError::invalid("فهرست فایل‌های بسته مسیر تکراری دارد"));
        }
        if self.entries.iter().any(|e| {
            e.path.is_empty()
                || e.path.starts_with('/')
                || e.path.contains("..")
                || e.path.contains('\\')
        }) {
            return Err(BackupError::invalid("مسیر ناامن در فهرست فایل‌های بسته"));
        }
        if self.cipher != CIPHER_ALGORITHM {
            return Err(BackupError::invalid(format!("رمز پشتیبانی‌نشده: {}", self.cipher)));
        }
        if self.kdf.algorithm != KDF_ALGORITHM {
            return Err(BackupError::invalid(format!(
                "مشتق کلید پشتیبانی‌نشده: {}",
                self.kdf.algorithm
            )));
        }
        if !self.entries.iter().any(|e| e.path == Self::DATABASE_ENTRY) {
            return Err(BackupError::invalid("snapshot پایگاه‌داده در بسته نیست"));
        }
        Ok(())
    }
}

/// آیا نسخه فرمت بسته پشتیبانی می‌شود؟ (بکاپ‌های قدیمی‌تر بله)
pub fn is_format_supported(v: u32) -> bool {
    v >= 1 && v <= PACKAGE_FORMAT_VERSION
}

/// فهرست پیوست‌های مانیفست.
pub fn attachment_entries(m: &BackupManifest) -> Vec<&EntryMeta> {
    m.entries
        .iter()
        .filter(|e| e.path.starts_with(BackupManifest::ATTACHMENTS_PREFIX))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> KdfParams {
        KdfParams {
            algorithm: KDF_ALGORITHM.into(),
            salt_hex: "0123456789abcdef0123456789abcdef".into(),
            m_cost_kib: 65536,
            t_cost: 3,
            p_cost: 1,
            key_len: 32,
        }
    }

    fn manifest() -> BackupManifest {
        BackupManifest {
            format_version: PACKAGE_FORMAT_VERSION,
            created_at: "2026-09-30T00:00:00Z".into(),
            app_version: "0.1.0".into(),
            schema_version: 2,
            cipher: CIPHER_ALGORITHM.into(),
            kdf: params(),
            zstd_level: DEFAULT_ZSTD_LEVEL,
            raw_bytes: 10,
            raw_blake3: "a".repeat(64),
            attachments_count: 1,
            includes_settings: true,
            entries: vec![
                EntryMeta {
                    path: BackupManifest::DATABASE_ENTRY.into(),
                    size: 10,
                    blake3: "b".repeat(64),
                },
                EntryMeta {
                    path: format!("{}{}", BackupManifest::ATTACHMENTS_PREFIX, "a.png"),
                    size: 5,
                    blake3: "c".repeat(64),
                },
            ],
            encryption_summary: "aes-256-gcm + argon2id + zstd".into(),
        }
    }

    #[test]
    fn manifest_roundtrips_through_json() {
        let m = manifest();
        let s = serde_json::to_string(&m).unwrap();
        let back: BackupManifest = serde_json::from_str(&s).unwrap();
        assert_eq!(m, back);
        assert!(back.is_supported());
    }

    #[test]
    fn format_version_bounds() {
        assert!(is_format_supported(1));
        assert!(!is_format_supported(0));
        assert!(!is_format_supported(PACKAGE_FORMAT_VERSION + 1));
        assert_eq!(PACKAGE_FORMAT_VERSION, aria_contracts::BACKUP_FORMAT_VERSION);
    }

    #[test]
    fn validate_accepts_wellformed_manifest() {
        assert!(manifest().validate().is_ok());
    }

    #[test]
    fn validate_rejects_unsupported_version() {
        let m = BackupManifest { format_version: 99, ..manifest() };
        let e = m.validate().unwrap_err();
        assert_eq!(e.code(), 1603);
    }

    #[test]
    fn validate_rejects_unsafe_paths() {
        for bad in ["../escape.db", "/abs.db", "dir\\win.db", ""] {
            let m = BackupManifest {
                entries: vec![
                    EntryMeta { path: bad.into(), size: 1, blake3: "d".repeat(64) },
                    EntryMeta {
                        path: BackupManifest::DATABASE_ENTRY.into(),
                        size: 1,
                        blake3: "e".repeat(64),
                    },
                ],
                ..manifest()
            };
            assert!(m.validate().is_err(), "path {bad:?} must be rejected");
        }
    }

    #[test]
    fn validate_requires_database_entry() {
        let m = BackupManifest {
            entries: vec![EntryMeta {
                path: "settings/settings.json".into(),
                size: 1,
                blake3: "f".repeat(64),
            }],
            ..manifest()
        };
        assert!(m.validate().unwrap_err().to_string().contains("پایگاه‌داده"));
    }

    #[test]
    fn validate_rejects_duplicate_paths_and_weak_algorithms() {
        let m = BackupManifest {
            entries: vec![
                EntryMeta {
                    path: BackupManifest::DATABASE_ENTRY.into(),
                    size: 1,
                    blake3: "1".repeat(64),
                },
                EntryMeta {
                    path: BackupManifest::DATABASE_ENTRY.into(),
                    size: 1,
                    blake3: "2".repeat(64),
                },
            ],
            ..manifest()
        };
        assert!(m.validate().unwrap_err().to_string().contains("تکراری"));

        let weak_cipher = BackupManifest { cipher: "aes-128-ecb".into(), ..manifest() };
        assert!(weak_cipher.validate().is_err());
        let weak_kdf = BackupManifest {
            kdf: KdfParams { algorithm: "md5".into(), ..params() },
            ..manifest()
        };
        assert!(weak_kdf.validate().is_err());
    }

    #[test]
    fn attachment_entries_filters_by_prefix() {
        let m = manifest();
        let atts = attachment_entries(&m);
        assert_eq!(atts.len(), 1);
        assert_eq!(atts[0].path, "attachments/a.png");
    }
}
