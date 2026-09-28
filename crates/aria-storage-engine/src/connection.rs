//! اتصال پایگاه‌داده رمزنگاری‌شده (SQLCipher سازگار) با PRAGMAهای امنیتی و تراکنش اتمیک.

use crate::error::StorageError;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// مهلت انتظار قفل پایگاه‌داده.
pub const BUSY_TIMEOUT_MS: u64 = 5000;

/// پایگاه‌داده رمزنگاری‌شده ژورنال طهان.
///
/// نکته همروندی: نسخه ۱ تک‌نویسنده (Mutex) است — کافی برای اپ دسکتاپ.
#[derive(Debug)]
pub struct Database {
    conn: Mutex<Connection>,
    path: PathBuf,
    encrypted: bool,
}

/// نتیجه بررسی سلامت پایگاه‌داده.
#[derive(Debug, Clone, PartialEq)]
pub struct HealthReport {
    pub integrity_ok: bool,
    pub integrity_message: String,
    pub schema_version: i64,
    pub page_size: i64,
    pub wal_enabled: bool,
}

impl Database {
    /// بازکردن/ساخت پایگاه‌داده روی مسیر مشخص.
    ///
    /// اگر `passphrase` داده شود، پایگاه‌داده با SQLCipher رمزنگاری می‌شود؛
    /// کلید باید قبل از هر دستور دیگر تنظیم شود.
    pub fn open(path: &Path, passphrase: Option<&str>) -> Result<Self, StorageError> {
        let conn = Connection::open(path)
            .map_err(|e| StorageError::open(format!("{}: {e}", path.display())))?;
        let encrypted = passphrase.is_some();
        Self::finish_setup(conn, path.to_path_buf(), passphrase, encrypted)
    }

    /// پایگاه‌داده درون‌حافظه‌ای (برای تست‌ها).
    pub fn open_memory(passphrase: Option<&str>) -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| StorageError::open(e.to_string()))?;
        Self::finish_setup(conn, PathBuf::from(":memory:"), passphrase, passphrase.is_some())
    }

    fn finish_setup(
        conn: Connection,
        path: PathBuf,
        passphrase: Option<&str>,
        encrypted: bool,
    ) -> Result<Self, StorageError> {
        if let Some(pass) = passphrase {
            set_key(&conn, pass)?;
            // صحت کلید را بلافاصله بعد از تنظیم کلید می‌آزماییم؛
            // با کلید نادرست، نخستین خواندن صفحه رمز با خطا مواجه می‌شود.
            conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
                .map_err(|e| {
                    StorageError::open(format!(
                        "database unlock failed (wrong key or corrupt file): {e}"
                    ))
                })?;
        }
        // PRAGMAهای استاندارد
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| StorageError::open(e.to_string()))?;
        conn.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))
            .map_err(|e| StorageError::open(e.to_string()))?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(|e| StorageError::open(e.to_string()))?;
        if path != Path::new(":memory:") {
            conn.pragma_update(None, "journal_mode", "WAL")
                .map_err(|e| StorageError::open(e.to_string()))?;
        }
        Ok(Self { conn: Mutex::new(conn), path, encrypted })
    }

    /// قفل داخلی و دسترسی به Connection (برای موتورهای بالادست).
    pub fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("storage mutex poisoned")
    }

    /// اجرای تابع در یک تراکنش اتمیک — خطا یعنی rollback کامل.
    pub fn with_transaction<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, StorageError>,
    ) -> Result<T, StorageError> {
        let mut guard = self.lock();
        let tx = guard
            .transaction()
            .map_err(|e| StorageError::transaction(e.to_string()))?;
        match f(&tx) {
            Ok(v) => {
                tx.commit()
                    .map_err(|e| StorageError::transaction(e.to_string()))?;
                Ok(v)
            }
            Err(e) => {
                // rollback صریح؛ اگر خودش خطا داد، خطای اصلی حفظ می‌شود
                let _ = tx.rollback();
                Err(e)
            }
        }
    }

    /// PRAGMA integrity_check.
    pub fn integrity_check(&self) -> Result<String, StorageError> {
        let conn = self.lock();
        let msg: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|e| StorageError::integrity(e.to_string()))?;
        Ok(msg)
    }

    /// گزارش کامل سلامت.
    pub fn health_check(&self) -> Result<HealthReport, StorageError> {
        let integrity = self.integrity_check()?;
        let conn = self.lock();
        let schema_version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| StorageError::query(e.to_string()))?;
        let page_size: i64 = conn
            .query_row("PRAGMA page_size", [], |r| r.get(0))
            .map_err(|e| StorageError::query(e.to_string()))?;
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .map_err(|e| StorageError::query(e.to_string()))?;
        Ok(HealthReport {
            integrity_ok: integrity == "ok",
            integrity_message: integrity,
            schema_version,
            page_size,
            wal_enabled: journal_mode.eq_ignore_ascii_case("wal"),
        })
    }

    /// آیا پایگاه‌داده رمزنگاری‌شده باز شده است.
    pub fn is_encrypted(&self) -> bool {
        self.encrypted
    }

    /// مسیر فایل پایگاه‌داده.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// تنظیم کلید SQLCipher (قبل از هر عملیات دیگر).
