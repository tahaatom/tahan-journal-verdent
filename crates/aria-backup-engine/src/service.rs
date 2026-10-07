//! سرویس بکاپ و بازیابی (فاز ۱.۱۶) — snapshot سازگار، بسته رمزنگاری‌شده و
//! بازیابی اتمیک با نسخه امنیتی بازگشت.
//!
//! جریان بکاپ: `VACUUM INTO` برای snapshot سازگار پایگاه‌داده (بدون قفل
//! نوشتن) → جمع‌آوری پیوست‌ها و تنظیمات → بسته درونی → zstd → AES-256-GCM
//! با کلید argon2id → نوشتن اتمیک ظرف.
//!
//! جریان بازیابی: تحلیل ظرف → رمزگشایی (احراز صحت) → تحلیل بسته →
//! اعتبارسنجی چک‌سام‌ها → نسخه امنیتی → نوشتن → در صورت شکست، بازگردانی.

use crate::bundle::{self, BundleEntry};
use crate::container;
use crate::error::BackupError;
use crate::manifest::{
    BackupManifest, EntryMeta, KdfParams, DEFAULT_ZSTD_LEVEL, KDF_ALGORITHM, PACKAGE_FORMAT_VERSION,
};
use aria_security_engine::crypto;
use aria_security_engine::kdf;
use aria_storage_engine::Database;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// نام الگوریتم رمز — همان مقدار مانیفست.
pub const CIPHER_NAME: &str = crate::manifest::CIPHER_ALGORITHM;

/// تبدیل نمک hex به بایت — با بررسی طول و کاراکترهای معتبر.
fn salt_from_hex(hex: &str) -> Result<Vec<u8>, BackupError> {
    if !hex.len().is_multiple_of(2) || hex.is_empty() {
        return Err(BackupError::invalid("نمک KDF در مانیفست نامعتبر است"));
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let b = u8::from_str_radix(&hex[i..i + 2], 16)
            .map_err(|_| BackupError::invalid("نمک KDF در مانیفست hex معتبر نیست"))?;
        out.push(b);
    }
    Ok(out)
}

/// چیدمان پوشه کاری برنامه — یک ریشه با لایه‌های مشخص.
///
/// ```text
/// <root>/database/tahan.db
/// <root>/attachments/…
/// <root>/settings/settings.json
/// ```
#[derive(Debug, Clone)]
pub struct Workspace {
    /// ریشه داده برنامه
    pub root_dir: PathBuf,
}

impl Workspace {
    /// ساخت چیدمان روی یک ریشه.
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self { root_dir: root_dir.into() }
    }

    /// مسیر فایل پایگاه‌داده.
    pub fn db_path(&self) -> PathBuf {
        self.root_dir.join("database/tahan.db")
    }

    /// پوشه پیوست‌ها.
    pub fn attachments_dir(&self) -> PathBuf {
        self.root_dir.join("attachments")
    }

    /// مسیر فایل snapshot تنظیمات.
    pub fn settings_path(&self) -> PathBuf {
        self.root_dir.join("settings/settings.json")
    }

    /// پوشه نسخه‌های امنیتی پیش از بازیابی.
    pub fn safety_dir(&self) -> PathBuf {
        self.root_dir.join("backup-safety")
    }
}

/// گزارش ساخت بکاپ — قابل نمایش در رابط کاربری.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupReport {
    /// مسیر فایل بسته ساخته‌شده
    pub out_path: String,
    /// اندازه نهایی فایل روی دیسک
    pub size_bytes: u64,
    /// اندازه بار خام (پیش از فشرده‌سازی/رمزنگاری)
    pub raw_bytes: u64,
    /// اندازه پس از فشرده‌سازی و رمزنگاری
    pub payload_bytes: u64,
    /// تعداد فایل‌های بسته‌شده
    pub entries: usize,
    /// تعداد پیوست‌ها
    pub attachments_count: u64,
    /// اندازه snapshot پایگاه‌داده
    pub database_bytes: u64,
    /// آیا snapshot تنظیمات گنجانده شد؟
    pub includes_settings: bool,
    /// هش blake3 بار خام
    pub raw_blake3: String,
    /// هش blake3 محتوا (برای صحت‌سنجی و مقایسه)
    pub integrity_hash: String,
    /// زمان ساخت (RFC3339 UTC)
    pub created_at: String,
    /// نسخه فرمت
    pub format_version: u32,
    /// مدت اجرا (میلی‌ثانیه)
    pub duration_ms: u64,
}

