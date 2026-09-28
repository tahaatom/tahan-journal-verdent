//! aria-storage-engine — موتور ذخیره‌سازی کرنل آریا
//!
//! - SQLite رمزنگاری‌شده سازگار با SQLCipher (rusqlite bundled-sqlcipher)
//! - مهاجرت‌های نسخه‌دار مبتنی بر PRAGMA user_version
//! - اسکیمای فیزیکی نسخه ۱ مطابق قرارداد دامنه
//! - تراکنش‌های اتمیک با rollback تضمینی
//! - فراداده پیوست‌ها با هش blake3
//! - پایه بسته بکاپ

pub mod attachments;
pub mod backup;
pub mod connection;
pub mod error;
pub mod migrations;
pub mod schema_v1;

pub use connection::Database;
pub use error::StorageError;

#[cfg(test)]
mod tests {
    use crate::error;

    #[test]
    fn crate_smoke() {
        assert!(!error::STORAGE_ERROR_RANGE.0.to_string().is_empty());
    }
}
