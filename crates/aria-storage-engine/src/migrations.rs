//! مهاجرت‌های نسخه‌دار مبتنی بر PRAGMA user_version.
//!
//! قواعد:
//! - مهاجرت‌ها ترتیبی، نسخه‌دار و تست‌پذیرند.
//! - مهاجرت هیچ داده‌ای از دست نمی‌دهد.
//! - مهاجرت پرخطر باید پس از بکاپ اجرا شود (`backup_before`).
//! - نسخه پایگاه‌داده همیشه با DATABASE_SCHEMA_VERSION قراردادها همگام است.

use crate::error::StorageError;
use crate::schema_v1::SCHEMA_V1;
use rusqlite::Connection;

/// نسخه فعلی اسکیما (همگام با aria-contracts::DATABASE_SCHEMA_VERSION).
pub const CURRENT_SCHEMA_VERSION: i64 = 2;

/// یک مهاجرت نسخه‌دار.
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

/// رجیستری مهاجرت‌ها — فقط افزودنی.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "physical_schema_v1",
        sql: SCHEMA_V1,
    },
    // ایندکس یکتا روی هش blake3 پیوست — تضمین یکتایی در سطح پایگاه‌داده
    // (سطح برنامه در attachments.rs تکرار را بازمی‌گرداند؛ ایندکس گارد
    // شرایط مسابقه است).
    Migration {
        version: 2,
        name: "unique_attachment_hash_index",
        sql: "DROP INDEX IF EXISTS idx_attachments_hash;\n\
              CREATE UNIQUE INDEX IF NOT EXISTS idx_attachments_hash_unique\n\
              ON attachments(blake3_hash);",
    },
];

/// نسخه فعلی پایگاه‌داده.
pub fn schema_version(conn: &Connection) -> Result<i64, StorageError> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| StorageError::query(e.to_string()))
}

/// اجرای همه مهاجرت‌های باقی‌مانده به‌صورت ترتیبی.
///
/// `backup_before`: برای هر مهاجرت پرخطر قبل از اجرا صدا زده می‌شود
/// (در نسخه ۱ مهاجرت اول ایجاد اسکیماست و نیازی به بکاپ ندارد).
pub fn run_migrations(
    conn: &mut Connection,
    mut backup_before: impl FnMut(i64) -> Result<(), StorageError>,
) -> Result<Vec<i64>, StorageError> {
    let current = schema_version(conn)?;
    let mut applied = Vec::new();

    for m in MIGRATIONS {
        if m.version <= current {
            continue;
        }
        // مهاجرت‌های > 1 پرخطر محسوب می‌شوند (تغییر ساختار روی داده موجود)
        if m.version > 1 {
            backup_before(m.version)?;
        }
        let tx = conn
            .transaction()
            .map_err(|e| StorageError::migration(m.version, e.to_string()))?;
        tx.execute_batch(m.sql)
            .map_err(|e| StorageError::migration(m.version, e.to_string()))?;
        tx.pragma_update(None, "user_version", m.version)
            .map_err(|e| StorageError::migration(m.version, e.to_string()))?;
        tx.commit()
            .map_err(|e| StorageError::migration(m.version, e.to_string()))?;
        tracing::info!(version = m.version, name = m.name, "migration applied");
        applied.push(m.version);
    }
    Ok(applied)
}

/// بررسی همگامی نسخه اسکیما با قرارداد.
pub fn is_schema_current(conn: &Connection) -> Result<bool, StorageError> {
    Ok(schema_version(conn)? == CURRENT_SCHEMA_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::Database;

    #[test]
    fn migrations_run_cleanly_on_empty_db() {
        let db = Database::open_memory(None).unwrap();
        let mut conn = db.lock();
        let applied = run_migrations(&mut conn, |_| Ok(())).unwrap();
        assert_eq!(applied, vec![1, 2]);
        assert!(is_schema_current(&conn).unwrap());
    }

    #[test]
    fn unique_attachment_hash_index_enforced() {
        let db = Database::open_memory(None).unwrap();
        let mut conn = db.lock();
        run_migrations(&mut conn, |_| Ok(())).unwrap();
        conn.execute(
            "INSERT INTO attachments (id, file_name, file_path, thumbnail_path, mime_type,
             size_bytes, blake3_hash, width, height, created_at)
             VALUES ('a1','f.jpg','p',NULL,NULL,10,'abc',NULL,NULL,'2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        // درج هش تکراری باید در سطح پایگاه‌داده رد شود
        let dup = conn.execute(
            "INSERT INTO attachments (id, file_name, file_path, thumbnail_path, mime_type,
             size_bytes, blake3_hash, width, height, created_at)
             VALUES ('a2','f2.jpg','p2',NULL,NULL,10,'abc',NULL,NULL,'2026-01-01T00:00:00Z')",
            [],
        );
        assert!(dup.is_err(), "duplicate blake3_hash must violate unique index");
    }

    #[test]
    fn migrations_are_idempotent() {
        let db = Database::open_memory(None).unwrap();
        {
            let mut conn = db.lock();
            run_migrations(&mut conn, |_| Ok(())).unwrap();
        }
        let mut conn = db.lock();
        let applied = run_migrations(&mut conn, |_| Ok(())).unwrap();
        assert!(applied.is_empty(), "second run must apply nothing");
    }

    #[test]
    fn backup_before_called_for_risky_migrations() {
        let db = Database::open_memory(None).unwrap();
        let mut conn = db.lock();
        let mut backups = Vec::new();
        run_migrations(&mut conn, |v| {
            backups.push(v);
            Ok(())
        })
        .unwrap();
        // مهاجرت ۱ نیازی به بکاپ ندارد؛ مهاجرت ۲ (> ۱) پرخطر محسوب می‌شود
        assert_eq!(backups, vec![2]);
    }

    #[test]
    fn schema_version_starts_at_zero_for_fresh_db() {
        let db = Database::open_memory(None).unwrap();
        let conn = db.lock();
        assert_eq!(schema_version(&conn).unwrap(), 0);
    }
}