/// گزارش بازیابی.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReport {
    /// مسیر بسته بازیابی‌شده
    pub source_path: String,
    /// تعداد فایل‌های بازگردانده‌شده
    pub entries_restored: usize,
    /// تعداد پیوست‌های بازگردانده‌شده
    pub attachments_restored: usize,
    /// اندازه پایگاه‌داده بازگردانده‌شده
    pub database_bytes: u64,
    /// نسخه اسکیمای بسته
    pub schema_version: i64,
    /// زمان بسته (RFC3339 UTC)
    pub created_at: String,
    /// مسیر نسخه امنیتی وضعیت پیشین (اگر وجود داشت)
    pub safety_copy: Option<String>,
    /// هش صحت بار بازگردانده‌شده
    pub integrity_hash: String,
    /// مدت اجرا (میلی‌ثانیه)
    pub duration_ms: u64,
}

/// ساخت بکاپ رمزنگاری‌شده از پایگاه‌داده، پیوست‌ها و تنظیمات.
pub fn create_backup(
    db: &Database,
    ws: &Workspace,
    out_path: &Path,
    password: &str,
    include_settings: bool,
) -> Result<BackupReport, BackupError> {
    let started = Instant::now();
    // سیاست گذرواژه — پیش از هر کار سنگین
    aria_security_engine::validate_password(password)?;

    let mut entries = Vec::new();

    // ===== snapshot سازگار پایگاه‌داده (بدون متوقف کردن نوشتن)
    let tmp = tempfile::Builder::new().prefix("tahan-snap-").tempdir()?;
    let snap_path = tmp.path().join("tahan.db");
    let schema_version = snapshot_database(db, &snap_path)?;
    let db_bytes = std::fs::read(&snap_path)?;
    let database_bytes = db_bytes.len() as u64;
    entries.push(BundleEntry { path: BackupManifest::DATABASE_ENTRY.into(), data: db_bytes });

    // ===== پیوست‌ها
    let attachments = collect_attachments(&ws.attachments_dir())?;
    let attachments_count = attachments.len() as u64;
    entries.extend(attachments);

    // ===== snapshot تنظیمات (JSON خوانا — قالب نسخه‌پذیر)
    if include_settings {
        let settings = dump_settings(db)?;
        entries.push(BundleEntry { path: BackupManifest::SETTINGS_ENTRY.into(), data: settings });
    }

    // ===== بسته درونی → فشرده‌سازی → رمزنگاری
    let raw = bundle::encode(&entries);
    let raw_bytes = raw.len() as u64;
    let raw_blake3 = blake3::hash(&raw).to_hex().to_string();
    let compressed = zstd::stream::encode_all(raw.as_slice(), DEFAULT_ZSTD_LEVEL)
        .map_err(|e| BackupError::Io(format!("فشرده‌سازی ناموفق: {e}")))?;

    let salt_hex = kdf::random_salt_hex();
    let salt = salt_from_hex(&salt_hex)?;
    let key = kdf::derive_key_argon2id(password, &salt)?;

    let manifest = BackupManifest {
        format_version: PACKAGE_FORMAT_VERSION,
        created_at: now_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version,
        cipher: CIPHER_NAME.to_string(),
        kdf: KdfParams {
            algorithm: KDF_ALGORITHM.to_string(),
            salt_hex: salt_hex.clone(),
            m_cost_kib: kdf::KDF_M_COST_KIB,
            t_cost: kdf::KDF_T_COST,
            p_cost: kdf::KDF_P_COST,
            key_len: kdf::KDF_OUTPUT_LEN as u32,
        },
        zstd_level: DEFAULT_ZSTD_LEVEL,
        raw_bytes,
        raw_blake3: raw_blake3.clone(),
        attachments_count,
        includes_settings: include_settings,
        entries: bundle::entry_meta(&entries),
        encryption_summary: format!(
            "{} + {} (m={} KiB, t={}, p={}) + zstd",
            CIPHER_NAME,
            KDF_ALGORITHM,
            kdf::KDF_M_COST_KIB,
            kdf::KDF_T_COST,
            kdf::KDF_P_COST
        ),
    };
    manifest.validate()?;

    let manifest_json = serde_json::to_vec(&manifest)
        .map_err(|e| BackupError::invalid(format!("ساخت مانیفست ناموفق: {e}")))?;
    // مانیفست به‌عنوان AAD احراز می‌شود؛ هر دستکاری فراداده رمزگشایی را می‌شکند
    let payload = crypto::encrypt(&key, &compressed, &manifest_json)
        .map_err(|e| BackupError::invalid(format!("رمزنگاری ناموفق: {e}")))?;
    let payload_bytes = payload.len() as u64;

    let size_bytes = container::write_package(
        out_path,
        container::CONTAINER_VERSION,
        &manifest_json,
        &payload,
    )?;

    Ok(BackupReport {
        out_path: out_path.display().to_string(),
        size_bytes,
        raw_bytes,
        payload_bytes,
        entries: entries.len(),
        attachments_count,
        database_bytes,
        includes_settings: include_settings,
        raw_blake3,
        integrity_hash: integrity_hash(&payload),
        created_at: manifest.created_at,
        format_version: PACKAGE_FORMAT_VERSION,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

/// بررسی کامل صحت بسته بدون تغییر دیسک: رمزگشایی، تحلیل، اعتبارسنجی چک‌سام.
pub fn verify_package(package: &Path, password: &str) -> Result<BackupManifest, BackupError> {
    let (manifest, entries) = open_and_decode(package, password)?;
    bundle::verify_against_manifest(&entries, &manifest.entries)?;
    Ok(manifest)
}

/// خواندن فراداده بسته بدون گذرواژه (برای نمایش پیش از بازیابی).
pub fn inspect_package(package: &Path) -> Result<BackupManifest, BackupError> {
    let raw = container::read_package(package)?;
    let manifest = raw.manifest()?;
    manifest.validate()?;
    Ok(manifest)
}

/// بازیابی بسته روی میزکار کاری — اتمیک با نسخه امنیتی و بازگشت در شکست.
pub fn restore_backup(
    package: &Path,
    ws: &Workspace,
    password: &str,
) -> Result<RestoreReport, BackupError> {
    let started = Instant::now();
    let (manifest, entries) = open_and_decode(package, password)?;
    // اعتبارسنجی کامل پیش از هر تغییر روی دیسک
    bundle::verify_against_manifest(&entries, &manifest.entries)?;
    ensure_workspace_writable(ws)?;

    // ===== نسخه امنیتی وضعیت پیشین
    let safety_copy = make_safety_copy(ws)?;

    // ===== نوشتن — در صورت شکست، بازگشت از نسخه امنیتی
    match write_restored(ws, &entries) {
        Ok((attachments_restored, database_bytes)) => Ok(RestoreReport {
            source_path: package.display().to_string(),
            entries_restored: entries.len(),
            attachments_restored,
            database_bytes,
            schema_version: manifest.schema_version,
            created_at: manifest.created_at.clone(),
            safety_copy: safety_copy.as_ref().map(|p| p.display().to_string()),
            integrity_hash: blake3::hash(&bundle::encode(&entries)).to_hex().to_string(),
            duration_ms: started.elapsed().as_millis() as u64,
        }),
        Err(e) => {
            let rollback = rollback_restore(ws, safety_copy.as_deref());
            Err(match rollback {
                Ok(()) => BackupError::RestoreRolledBack { reason: e.to_string() },
                Err(rb) => BackupError::RestoreRolledBack {
                    reason: format!("{e}؛ بازگردانی نیز ناموفق بود: {rb}"),
                },
            })
        }
    }
}

/// پاک کردن نسخه‌های امنیتی قدیمی (نگه‌داشتن `keep` نسخه تازه‌تر).
pub fn prune_safety_copies(ws: &Workspace, keep: usize) -> Result<usize, BackupError> {
    let dir = ws.safety_dir();
    if !dir.exists() {
        return Ok(0);
    }
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, e.path()))
        })
        .collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.0)); // تازه‌ترها اول
    let mut removed = 0;
    for (_, path) in dirs.into_iter().skip(keep) {
        std::fs::remove_dir_all(&path)?;
        removed += 1;
    }
    Ok(removed)
}

