//! قالب فایل بسته بکاپ `.tahanbak` (فاز ۱.۱۶).
//!
//! ```text
//! magic           8 بایت  = "TAHANBAK"
//! format_version  u32 LE   (نسخه ظرف؛ باید ≥ ۱ و ≤ پشتیبانی‌شده)
//! manifest_len    u32 LE
//! manifest        JSON خام (متن روشن — فراداده غیرحساس، به‌عنوان AAD)
//! payload_len     u64 LE
//! payload         blob رمزنگاری‌شده = AES-GCM(zstd(bundle))
//! ```
//!
//! متن روشن مانیفست فقط فراداده است (نسخه، زمان، پارامترهای KDF، فهرست
//! چک‌سام‌ها) و هیچ داده معاملاتی یا متن روشن پایگاه‌داده در آن نیست؛
//! کل بار معاملاتی رمزنگاری‌شده است.

use crate::error::BackupError;
use crate::manifest::BackupManifest;
use std::io::Write;
use std::path::Path;

/// جادوگر فایل بسته.
pub const MAGIC: &[u8; 8] = b"TAHANBAK";

/// نسخه قالب ظرف (مستقل از نسخه مانیفست).
pub const CONTAINER_VERSION: u32 = 1;

/// سقف اندازه مانیفست برای جلوگیری از تخصیص مخرب.
pub const MAX_MANIFEST_BYTES: u32 = 4 * 1024 * 1024;

/// محتوای خوانده‌شده بسته: بایت‌های خام مانیفست + بار رمزنگاری‌شده.
#[derive(Debug)]
pub struct RawPackage {
    /// بایت‌های دقیق مانیفست همان‌طور که در فایل آمده (AAD رمزگشایی)
    pub manifest_bytes: Vec<u8>,
    /// بار رمزنگاری‌شده
    pub payload: Vec<u8>,
}

impl RawPackage {
    /// تحلیل مانیفست از بایت‌های خام.
    pub fn manifest(&self) -> Result<BackupManifest, BackupError> {
        serde_json::from_slice(&self.manifest_bytes)
            .map_err(|e| BackupError::invalid(format!("مانیفست بسته قابل خواندن نیست: {e}")))
    }
}

/// نوشتن بسته روی دیسک — نوشتن اتمیک با فایل موقت و سپس انتقال.
pub fn write_package(
    out_path: &Path,
    version: u32,
    manifest_json: &[u8],
    payload: &[u8],
) -> Result<u64, BackupError> {
    if manifest_json.len() as u64 > MAX_MANIFEST_BYTES as u64 {
        return Err(BackupError::invalid("مانیفست بسته بیش از حد بزرگ است"));
    }
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let tmp = out_path.with_extension("tahanbak.part");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(MAGIC)?;
        f.write_all(&version.to_le_bytes())?;
        f.write_all(&(manifest_json.len() as u32).to_le_bytes())?;
        f.write_all(manifest_json)?;
        f.write_all(&(payload.len() as u64).to_le_bytes())?;
        f.write_all(payload)?;
        f.flush()?;
        // دوام روی دیسک پیش از انتقال اتمیک
        f.sync_all()?;
    }
    // اگر بسته قبلی هست، ابتدا کنار گذاشته می‌شود تا انتقال اتمیک بماند
    let bytes = std::fs::metadata(&tmp)?.len();
    if out_path.exists() {
        std::fs::remove_file(out_path)?;
    }
    std::fs::rename(&tmp, out_path)?;
    Ok(bytes)
}

/// خواندن ظرف بسته و تحلیل ساختار بیرونی (بدون رمزگشایی).
pub fn read_package(path: &Path) -> Result<RawPackage, BackupError> {
    let bytes = std::fs::read(path)?;
    parse_package(&bytes)
}

