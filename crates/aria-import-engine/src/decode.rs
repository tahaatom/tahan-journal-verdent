//! تشخیص رمزگذاری و رمزگشایی امن فایل خروجی متاتریدر.
//!
//! خروجی‌های متاتریدر عموماً UTF-16LE (گزارش HTML) یا UTF-8/ANSI
//! هستند؛ ترتیب تشخیص: BOM صریح → UTF-8 سخت‌گیرانه → Windows-1252.

use encoding_rs::{UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252};

use crate::error::ImportError;

/// نتیجه رمزگشایی — متن و اینکه آیا رمزگذاری به‌صورت قطعی تشخیص داده شد.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub text: String,
    /// false یعنی بدون BOM و با fallback رمزگشایی شده — هشدار گزارش
    pub certain: bool,
}

/// رمزگشایی بایت‌های فایل خروجی متاتریدر.
pub fn decode(bytes: &[u8]) -> Result<Decoded, ImportError> {
    if bytes.is_empty() {
        return Err(ImportError::msg("فایل ایمپورت خالی است"));
    }
    // BOM صریح
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (text, _, _) = UTF_8.decode(&bytes[3..]);
        return Ok(Decoded { text: text.into_owned(), certain: true });
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (text, _, _) = UTF_16LE.decode(&bytes[2..]);
        return Ok(Decoded { text: text.into_owned(), certain: true });
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (text, _, _) = UTF_16BE.decode(&bytes[2..]);
        return Ok(Decoded { text: text.into_owned(), certain: true });
    }
    // UTF-8 سخت‌گیرانه
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(Decoded { text: text.to_string(), certain: true }),
        // fallback تک‌بایتی — رایج در خروجی‌های ANSI بروکرها
        Err(_) => {
            let (text, _, _) = WINDOWS_1252.decode(bytes);
            Ok(Decoded { text: text.into_owned(), certain: false })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8_utf16_and_fallback() {
        // UTF-8 با BOM
        let mut b = vec![0xEF, 0xBB, 0xBF];
        b.extend_from_slice("سلام,price".as_bytes());
        let d = decode(&b).unwrap();
        assert!(d.certain && d.text.starts_with("سلام"));

        // UTF-16LE با BOM — قالب رایج گزارش متاتریدر
        // (UTF_16LE در encoding_rs فقط رمزگشا است؛ بایت‌ها دستی ساخته می‌شوند)
        let mut b16 = vec![0xFF, 0xFE];
        b16.extend("2024.01.15\tdeal".encode_utf16().flat_map(u16::to_le_bytes));
        let d16 = decode(&b16).unwrap();
        assert!(d16.certain);
        assert_eq!(d16.text, "2024.01.15\tdeal");

        // UTF-8 بدون BOM
        let d8 = decode("Time,Symbol".as_bytes()).unwrap();
        assert!(d8.certain && d8.text == "Time,Symbol");

        // بایت‌های نامعتبر → fallback 1252 با هشدار
        let df = decode(&[0x48, 0xFC, 0x46]).unwrap();
        assert!(!df.certain);
        assert_eq!(df.text, "HüF");
    }

    #[test]
    fn rejects_empty() {
        let err = decode(&[]).unwrap_err();
        assert_eq!(err.code(), 1500);
    }
}