fn set_key(conn: &Connection, passphrase: &str) -> Result<(), StorageError> {
    if passphrase.is_empty() {
        return Err(StorageError::open("empty passphrase"));
    }
    // جلوگیری از SQL injection در PRAGMA key با escape تک‌کوتیشن
    let escaped = passphrase.replace('\'', "''");
    conn.execute_batch(&format!("PRAGMA key = '{escaped}';"))
        .map_err(|e| StorageError::open(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_db_open_and_pragmas() {
        let db = Database::open_memory(None).unwrap();
        assert!(!db.is_encrypted());
        let h = db.health_check().unwrap();
        assert!(h.integrity_ok);
        assert!(!h.wal_enabled); // in-memory → WAL ندارد
        assert!(h.schema_version == 0);
    }

    #[test]
    fn file_db_wal_enabled() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("t.db");
        let db = Database::open(&p, None).unwrap();
        let h = db.health_check().unwrap();
        assert!(h.wal_enabled);
        assert!(p.exists());
        // فایل‌های WAL/SHM هم ممکن است ساخته شوند
        drop(db);
    }

    #[test]
    fn encrypted_db_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("enc.db");
        {
            let db = Database::open(&p, Some("S3cret-کلید")).unwrap();
            assert!(db.is_encrypted());
            db.lock()
                .execute_batch("CREATE TABLE t(x INTEGER); INSERT INTO t VALUES (42);")
                .unwrap();
        }
        // بازکردن با کلید درست موفق است
        let db = Database::open(&p, Some("S3cret-کلید")).unwrap();
        let n: i64 = db.lock().query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn wrong_key_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("enc2.db");
        {
            let db = Database::open(&p, Some("right")).unwrap();
            db.lock().execute_batch("CREATE TABLE t(x);").unwrap();
        }
        let err = Database::open(&p, Some("wrong")).unwrap_err();
        assert_eq!(err.code(), 1101);
        assert!(err.to_string().contains("unlock failed"));
    }

    #[test]
    fn foreign_keys_enforced() {
        let db = Database::open_memory(None).unwrap();
        let conn = db.lock();
        conn.execute_batch("CREATE TABLE parent(id TEXT PRIMARY KEY); CREATE TABLE child(pid TEXT REFERENCES parent(id));").unwrap();
        let err = conn
            .execute("INSERT INTO child VALUES ('missing')", [])
            .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("foreign key"));
    }

    #[test]
    fn transaction_rollback_on_error() {
        let db = Database::open_memory(None).unwrap();
        db.lock().execute_batch("CREATE TABLE t(x TEXT);").unwrap();
        let res: Result<(), StorageError> = db.with_transaction(|tx| {
            tx.execute("INSERT INTO t VALUES ('a')", []).map_err(StorageError::from)?;
            Err(StorageError::query("intentional failure"))
        });
        assert!(res.is_err());
        let n: i64 = db.lock().query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "rollback must erase uncommitted inserts");
    }

    #[test]
    fn transaction_commit_on_success() {
        let db = Database::open_memory(None).unwrap();
        db.lock().execute_batch("CREATE TABLE t(x TEXT);").unwrap();
        db.with_transaction(|tx| {
            tx.execute("INSERT INTO t VALUES ('a')", []).map_err(StorageError::from)?;
            tx.execute("INSERT INTO t VALUES ('b')", []).map_err(StorageError::from)?;
            Ok(())
        })
        .unwrap();
        let n: i64 = db.lock().query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn empty_passphrase_rejected() {
        let err = Database::open_memory(Some("")).unwrap_err();
        assert_eq!(err.code(), 1101);
    }
}