/// تحلیل بایت‌های ظرف بسته.
pub fn parse_package(bytes: &[u8]) -> Result<RawPackage, BackupError> {
    const HEADER: usize = 8 + 4 + 4;
    if bytes.len() < HEADER {
        return Err(BackupError::invalid("فایل بسته کوتاه‌تر از سرآیند است"));
    }
    if &bytes[0..8] != MAGIC {
        return Err(BackupError::invalid(
            "این فایل بسته بکاپ ژورنال طهان نیست (جادوگر نامعتبر)",
        ));
    }
    let version = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if version < 1 || version > CONTAINER_VERSION {
        return Err(BackupError::UnsupportedFormat {
            found: version,
            supported: CONTAINER_VERSION,
        });
    }
    let manifest_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    if manifest_len == 0 || manifest_len > MAX_MANIFEST_BYTES as usize {
        return Err(BackupError::invalid("طول مانیفست بسته نامعتبر است"));
    }
    let manifest_end = HEADER
        .checked_add(manifest_len)
        .ok_or_else(|| BackupError::invalid("طول مانیفست سرریز می‌کند"))?;
    if manifest_end + 8 > bytes.len() {
        return Err(BackupError::invalid("مانیفست بسته بریده است"));
    }
    let manifest_bytes = bytes[HEADER..manifest_end].to_vec();
    let payload_len = u64::from_le_bytes([
        bytes[manifest_end],
        bytes[manifest_end + 1],
        bytes[manifest_end + 2],
        bytes[manifest_end + 3],
        bytes[manifest_end + 4],
        bytes[manifest_end + 5],
        bytes[manifest_end + 6],
        bytes[manifest_end + 7],
    ]) as usize;
    let payload_start = manifest_end + 8;
    let payload_end = payload_start
        .checked_add(payload_len)
        .ok_or_else(|| BackupError::invalid("طول بار بسته سرریز می‌کند"))?;
    if payload_end != bytes.len() {
        return Err(BackupError::invalid(
            "طول بار بسته با اندازه فایل منطبق نیست (بریده یا دستکاری‌شده)",
        ));
    }
    Ok(RawPackage {
        manifest_bytes,
        payload: bytes[payload_start..payload_end].to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{BackupManifest, CIPHER_ALGORITHM, KDF_ALGORITHM, PACKAGE_FORMAT_VERSION};
    use crate::manifest::{EntryMeta, KdfParams};

    fn manifest_json() -> Vec<u8> {
        let m = BackupManifest {
            format_version: PACKAGE_FORMAT_VERSION,
            created_at: "2026-09-30T00:00:00Z".into(),
            app_version: "0.1.0".into(),
            schema_version: 2,
            cipher: CIPHER_ALGORITHM.into(),
            kdf: KdfParams {
                algorithm: KDF_ALGORITHM.into(),
                salt_hex: "aa".repeat(16),
                m_cost_kib: 65536,
                t_cost: 3,
                p_cost: 1,
                key_len: 32,
            },
            zstd_level: 3,
            raw_bytes: 5,
            raw_blake3: "a".repeat(64),
            attachments_count: 0,
            includes_settings: false,
            entries: vec![EntryMeta {
                path: BackupManifest::DATABASE_ENTRY.into(),
                size: 5,
                blake3: "b".repeat(64),
            }],
            encryption_summary: "aes-256-gcm + argon2id + zstd".into(),
        };
        serde_json::to_vec(&m).unwrap()
    }

    #[test]
    fn write_and_read_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("b.tahanbak");
        let mj = manifest_json();
        let n = write_package(&p, CONTAINER_VERSION, &mj, b"ciphertext-here").unwrap();
        assert_eq!(n, std::fs::metadata(&p).unwrap().len());

        let raw = read_package(&p).unwrap();
        assert_eq!(raw.manifest_bytes, mj, "manifest bytes preserved verbatim for AAD");
        assert_eq!(raw.payload, b"ciphertext-here");
        assert!(raw.manifest().unwrap().is_supported());
    }

    #[test]
    fn overwrite_replaces_previous_package() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("b.tahanbak");
        write_package(&p, CONTAINER_VERSION, &manifest_json(), b"first").unwrap();
        write_package(&p, CONTAINER_VERSION, &manifest_json(), b"second-longer").unwrap();
        assert_eq!(read_package(&p).unwrap().payload, b"second-longer");
        // فایل موقت باقی نمی‌ماند
        assert!(!tmp.path().join("b.tahanbak.part").exists());
    }

    #[test]
    fn rejects_foreign_file() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("x.dat");
        std::fs::write(&p, b"not a backup file at all, definitely not").unwrap();
        let e = read_package(&p).unwrap_err();
        assert_eq!(e.code(), 1600);
        assert!(e.to_string().contains("جادوگر"));
    }

    #[test]
    fn rejects_shorts_and_truncations() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("b.tahanbak");
        write_package(&p, CONTAINER_VERSION, &manifest_json(), b"payload-bytes").unwrap();
        let full = std::fs::read(&p).unwrap();

        // کوتاه‌تر از سرآیند
        assert!(parse_package(&full[..10]).is_err());
        // بدون طول/بار
        for cut in [16, 20, full.len() - 1] {
            assert!(parse_package(&full[..cut]).is_err(), "cut {cut} must fail");
        }
        // داده اضافی در انتها
        let mut extra = full.clone();
        extra.push(0);
        assert!(parse_package(&extra).is_err());
    }

    #[test]
    fn rejects_unsupported_container_and_format_versions() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("b.tahanbak");
        write_package(&p, CONTAINER_VERSION, &manifest_json(), b"x").unwrap();
        let mut bytes = std::fs::read(&p).unwrap();

        // نسخه ظرف نامعتبر
        bytes[8..12].copy_from_slice(&9u32.to_le_bytes());
        let e = parse_package(&bytes).unwrap_err();
        assert_eq!(e.code(), 1603);

        // نسخه صفر ظرف
        bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
        assert!(parse_package(&bytes).is_err());
    }

    #[test]
    fn rejects_absurd_manifest_length() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&CONTAINER_VERSION.to_le_bytes());
        bytes.extend_from_slice(&(MAX_MANIFEST_BYTES + 1).to_le_bytes());
        bytes.extend_from_slice(b"{}");
        assert!(parse_package(&bytes).is_err());
        // صفر هم نامعتبر است
        bytes[12..16].copy_from_slice(&0u32.to_le_bytes());
        assert!(parse_package(&bytes).is_err());
    }

    #[test]
    fn payload_is_preserved_verbatim_for_the_encryptor() {
        // لایه ظرف رمزنگاری انجام نمی‌دهد؛ بار باید بیت‌به‌بیت حفظ شود تا
        // موتور سرویس بتواند بسته رمزنگاری‌شده را بنویسد و بخواند
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("b.tahanbak");
        let payload: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        write_package(&p, CONTAINER_VERSION, &manifest_json(), &payload).unwrap();
        let raw = read_package(&p).unwrap();
        assert_eq!(raw.payload, payload, "container must preserve payload verbatim");
    }
}
