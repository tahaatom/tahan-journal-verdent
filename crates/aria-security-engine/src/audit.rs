//! پایه حسابرسی — ثبت رویدادهای حساس در جدول audit_logs.
//!
//! قواعد: جزئیات بدون داده حساس (مقادیر مالی کامل یا گذرواژه هرگز ثبت نمی‌شوند).
//! پیاده‌سازی قرارداد `aria_contracts::AuditProvider`.

use crate::error::SecurityError;
use aria_contracts::traits::AuditProvider;
use aria_storage_engine::Database;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// رکورد حسابرسی.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AuditEntry {
    pub id: String,
    pub action: String,
    pub actor: String,
    pub target: Option<String>,
    pub detail: Option<String>,
    pub created_at: String,
}

/// زمان فعلی ISO UTC.
pub(crate) fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

/// ثبت رویداد حساس — خطا هرگز نباید عمل اصلی را متوقف کند،
/// بنابراین این تابع خطای حسابرسی را جداگانه گزارش می‌کند.
pub fn record_event(
    db: &Database,
    action: &str,
    actor: &str,
    target: Option<&str>,
    detail: Option<&serde_json::Value>,
) -> Result<String, SecurityError> {
    if action.trim().is_empty() || actor.trim().is_empty() {
        return Err(SecurityError::audit("action and actor are required"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let detail_json = detail.map(|d| d.to_string());
    db.lock()
        .execute(
            "INSERT INTO audit_logs (id, action, actor, target, detail, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, action.trim(), actor.trim(), target, detail_json, now_iso()],
        )
        .map_err(|e| SecurityError::audit(e.to_string()))?;
    tracing::info!(action = action, actor = actor, "audit");
    Ok(id)
}

/// خواندن آخرین رویدادهای حسابرسی.
pub fn read_recent(db: &Database, limit: usize) -> Result<Vec<AuditEntry>, SecurityError> {
    let limit = limit.clamp(1, 1000) as i64;
    let conn = db.lock();
    let mut stmt = conn
        .prepare_cached(
            "SELECT id, action, actor, target, detail, created_at
             FROM audit_logs ORDER BY created_at DESC, id DESC LIMIT ?1",
        )
        .map_err(|e| SecurityError::audit(e.to_string()))?;
    let rows = stmt
        .query_map([limit], |r| {
            Ok(AuditEntry {
                id: r.get(0)?,
                action: r.get(1)?,
                actor: r.get(2)?,
                target: r.get(3)?,
                detail: r.get(4)?,
                created_at: r.get(5)?,
            })
        })
        .map_err(|e| SecurityError::audit(e.to_string()))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| SecurityError::audit(e.to_string()))
}

/// شمارش رویدادهای یک اقدام خاص (مثل شمارش ورودهای ناموفق).
pub fn count_action(db: &Database, action: &str) -> Result<i64, SecurityError> {
    db.lock()
        .query_row(
            "SELECT count(*) FROM audit_logs WHERE action = ?1",
            [action],
            |r| r.get(0),
        )
        .map_err(|e| SecurityError::audit(e.to_string()))
}

/// شمارش رویدادهای یک اقدام از نقطه زمانی مشخص به بعد (مقایسه لغوی ISO UTC).
/// برای محدودسازی نرخ تلاش ورود (پنجره زمانی) استفاده می‌شود.
pub fn count_action_since(
    db: &Database,
    action: &str,
    since_iso: &str,
) -> Result<i64, SecurityError> {
    db.lock()
        .query_row(
            "SELECT count(*) FROM audit_logs WHERE action = ?1 AND created_at >= ?2",
            rusqlite::params![action, since_iso],
            |r| r.get(0),
        )
        .map_err(|e| SecurityError::audit(e.to_string()))
}

/// پل به قرارداد عمومی کرنل — سازگاری با `aria_contracts::AuditProvider`.
pub struct ContractAuditBridge<'a> {
    pub db: &'a Database,
}

impl AuditProvider for ContractAuditBridge<'_> {
    fn record(
        &self,
        action: &str,
        actor: &str,
        target: &str,
        detail: Option<serde_json::Value>,
    ) -> Result<(), aria_contracts::ContractError> {
        record_event(self.db, action, actor, Some(target), detail.as_ref())
            .map(|_| ())
            .map_err(|e| aria_contracts::ContractError::Violation { reason: e.to_string() })
    }

    fn read_recent(&self, limit: usize) -> Result<serde_json::Value, aria_contracts::ContractError> {
        let entries = read_recent(self.db, limit)
            .map_err(|e| aria_contracts::ContractError::Violation { reason: e.to_string() })?;
        serde_json::to_value(entries)
            .map_err(|e| aria_contracts::ContractError::Violation { reason: e.to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aria_storage_engine::migrations::run_migrations;

    fn setup_db() -> Database {
        let db = Database::open_memory(None).unwrap();
        {
            let mut conn = db.lock();
            run_migrations(&mut conn, |_| Ok(())).unwrap();
        }
        db
    }

    #[test]
    fn record_and_read_recent() {
        let db = setup_db();
        for i in 0..5 {
            record_event(&db, "login", "ui", Some("profile-1"), Some(&serde_json::json!({"i": i})))
                .unwrap();
        }
        let entries = read_recent(&db, 3).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().all(|e| e.action == "login"));
        assert!(entries[0].detail.as_ref().unwrap().contains("\"i\""));
    }

    #[test]
    fn empty_action_or_actor_rejected() {
        let db = setup_db();
        assert!(matches!(
            record_event(&db, "  ", "ui", None, None),
            Err(SecurityError::Audit { .. })
        ));
        assert!(matches!(
            record_event(&db, "login", "", None, None),
            Err(SecurityError::Audit { .. })
        ));
    }

    #[test]
    fn count_action_counts_by_action() {
        let db = setup_db();
        record_event(&db, "failed_login", "ui", None, None).unwrap();
        record_event(&db, "failed_login", "ui", None, None).unwrap();
        record_event(&db, "login", "ui", None, None).unwrap();
        assert_eq!(count_action(&db, "failed_login").unwrap(), 2);
        assert_eq!(count_action(&db, "login").unwrap(), 1);
        assert_eq!(count_action(&db, "nonexistent").unwrap(), 0);
    }

    #[test]
    fn count_action_since_filters_by_window() {
        let db = setup_db();
        record_event(&db, "failed_login", "ui", None, None).unwrap();
        record_event(&db, "failed_login", "ui", None, None).unwrap();
        // پنجره‌ای که کل گذشته را می‌پوشاند → همه شمرده می‌شوند
        let all_time = "2000-01-01T00:00:00Z";
        assert_eq!(count_action_since(&db, "failed_login", all_time).unwrap(), 2);
        // پنجره‌ای در آینده → هیچ
        assert_eq!(count_action_since(&db, "failed_login", "2999-01-01T00:00:00Z").unwrap(), 0);
    }

    #[test]
    fn detail_json_serialized() {
        let db = setup_db();
        record_event(
            &db,
            "backup_create",
            "ui",
            Some("backup-9"),
            Some(&serde_json::json!({"path": "/x", "size": 123})),
        )
        .unwrap();
        let e = &read_recent(&db, 1).unwrap()[0];
        assert!(e.detail.as_ref().unwrap().contains("/x"));
        assert_eq!(e.action, "backup_create");
    }

    #[test]
    fn contract_bridge_roundtrip() {
        let db = setup_db();
        let bridge = ContractAuditBridge { db: &db };
        bridge.record("unlock", "ui", "p1", None).unwrap();
        bridge.record("lock", "ui", "p1", None).unwrap();
        let recent = bridge.read_recent(10).unwrap();
        assert_eq!(recent.as_array().unwrap().len(), 2);
    }
}