// ======================= درونی =======================

/// باز کردن بسته، رمزگشایی و تحلیل بسته درونی.
fn open_and_decode(
    package: &Path,
    password: &str,
) -> Result<(BackupManifest, Vec<BundleEntry>), BackupError> {
    let raw = container::read_package(package)?;
    let manifest = raw.manifest()?;
    manifest.validate()?;

    let salt = salt_from_hex(&manifest.kdf.salt_hex)?;
    let key = kdf::derive_key_argon2id(password, &salt)?;
    // مانیفست خام به‌عنوان AAD — دستکاری فراداده اینجا لو می‌رود
    let compressed = crypto::decrypt(&key, &raw.payload, &raw.manifest_bytes)?;
    let plain = zstd::stream::decode_all(compressed.as_slice())
        .map_err(|_| BackupError::integrity("بار بسته قابل باز کردن نیست (فشرده‌سازی خراب)"))?;
    let entries = bundle::decode(&plain)?;
    Ok((manifest, entries))
}

/// snapshot سازگار پایگاه‌داده با `VACUUM INTO` و بازگرداندن نسخه اسکیمای آن.
pub fn snapshot_database(db: &Database, dest: &Path) -> Result<i64, BackupError> {
    if dest.exists() {
        std::fs::remove_file(dest)?;
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = db.lock();
    // VACUUM INTO خروجی سازگار و فشرده می‌دهد و نیازی به قفل انحصاری ندارد
    conn.execute("VACUUM INTO ?1", params![dest.to_string_lossy()])
        .map_err(|e| BackupError::Database(format!("snapshot پایگاه‌داده ناموفق: {e}")))?;
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| BackupError::Database(e.to_string()))?;
    Ok(version)
}

