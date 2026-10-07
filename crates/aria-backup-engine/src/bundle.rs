//! بسته درونی — دنباله طول‌پیشوندی فایل‌ها (فاز ۱.۱۶).
//!
//! قالب بایت (پیش از فشرده‌سازی و رمزنگاری):
//!
//! ```text
//! u32  count
//! تکرار برای هر فایل:
//!   u32  path_len || path (UTF-8)
//!   u64  data_len || data
//! ```
//!
//! این قالب بدون وابستگی بیرونی و قابل جریان‌سازی است؛ همه طول‌ها
//! little-endian هستند.

use crate::error::BackupError;
use crate::manifest::EntryMeta;

/// یک فایل درون بسته.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleEntry {
    pub path: String,
    pub data: Vec<u8>,
}

/// ساخت بایت‌های بسته درونی از فهرست فایل‌ها (مرتب‌شده بر اساس مسیر — خروجی قطعی).
pub fn encode(entries: &[BundleEntry]) -> Vec<u8> {
    let mut sorted: Vec<&BundleEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    let total: usize = sorted.iter().map(|e| 8 + e.path.len() + e.data.len()).sum();
    let mut out = Vec::with_capacity(total + 4);
    out.extend_from_slice(&(sorted.len() as u32).to_le_bytes());
    for e in sorted {
        out.extend_from_slice(&(e.path.len() as u32).to_le_bytes());
        out.extend_from_slice(e.path.as_bytes());
        out.extend_from_slice(&(e.data.len() as u64).to_le_bytes());
        out.extend_from_slice(&e.data);
    }
    out
}

/// تحلیل بایت‌های بسته درونی — خطای ساختاری می‌دهد اگر بریده یا نامعتبر باشد.
pub fn decode(bytes: &[u8]) -> Result<Vec<BundleEntry>, BackupError> {
    let mut cursor = Cursor { buf: bytes, pos: 0 };
    let count = cursor.u32()? as usize;
    // هر فایل حداقل ۱۲ بایت سرآیند دارد — سقف واقع‌گرایانه برای جلوگیری از تخصیص بزرگ
    if count > bytes.len() / 12 + 1 {
        return Err(BackupError::invalid("تعداد فایل‌های بسته نامعتبر است"));
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let path_len = cursor.u32()? as usize;
        let path = cursor.take(path_len)?;
        let path = String::from_utf8(path.to_vec())
            .map_err(|_| BackupError::invalid("مسیر فایل بسته UTF-8 نیست"))?;
        if path.is_empty() || path.starts_with('/') || path.contains("..") || path.contains('\\') {
            return Err(BackupError::invalid(format!("مسیر ناامن در بسته: {path}")));
        }
        let data_len = cursor.u64()? as usize;
        let data = cursor.take(data_len)?.to_vec();
        out.push(BundleEntry { path, data });
    }
    if cursor.pos != bytes.len() {
        return Err(BackupError::invalid("داده اضافی در انتهای بسته"));
    }
    Ok(out)
}

