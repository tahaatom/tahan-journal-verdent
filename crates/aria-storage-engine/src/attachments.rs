//! پیوست‌ها — ذخیره فایل بیرون پایگاه‌داده، فراداده درون پایگاه‌داده،
//! هش blake3 برای صحت و تشخیص تکرار، و فراداده بندانگشتی.

use crate::connection::Database;
use crate::error::StorageError;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// فراداده پیوست.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AttachmentMetadata {
    pub id: String,
    pub file_name: String,
    pub file_path: String,
    pub thumbnail_path: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
    pub blake3_hash: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub created_at: String,
}

/// محاسبه هش blake3 یک فایل (جریانی، بدون بارگذاری کامل در حافظه).
pub fn file_blake3_hash(path: &Path) -> Result<String, StorageError> {
    use std::io::Read;
    let mut file =
        std::fs::File::open(path).map_err(|e| StorageError::attachment(e.to_string()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| StorageError::attachment(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// ثبت فراداده پیوست پس از انتقال فایل به پوشه پیوست‌ها.
///
/// فایل باید از قبل در `file_path` کپی شده باشد؛ این تابع فقط فراداده می‌سازد
/// و تشخیص تکرار را با هش انجام می‌دهد.
pub fn register_attachment(
    db: &Database,
    file_name: &str,
    file_path: &Path,
    mime_type: Option<&str>,
    width: Option<i64>,
    height: Option<i64>,
) -> Result<AttachmentMetadata, StorageError> {
    if !file_path.exists() {
        return Err(StorageError::attachment(format!(
            "file not found: {}",
            file_path.display()
        )));
    }
    let hash = file_blake3_hash(file_path)?;
    let size = std::fs::metadata(file_path)
        .map_err(|e| StorageError::attachment(e.to_string()))?
        .len() as i64;

    // تشخیص تکرار: پیوست با همین هش قبلاً ثبت شده؟
    if let Some(existing) = find_by_hash(db, &hash)? {
        return Ok(existing);
    }

    let id = uuid::Uuid::new_v4().to_string();
    let created_at = now_iso();
    db.lock()
        .execute(
            "INSERT INTO attachments (id, file_name, file_path, thumbnail_path, mime_type, size_bytes, blake3_hash, width, height, created_at)
             VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                id,
                file_name,
                file_path.display().to_string(),
                mime_type,
                size,
                hash,
                width,
                height,
                created_at
            ],
        )
        .map_err(StorageError::from)?;

    Ok(AttachmentMetadata {
        id,
        file_name: file_name.to_string(),
        file_path: file_path.display().to_string(),
        thumbnail_path: None,
        mime_type: mime_type.map(str::to_string),
        size_bytes: size,
        blake3_hash: hash,
        width,
        height,
        created_at,
    })
}

/// یافتن پیوست با هش (تشخیص فایل تکراری).
pub fn find_by_hash(db: &Database, hash: &str) -> Result<Option<AttachmentMetadata>, StorageError> {
    let conn = db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id, file_name, file_path, thumbnail_path, mime_type, size_bytes, blake3_hash, width, height, created_at
             FROM attachments WHERE blake3_hash = ?1 LIMIT 1",
        )
        .map_err(StorageError::from)?;
    let mut rows = stmt.query([hash]).map_err(StorageError::from)?;
    if let Some(r) = rows.next().map_err(StorageError::from)? {
        return Ok(Some(map_row(r).map_err(StorageError::from)?));
    }
    Ok(None)
}

/// خواندن فراداده پیوست.
pub fn get_attachment(db: &Database, id: &str) -> Result<AttachmentMetadata, StorageError> {
    let conn = db.lock();
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, file_name, file_path, thumbnail_path, mime_type, size_bytes, blake3_hash, width, height, created_at
             FROM attachments WHERE id = ?1",
        )
        .map_err(StorageError::from)?;
    let mut rows = stmt.query([id]).map_err(StorageError::from)?;
    match rows.next().map_err(StorageError::from)? {
        Some(r) => map_row(r).map_err(StorageError::from),
        None => Err(StorageError::not_found("attachment", id)),
    }
}

/// اعتبارسنجی صحت فایل پیوست روی دیسک (مقایسه هش).
pub fn verify_integrity(db: &Database, id: &str) -> Result<bool, StorageError> {
    let meta = get_attachment(db, id)?;
    let p = Path::new(&meta.file_path);
    if !p.exists() {
        return Ok(false);
    }
    let current = file_blake3_hash(p)?;
    Ok(current == meta.blake3_hash)
}

/// ثبت مسیر بندانگشتی برای پیوست (تولید تصویر در فاز ۱.۱۳).
pub fn set_thumbnail_path(db: &Database, id: &str, thumbnail_path: &str) -> Result<(), StorageError> {
    let n = db
        .lock()
        .execute(
            "UPDATE attachments SET thumbnail_path = ?2 WHERE id = ?1",
            rusqlite::params![id, thumbnail_path],
        )
        .map_err(StorageError::from)?;
    if n == 0 {
        return Err(StorageError::not_found("attachment", id));
    }
    Ok(())
}

fn map_row(r: &rusqlite::Row<'_>) -> Result<AttachmentMetadata, rusqlite::Error> {
    Ok(AttachmentMetadata {
        id: r.get(0)?,
        file_name: r.get(1)?,
        file_path: r.get(2)?,
        thumbnail_path: r.get(3)?,
        mime_type: r.get(4)?,
        size_bytes: r.get(5)?,
        blake3_hash: r.get(6)?,
        width: r.get(7)?,
        height: r.get(8)?,
        created_at: r.get(9)?,
    })
}

/// زمان فعلی ISO UTC (کمک داخلی).
pub(crate) fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::Database;
    use crate::migrations::run_migrations;

    fn setup() -> Database {
        let db = Database::open_memory(None).unwrap();
        {
            let mut conn = db.lock();
            run_migrations(&mut conn, |_| Ok(())).unwrap();
        }
        db
    }

    #[test]
    fn register_and_get_attachment() {
        let db = setup();
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("chart.png");
        std::fs::write(&f, b"fake-png-bytes").unwrap();

        let meta = register_attachment(&db, "chart.png", &f, Some("image/png"), Some(800), Some(600)).unwrap();
        assert_eq!(meta.size_bytes, 14);
        assert!(meta.blake3_hash.len() == 64);
        assert!(meta.thumbnail_path.is_none());

        let got = get_attachment(&db, &meta.id).unwrap();
        assert_eq!(got, meta);
    }

    #[test]
    fn duplicate_hash_returns_existing() {
        let db = setup();
        let tmp = tempfile::tempdir().unwrap();
        let f1 = tmp.path().join("a.png");
        let f2 = tmp.path().join("b.png");
        std::fs::write(&f1, b"same").unwrap();
        std::fs::write(&f2, b"same").unwrap();

        let m1 = register_attachment(&db, "a.png", &f1, None, None, None).unwrap();
        let m2 = register_attachment(&db, "b.png", &f2, None, None, None).unwrap();
        assert_eq!(m1.id, m2.id, "same content must map to same attachment");
    }

    #[test]
    fn integrity_detects_corruption_and_missing_file() {
        let db = setup();
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("x.bin");
        std::fs::write(&f, b"hello").unwrap();
        let m = register_attachment(&db, "x.bin", &f, None, None, None).unwrap();
        assert!(verify_integrity(&db, &m.id).unwrap());

        std::fs::write(&f, b"corrupted!").unwrap();
        assert!(!verify_integrity(&db, &m.id).unwrap());

        std::fs::remove_file(&f).unwrap();
        assert!(!verify_integrity(&db, &m.id).unwrap());
    }

    #[test]
    fn thumbnail_path_metadata() {
        let db = setup();
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("y.png");
        std::fs::write(&f, b"img").unwrap();
        let m = register_attachment(&db, "y.png", &f, None, None, None).unwrap();
        set_thumbnail_path(&db, &m.id, "thumbs/y_thumb.png").unwrap();
        let got = get_attachment(&db, &m.id).unwrap();
        assert_eq!(got.thumbnail_path.as_deref(), Some("thumbs/y_thumb.png"));
    }

    #[test]
    fn missing_file_rejected() {
        let db = setup();
        let err = register_attachment(&db, "nope", Path::new("Z:/does/not/exist"), None, None, None).unwrap_err();
        assert_eq!(err.code(), 1109);
    }
}