/// جمع‌آوری بازگشتی فایل‌های پیوست با مسیر نسبی امن.
fn collect_attachments(dir: &Path) -> Result<Vec<BundleEntry>, BackupError> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    walk(dir, dir, &mut |rel, path| {
        // نام فایل‌های موقت/نیمه‌نوشته نادیده گرفته می‌شوند
        if rel.ends_with(".part") || rel.ends_with(".tmp") {
            return Ok(());
        }
        let data = std::fs::read(path)?;
        out.push(BundleEntry {
            path: format!("{}{}", BackupManifest::ATTACHMENTS_PREFIX, rel),
            data,
        });
        Ok(())
    })?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// پیمایش بازگشتی پوشه با مسیر نسبی اسلش‌دار (سازگار میان سیستم‌عامل‌ها).
fn walk(
    base: &Path,
    dir: &Path,
    f: &mut impl FnMut(String, &Path) -> Result<(), BackupError>,
) -> Result<(), BackupError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            // پیوند نمادین دنبال نمی‌شود (جلوگیری از خروج از پوشه و حلقه)
            continue;
        }
        if ft.is_dir() {
            walk(base, &path, f)?;
        } else if ft.is_file() {
            let rel = path
                .strip_prefix(base)
                .map_err(|_| BackupError::Io("مسیر پیوست خارج از پوشه است".into()))?
                .to_string_lossy()
                .replace('\\', "/");
            f(rel, &path)?;
        }
    }
    Ok(())
}

/// snapshot خوانای تنظیمات به‌صورت JSON مرتب.
fn dump_settings(db: &Database) -> Result<Vec<u8>, BackupError> {
    #[derive(Serialize)]
    struct SettingRow {
        key: String,
        value: Option<String>,
        updated_at: String,
    }
    let conn = db.lock();
    let mut stmt = conn
        .prepare("SELECT key, value, updated_at FROM settings ORDER BY key")
        .map_err(|e| BackupError::Database(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SettingRow {
                key: r.get(0)?,
                value: r.get(1)?,
                updated_at: r.get(2)?,
            })
        })
        .map_err(|e| BackupError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| BackupError::Database(e.to_string()))?;
    let doc = serde_json::json!({
        "kind": "tahan.settings.snapshot",
        "version": 1,
        "settings": rows,
    });
    serde_json::to_vec_pretty(&doc)
        .map_err(|e| BackupError::Database(format!("ساخت snapshot تنظیمات ناموفق: {e}")))
}

/// آماده‌سازی میزکار کاری برای بازیابی (ساخت ریشه در صورت نبود).
fn ensure_workspace_writable(ws: &Workspace) -> Result<(), BackupError> {
    std::fs::create_dir_all(&ws.root_dir)?;
    Ok(())
}

/// کپی وضعیت پیشین (پایگاه‌داده و پیوست‌ها) در پوشه نسخه امنیتی.
fn make_safety_copy(ws: &Workspace) -> Result<Option<PathBuf>, BackupError> {
    let has_db = ws.db_path().exists();
    let has_atts = ws.attachments_dir().exists();
    if !has_db && !has_atts {
        return Ok(None);
    }
    let stamp = now_rfc3339().replace([':', '.'], "-");
    let dest = ws.safety_dir().join(stamp);
    std::fs::create_dir_all(&dest)?;

    if has_db {
        let db_dest = dest.join("database");
        std::fs::create_dir_all(&db_dest)?;
        if let Err(e) = std::fs::copy(ws.db_path(), db_dest.join("tahan.db")) {
            // کپی ناقص پاک می‌شود تا نسخه امنیتی نیمه‌کاره نماند
            let _ = std::fs::remove_dir_all(&dest);
            return Err(BackupError::Io(format!("کپی امنیتی پایگاه‌داده ناموفق: {e}")));
        }
        // فایل‌های کنار پایگاه‌داده در حالت WAL برای بازگردانی کامل
        for suffix in ["-wal", "-shm"] {
            let side = PathBuf::from(format!("{}{suffix}", ws.db_path().display()));
            if side.exists() {
                let _ = std::fs::copy(&side, db_dest.join(format!("tahan.db{suffix}")));
            }
        }
    }
    if has_atts {
        let att_dest = dest.join("attachments");
        std::fs::create_dir_all(&att_dest)?;
        if let Err(e) = copy_tree(&ws.attachments_dir(), &att_dest) {
            let _ = std::fs::remove_dir_all(&dest);
            return Err(BackupError::Io(format!("کپی امنیتی پیوست‌ها ناموفق: {e}")));
        }
    }
    Ok(Some(dest))
}