/// استخراج فراداده چک‌سام از فایل‌های بسته.
pub fn entry_meta(entries: &[BundleEntry]) -> Vec<EntryMeta> {
    let mut out: Vec<EntryMeta> = entries
        .iter()
        .map(|e| EntryMeta {
            path: e.path.clone(),
            size: e.data.len() as u64,
            blake3: blake3::hash(&e.data).to_hex().to_string(),
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// اعتبارسنجی فایل‌های استخراج‌شده در برابر فهرست مانیفست — مجموعه و هش‌ها.
pub fn verify_against_manifest(
    entries: &[BundleEntry],
    expected: &[EntryMeta],
) -> Result<(), BackupError> {
    let actual = entry_meta(entries);
    if actual.len() != expected.len() {
        return Err(BackupError::integrity(format!(
            "تعداد فایل‌ها منطبق نیست (بسته: {}، مانیفست: {})",
            actual.len(),
            expected.len()
        )));
    }
    for (a, e) in actual.iter().zip(expected.iter()) {
        if a.path != e.path {
            return Err(BackupError::integrity(format!(
                "مسیر فایل منطبق نیست (بسته: {}، مانیفست: {})",
                a.path, e.path
            )));
        }
        if a.size != e.size {
            return Err(BackupError::integrity(format!(
                "اندازه فایل «{}» منطبق نیست",
                a.path
            )));
        }
        if a.blake3 != e.blake3 {
            return Err(BackupError::integrity(format!(
                "هش فایل «{}» منطبق نیست",
                a.path
            )));
        }
    }
    Ok(())
}

/// هش blake3 کل بار خام.
pub fn raw_hash(entries: &[BundleEntry]) -> String {
    blake3::hash(&encode(entries)).to_hex().to_string()
}

/// مکان‌نمای خواندن بایت‌ها با بررسی مرزها.
struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], BackupError> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| BackupError::invalid("طول نامعتبر در بسته"))?;
        if end > self.buf.len() {
            return Err(BackupError::invalid("بسته بریده یا خراب است"));
        }
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32, BackupError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u64(&mut self) -> Result<u64, BackupError> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<BundleEntry> {
        vec![
            BundleEntry { path: "database/tahan.db".into(), data: b"SQLite format 3".to_vec() },
            BundleEntry { path: "attachments/a.png".into(), data: vec![1, 2, 3, 4] },
            BundleEntry { path: "settings/settings.json".into(), data: b"{}".to_vec() },
        ]
    }

    #[test]
    fn encode_decode_roundtrip() {
        let e = entries();
        let bytes = encode(&e);
        let back = decode(&bytes).unwrap();
        assert_eq!(back.len(), 3);
        // خروجی قطعی و مرتب بر اساس مسیر
        assert_eq!(back[0].path, "attachments/a.png");
        assert_eq!(back[1].path, "database/tahan.db");
        assert_eq!(back[2].path, "settings/settings.json");
        assert_eq!(back[2].data, b"{}");
    }

    #[test]
    fn encode_is_deterministic_regardless_of_input_order() {
        let mut shuffled = entries();
        shuffled.reverse();
        assert_eq!(encode(&entries()), encode(&shuffled));
    }

    #[test]
    fn empty_bundle_roundtrips() {
        let bytes = encode(&[]);
        assert_eq!(bytes, 0u32.to_le_bytes().to_vec());
        assert!(decode(&bytes).unwrap().is_empty());
    }

    #[test]
    fn binary_payload_survives_roundtrip() {
        let data: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
        let e = vec![BundleEntry { path: "attachments/big.bin".into(), data: data.clone() }];
        let back = decode(&encode(&e)).unwrap();
        assert_eq!(back[0].data, data);
    }

    #[test]
    fn truncated_bundle_is_rejected() {
        let bytes = encode(&entries());
        for cut in [1, 3, 5, 20, bytes.len() - 1] {
            assert!(
                decode(&bytes[..cut]).is_err(),
                "truncation at {cut} must fail"
            );
        }
    }

    #[test]
    fn trailing_garbage_is_rejected() {
        let mut bytes = encode(&entries());
        bytes.push(0xFF);
        assert!(decode(&bytes).is_err());
    }

    #[test]
    fn unsafe_paths_in_bundle_are_rejected() {
        // بسته دست‌ساز با مسیر فرار
        let mut bytes = 1u32.to_le_bytes().to_vec();
        let path = b"../evil";
        bytes.extend_from_slice(&(path.len() as u32).to_le_bytes());
        bytes.extend_from_slice(path);
        bytes.extend_from_slice(&0u64.to_le_bytes());
        assert!(decode(&bytes).is_err());
    }

    #[test]
    fn absurd_entry_count_is_rejected_without_allocation() {
        // count بزرگ در بسته کوچک → رد پیش از تخصیص حافظه
        let bytes = u32::MAX.to_le_bytes().to_vec();
        assert!(decode(&bytes).is_err());
    }

    #[test]
    fn entry_meta_sorted_with_hashes() {
        let meta = entry_meta(&entries());
        assert_eq!(meta.len(), 3);
        assert_eq!(meta[0].path, "attachments/a.png");
        assert_eq!(meta[0].size, 4);
        assert_eq!(meta[0].blake3, blake3::hash(&[1u8, 2, 3, 4]).to_hex().to_string());
    }

    #[test]
    fn verify_against_manifest_detects_tampering() {
        let e = entries();
        let meta = entry_meta(&e);
        assert!(verify_against_manifest(&e, &meta).is_ok());

        // دستکاری محتوا → عدم تطابق هش (هم‌طول تا اندازه تغییر نکند)
        let mut tampered = e.clone();
        tampered[0].data = b"XQLite format 3".to_vec();
        let err = verify_against_manifest(&tampered, &meta).unwrap_err();
        assert_eq!(err.code(), 1602);
        assert!(err.to_string().contains("هش"));

        // حذف فایل → عدم تطابق تعداد
        let fewer = &e[..1];
        assert!(verify_against_manifest(fewer, &meta).is_err());
        // فایل اضافی → عدم تطابق تعداد
        let mut more = e.clone();
        more.push(BundleEntry { path: "attachments/extra.bin".into(), data: vec![9] });
        assert!(verify_against_manifest(&more, &meta).is_err());
    }

    #[test]
    fn raw_hash_tracks_content() {
        let e = entries();
        let h = raw_hash(&e);
        assert_eq!(h.len(), 64);
        assert_eq!(h, raw_hash(&e));
        let mut changed = e.clone();
        changed[0].data.push(0);
        assert_ne!(h, raw_hash(&changed));
    }
}
