//! پایه بسته بکاپ — چیدمان بسته، مانیفست، چک‌سام‌ها و جای‌گاه بار رمزنگاری‌شده.
//!
//! رمزنگاری واقعی بسته در فاز ۱.۱۶ (موتور امنیت) کامل می‌شود؛
//! این ماژول قرارداد فرمت را تثبیت می‌کند.

use crate::error::StorageError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// نسخه فرمت بکاپ (همگام با aria-contracts::BACKUP_FORMAT_VERSION).
pub const BACKUP_FORMAT_VERSION: u32 = 1;

/// چیدمان بسته بکاپ روی دیسک:
///
/// ```text
/// <backup-name>/
/// ├── manifest.json          ← فراداده بسته
/// ├── data/                  ← snapshot رمزنگاری‌شده پایگاه‌داده (payload)
/// ├── attachments/           ← فایل‌های پیوست
/// └── checksums.json         ← هش blake3 همه فایل‌ها
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BackupManifest {
    pub format_version: u32,
    pub created_at: String,
    pub app_version: String,
    /// نام فایل snapshot پایگاه‌داده درون بسته
    pub database_file: String,
    /// تعداد فایل‌های پیوست
    pub attachments_count: u64,
    /// توضیح مدل رمزنگاری (kdf/cipher) — بدون راز
    pub encryption_summary: String,
}

/// نگاشت نام فایل → هش blake3.
pub type Checksums = BTreeMap<String, String>;

/// ساخت مانیفست جدید بسته.
pub fn new_manifest(database_file: &str, attachments_count: u64, encryption_summary: &str) -> BackupManifest {
    BackupManifest {
        format_version: BACKUP_FORMAT_VERSION,
        created_at: crate::attachments::now_iso(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        database_file: database_file.to_string(),
        attachments_count,
        encryption_summary: encryption_summary.to_string(),
    }
}

/// محاسبه چک‌سام همه فایل‌های یک پوشه (بازگشتی) با مسیر نسبی.
pub fn checksums_for_directory(dir: &Path) -> Result<Checksums, StorageError> {
    let mut out = BTreeMap::new();
    checksums_for_dir_inner(dir, dir, &mut out)?;
    Ok(out)
}

fn checksums_for_dir_inner(
    root: &Path,
    dir: &Path,
    out: &mut Checksums,
) -> Result<(), StorageError> {
    let entries = std::fs::read_dir(dir).map_err(|e| StorageError::backup(e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| StorageError::backup(e.to_string()))?;
        let p = entry.path();
        if p.is_dir() {
            checksums_for_dir_inner(root, &p, out)?;
        } else {
            let rel = p
                .strip_prefix(root)
                .map_err(|e| StorageError::backup(e.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, crate::attachments::file_blake3_hash(&p)?);
        }
    }
    Ok(())
}

/// اعتبارسنجی چک‌سام‌های یک پوشه بسته — همه فایل‌ها باید منطبق باشند.
pub fn verify_checksums(dir: &Path, expected: &Checksums) -> Result<bool, StorageError> {
    let actual = checksums_for_directory(dir)?;
    Ok(actual == *expected)
}

/// بررسی سازگاری نسخه فرمت بکاپ.
pub fn is_format_supported(v: u32) -> bool {
    v <= BACKUP_FORMAT_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrip_and_format_check() {
        let m = new_manifest("db.enc", 3, "aes-gcm + argon2id");
        let s = serde_json::to_string(&m).unwrap();
        let back: BackupManifest = serde_json::from_str(&s).unwrap();
        assert_eq!(m, back);
        assert_eq!(back.format_version, 1);
        assert!(is_format_supported(1));
        assert!(!is_format_supported(99));
    }

    #[test]
    fn checksums_match_and_detect_tamper() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"aaa").unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub").join("b.txt"), b"bbb").unwrap();

        let sums = checksums_for_directory(tmp.path()).unwrap();
        assert_eq!(sums.len(), 2);
        assert!(sums.contains_key("a.txt"));
        assert!(sums.contains_key("sub/b.txt"));
        assert!(verify_checksums(tmp.path(), &sums).unwrap());

        // دستکاری فایل → عدم تطابق
        std::fs::write(tmp.path().join("a.txt"), b"xxx").unwrap();
        assert!(!verify_checksums(tmp.path(), &sums).unwrap());
    }
}