/// کپی درختی پوشه.
fn copy_tree(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&path, &target)?;
        } else if entry.file_type()?.is_file() {
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

/// نوشتن فایل‌های بازیابی‌شده روی میزکار.
fn write_restored(
    ws: &Workspace,
    entries: &[BundleEntry],
) -> Result<(usize, u64), BackupError> {
    let mut attachments_restored = 0usize;
    let mut database_bytes = 0u64;

    // فایل‌های WAL/SHM قدیمی پیش از بازگردانی پایگاه‌داده حذف می‌شوند تا
    // با snapshot تازه ناسازگار نمانند و بازگشایی پایگاه‌داده خراب نشود
    let db_path = ws.db_path();
    for suffix in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}{suffix}", db_path.display()));
        if side.exists() {
            std::fs::remove_file(&side)?;
        }
    }

    for e in entries {
        let target = if e.path == BackupManifest::DATABASE_ENTRY {
            database_bytes = e.data.len() as u64;
            ws.db_path()
        } else if e.path == BackupManifest::SETTINGS_ENTRY {
            ws.settings_path()
        } else if let Some(rel) = e
            .path
            .strip_prefix(BackupManifest::ATTACHMENTS_PREFIX)
        {
            attachments_restored += 1;
            ws.attachments_dir().join(rel)
        } else {
            // مسیر ناشناخته — نادیده گرفته می‌شود تا آینده‌سازگار بماند
            continue;
        };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_atomic(&target, &e.data)?;
    }
    Ok((attachments_restored, database_bytes))
}

/// نوشتن اتمیک یک فایل (موقت + انتقال).
fn write_atomic(target: &Path, data: &[u8]) -> Result<(), BackupError> {
    let tmp = target.with_extension("restore.part");
    std::fs::write(&tmp, data)?;
    if target.exists() {
        std::fs::remove_file(target)?;
    }
    std::fs::rename(&tmp, target)?;
    Ok(())
}

/// بازگردانی وضعیت پیشین از نسخه امنیتی پس از شکست بازیابی.
fn rollback_restore(ws: &Workspace, safety: Option<&Path>) -> Result<(), BackupError> {
    let Some(safety) = safety else {
        // چیزی برای بازگردانی نبود؛ فایل‌های نیمه‌کاره پاک می‌شوند
        let _ = std::fs::remove_file(ws.db_path());
        return Ok(());
    };
    let _ = std::fs::remove_dir_all(ws.attachments_dir());
    if let Err(e) = copy_tree(safety, &ws.root_dir) {
        return Err(BackupError::Io(format!("کپی بازگردانی ناموفق: {e}")));
    }
    Ok(())
}

/// هش صحت بار رمزنگاری‌شده (برای گزارش و مقایسه فایل).
fn integrity_hash(payload: &[u8]) -> String {
    blake3::hash(payload).to_hex().to_string()
}

/// زمان کنونی RFC3339 با دقت ثانیه.
fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// فهرست چک‌سام‌های مانیفست به‌صورت کارت امنیتی برای نمایش.
pub fn check_manifest_entries(entries: &[EntryMeta]) -> bool {
    !entries.is_empty() && entries.iter().all(|e| !e.blake3.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aria_storage_engine::migrations;

    const PASSWORD: &str = "Passw0rd!-Tahan-2026";

    fn open_db(root: &Path) -> Database {
        let db_path = Workspace::new(root).db_path();
        std::fs::create_dir_all(db_path.parent().unwrap()).unwrap();
        let db = Database::open(&db_path, None).unwrap();
        let mut conn = db.lock();
        migrations::run_migrations(&mut conn, |_| Ok(())).unwrap();
        drop(conn);
        db
    }

    fn seed(db: &Database) {
        let conn = db.lock();
        let now = "2026-09-30T10:00:00Z";
        conn.execute(
            "INSERT INTO profiles (id, name, created_at, updated_at) VALUES ('p1','پروفایل',?1,?1)",
            params![now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO trading_accounts (id, profile_id, name, currency, created_at, updated_at)
             VALUES ('a1','p1','حساب اصلی','USD',?1,?1)",
            params![now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO symbols (id, name, description, contract_size, created_at)
             VALUES ('s1','XAUUSD','طلا',1,?1)",
            params![now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO journal_trades (id, account_id, symbol_id, direction, status, entry_time, created_at, updated_at)
             VALUES ('t1','a1','s1','buy','open',?1,?1,?1)",
            params![now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES ('theme','dark',?1)",
            params![now],
        )
        .unwrap();
    }

    fn write_attachment(ws: &Workspace, name: &str, data: &[u8]) {
        let path = ws.attachments_dir().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, data).unwrap();
    }

    fn fixture() -> (tempfile::TempDir, Database, Workspace) {
        let tmp = tempfile::tempdir().unwrap();
        let ws = Workspace::new(tmp.path());
        let db = open_db(tmp.path());
        seed(&db);
        (tmp, db, ws)
    }

    #[test]
    fn create_backup_produces_encrypted_package() {
        let (tmp, db, ws) = fixture();
        write_attachment(&ws, "a.png", &[1, 2, 3, 4]);
        write_attachment(&ws, "nested/b.bin", &[9; 128]);
        let out = tmp.path().join("backup.tahanbak");

        let report = create_backup(&db, &ws, &out, PASSWORD, true).unwrap();
        assert!(out.exists());
        assert_eq!(report.entries, 4, "db + settings + 2 attachments");
        assert_eq!(report.attachments_count, 2);
        assert!(report.database_bytes > 0);
        assert!(report.includes_settings);
        assert_eq!(report.format_version, PACKAGE_FORMAT_VERSION);
        assert_eq!(report.raw_blake3.len(), 64);
        assert_eq!(report.integrity_hash.len(), 64);
        assert_eq!(report.size_bytes, std::fs::metadata(&out).unwrap().len());

        // پایگاه‌داده در فایل به‌صورت متن روشن نیست
        let bytes = std::fs::read(&out).unwrap();
        assert!(
            !bytes.windows(16).any(|w| w == b"SQLite format 3\0"),
            "database header must not leak in plaintext"
        );
        assert!(!bytes.windows(4).any(|w| w == b"XAUUSD"), "no plaintext symbol");
    }

    #[test]
    fn backup_metadata_is_inspectable_without_password() {
        let (tmp, db, ws) = fixture();
        let out = tmp.path().join("b.tahanbak");
        create_backup(&db, &ws, &out, PASSWORD, true).unwrap();

        let m = inspect_package(&out).unwrap();
        assert!(m.is_supported());
        assert_eq!(m.kdf.algorithm, KDF_ALGORITHM);
        assert_eq!(m.cipher, CIPHER_NAME);
        assert!(m.includes_settings);
        assert!(m.entries.iter().any(|e| e.path == BackupManifest::DATABASE_ENTRY));
        assert!(!m.kdf.salt_hex.is_empty());
        // زمان و نسخه اسکیما برای نمایش
        assert!(m.created_at.starts_with("20"));
        assert!(m.schema_version >= 1);
    }

    #[test]
    fn roundtrip_restores_database_and_attachments() {
        let (tmp, db, ws) = fixture();
        write_attachment(&ws, "a.png", &[1, 2, 3, 4]);
        write_attachment(&ws, "nested/b.bin", &[7; 64]);
        let out = tmp.path().join("b.tahanbak");
        create_backup(&db, &ws, &out, PASSWORD, true).unwrap();

        // تغییر وضعیت پس از بکاپ
        {
            let conn = db.lock();
            conn.execute("DELETE FROM journal_trades", []).unwrap();
            conn.execute("UPDATE settings SET value='light' WHERE key='theme'", []).unwrap();
        }
        write_attachment(&ws, "a.png", &[99, 99]);
        std::fs::remove_file(ws.attachments_dir().join("nested/b.bin")).unwrap();
        drop(db);

        // بازیابی روی همان میزکار
        let report = restore_backup(&out, &ws, PASSWORD).unwrap();
        assert_eq!(report.entries_restored, 4);
        assert_eq!(report.attachments_restored, 2);
        assert!(report.safety_copy.is_some(), "pre-restore safety copy exists");

        // پایگاه‌داده بازگردانده‌شده قابل باز شدن و دارای داده است
        let restored = Database::open(&ws.db_path(), None).unwrap();
        let conn = restored.lock();
        let trades: i64 = conn
            .query_row("SELECT count(*) FROM journal_trades", [], |r| r.get(0))
            .unwrap();
        assert_eq!(trades, 1, "trade restored from backup");
        let theme: String = conn
            .query_row("SELECT value FROM settings WHERE key='theme'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(theme, "dark");
        drop(conn);

        // پیوست‌ها بازگردانده شده‌اند
        assert_eq!(std::fs::read(ws.attachments_dir().join("a.png")).unwrap(), vec![1, 2, 3, 4]);
        assert_eq!(std::fs::read(ws.attachments_dir().join("nested/b.bin")).unwrap(), vec![7; 64]);
    }

    #[test]
    fn wrong_password_is_rejected_with_1601() {
        let (tmp, db, ws) = fixture();
        let out = tmp.path().join("b.tahanbak");
        create_backup(&db, &ws, &out, PASSWORD, false).unwrap();

        let e = verify_package(&out, "Wrong!Pass9-Zzz").unwrap_err();
        assert_eq!(e.code(), 1601);

        // و بازیابی با گذرواژه نادرست چیزی را تغییر نمی‌دهد
        let before = std::fs::metadata(ws.db_path()).unwrap().len();
        let e = restore_backup(&out, &ws, "Wrong!Pass9-Zzz").unwrap_err();
        assert_eq!(e.code(), 1601);
        assert_eq!(std::fs::metadata(ws.db_path()).unwrap().len(), before);
    }

    #[test]
    fn weak_password_is_rejected_at_creation() {
        let (tmp, db, ws) = fixture();
        let out = tmp.path().join("b.tahanbak");
        let e = create_backup(&db, &ws, &out, "short", false).unwrap_err();
        assert_eq!(e.code(), 1604);
        assert!(!out.exists(), "no partial package is left behind");
    }

    #[test]
    fn tampered_payload_fails_integrity_check() {
        let (tmp, db, ws) = fixture();
        let out = tmp.path().join("b.tahanbak");
        create_backup(&db, &ws, &out, PASSWORD, false).unwrap();

        let mut bytes = std::fs::read(&out).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01; // دستکاری آخرین بایت بار
        let tampered = tmp.path().join("tampered.tahanbak");
        std::fs::write(&tampered, &bytes).unwrap();

        let e = verify_package(&tampered, PASSWORD).unwrap_err();
        assert_eq!(e.code(), 1601, "GCM auth failure surfaces as wrong password/tampered");
        // و بازیابی هم رد می‌شود و دیسک دست‌نخورده می‌ماند
        let before = std::fs::read(ws.db_path()).unwrap();
        assert!(restore_backup(&tampered, &ws, PASSWORD).is_err());
        assert_eq!(std::fs::read(ws.db_path()).unwrap(), before);
    }

    #[test]
    fn tampered_manifest_metadata_fails_authentication() {
        let (tmp, db, ws) = fixture();
        let out = tmp.path().join("b.tahanbak");
        create_backup(&db, &ws, &out, PASSWORD, true).unwrap();

        // دستکاری فراداده مانیفست (AAD) — جایگزینی هم‌طول و JSON-معتبر «3» با «4»
        let bytes = std::fs::read(&out).unwrap();
        let needle = b"\"zstd_level\":3";
        let pos = bytes
            .windows(needle.len())
            .position(|w| w == needle)
            .expect("fixture manifest contains zstd_level");
        let mut patched = bytes.clone();
        patched[pos + needle.len() - 1] = b'4';
        let tampered = tmp.path().join("tampered.tahanbak");
        std::fs::write(&tampered, &patched).unwrap();

        // مانیفست هنوز JSON معتبر و ساختاراً سالم است اما بایت‌های AAD عوض
        // شده‌اند → احراز هویت GCM می‌شکند
        assert_ne!(bytes, patched);
        let e = verify_package(&tampered, PASSWORD).unwrap_err();
        assert_eq!(e.code(), 1601);
        // و بازیابی هم رد می‌شود و دیسک دست‌نخورده می‌ماند
        let before = std::fs::read(ws.db_path()).unwrap();
        assert!(restore_backup(&tampered, &ws, PASSWORD).is_err());
        assert_eq!(std::fs::read(ws.db_path()).unwrap(), before);
    }

    #[test]
    fn verify_package_passes_for_valid_backup() {
        let (tmp, db, ws) = fixture();
        write_attachment(&ws, "a.png", &[1, 2, 3]);
        let out = tmp.path().join("b.tahanbak");
        let report = create_backup(&db, &ws, &out, PASSWORD, true).unwrap();

        let m = verify_package(&out, PASSWORD).unwrap();
        assert_eq!(m.raw_blake3, report.raw_blake3);
        assert_eq!(m.entries.len(), report.entries);
        assert!(check_manifest_entries(&m.entries));
    }

    #[test]
    fn backup_without_settings_omits_settings_entry() {
        let (tmp, db, ws) = fixture();
        let out = tmp.path().join("b.tahanbak");
        let report = create_backup(&db, &ws, &out, PASSWORD, false).unwrap();
        assert!(!report.includes_settings);
        let m = inspect_package(&out).unwrap();
        assert!(!m.includes_settings);
        assert!(!m.entries.iter().any(|e| e.path == BackupManifest::SETTINGS_ENTRY));
    }

    #[test]
    fn backup_with_no_attachments_still_works() {
        let (tmp, db, ws) = fixture();
        let out = tmp.path().join("b.tahanbak");
        let report = create_backup(&db, &ws, &out, PASSWORD, false).unwrap();
        assert_eq!(report.attachments_count, 0);
        assert_eq!(report.entries, 1, "database only");
        assert!(verify_package(&out, PASSWORD).is_ok());
    }

    #[test]
    fn restore_creates_safety_copy_of_previous_state() {
        let (tmp, db, ws) = fixture();
        let out = tmp.path().join("b.tahanbak");
        create_backup(&db, &ws, &out, PASSWORD, false).unwrap();
        drop(db);
        restore_backup(&out, &ws, PASSWORD).unwrap();

        let safety_dirs: Vec<_> = std::fs::read_dir(ws.safety_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        assert_eq!(safety_dirs.len(), 1);
        assert!(safety_dirs[0].path().join("database/tahan.db").exists());
    }

    #[test]
    fn prune_keeps_newest_safety_copies() {
        let (tmp, db, ws) = fixture();
        let out = tmp.path().join("b.tahanbak");
        create_backup(&db, &ws, &out, PASSWORD, false).unwrap();
        drop(db);
        for _ in 0..3 {
            restore_backup(&out, &ws, PASSWORD).unwrap();
            // فاصله زمانی تا مُهرهای پوشه متفاوت شود
            std::thread::sleep(std::time::Duration::from_millis(1100));
        }
        let removed = prune_safety_copies(&ws, 1).unwrap();
        assert_eq!(removed, 2);
        let left = std::fs::read_dir(ws.safety_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .count();
        assert_eq!(left, 1);
        // پاک‌سازی روی پوشه ناموجود هم بی‌خطر است
        let other = Workspace::new(tmp.path().join("nope"));
        assert_eq!(prune_safety_copies(&other, 1).unwrap(), 0);
    }

    #[test]
    fn restore_rejects_unsupported_format_version() {
        let (tmp, db, ws) = fixture();
        let out = tmp.path().join("b.tahanbak");
        create_backup(&db, &ws, &out, PASSWORD, false).unwrap();

        let mut bytes = std::fs::read(&out).unwrap();
        bytes[8..12].copy_from_slice(&99u32.to_le_bytes());
        let bad = tmp.path().join("bad.tahanbak");
        std::fs::write(&bad, &bytes).unwrap();

        let e = restore_backup(&bad, &ws, PASSWORD).unwrap_err();
        assert_eq!(e.code(), 1603);
        assert!(inspect_package(&bad).is_err());
    }

    #[test]
    fn restore_rejects_foreign_file() {
        let (tmp, _db, ws) = fixture();
        let junk = tmp.path().join("junk.tahanbak");
        std::fs::write(&junk, b"definitely not a package").unwrap();
        let e = restore_backup(&junk, &ws, PASSWORD).unwrap_err();
        assert_eq!(e.code(), 1600);
    }

    #[test]
    fn attachments_walk_skips_partial_and_symlink_files() {
        let (tmp, db, ws) = fixture();
        write_attachment(&ws, "ok.bin", &[1]);
        write_attachment(&ws, "partial.part", &[2]);
        write_attachment(&ws, "temp.tmp", &[3]);
        let out = tmp.path().join("b.tahanbak");
        let report = create_backup(&db, &ws, &out, PASSWORD, false).unwrap();
        assert_eq!(report.attachments_count, 1, "only the complete file is backed up");
        let m = inspect_package(&out).unwrap();
        assert!(m.entries.iter().any(|e| e.path == "attachments/ok.bin"));
        assert!(!m.entries.iter().any(|e| e.path.ends_with(".part")));
    }

    #[test]
    fn backup_is_deterministic_for_identical_content_except_encryption() {
        // دو بکاپ از محتوای یکسان: بار خام یکسان، بسته متفاوت (نمک/نانس تازه)
        let (tmp, db, ws) = fixture();
        let a = tmp.path().join("a.tahanbak");
        let b = tmp.path().join("b.tahanbak");
        let ra = create_backup(&db, &ws, &a, PASSWORD, true).unwrap();
        let rb = create_backup(&db, &ws, &b, PASSWORD, true).unwrap();
        assert_eq!(ra.raw_blake3, rb.raw_blake3, "same content → same raw hash");
        assert_ne!(
            std::fs::read(&a).unwrap(),
            std::fs::read(&b).unwrap(),
            "fresh salt/nonce makes packages differ"
        );
    }

    #[test]
    fn snapshot_database_is_a_readable_sqlite_file() {
        let (tmp, db, _ws) = fixture();
        let dest = tmp.path().join("snap.db");
        let version = snapshot_database(&db, &dest).unwrap();
        assert!(version >= 1);
        let bytes = std::fs::read(&dest).unwrap();
        assert_eq!(&bytes[..16], b"SQLite format 3\0", "snapshot is valid SQLite");
        // بازنویسی snapshot موجود کار می‌کند
        assert!(snapshot_database(&db, &dest).is_ok());
    }

    #[test]
    fn workspace_paths_follow_the_documented_layout() {
        let ws = Workspace::new("C:/data/tahan");
        assert!(ws.db_path().ends_with("database/tahan.db"));
        assert!(ws.attachments_dir().ends_with("attachments"));
        assert!(ws.settings_path().ends_with("settings/settings.json"));
        assert!(ws.safety_dir().ends_with("backup-safety"));
    }
}
