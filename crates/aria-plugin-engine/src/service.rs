//! سرویس موتور پلاگین — رجیستری، چرخه حیات، قرنطینه، مجوز و سلامت.
//!
//! قواعد پیاده‌سازی:
//! - هیچ پلاگینی هرگز به پایگاه‌داده دسترسی مستقیم ندارد؛ تنها مجرای داده،
//!   مجوزهای مانیفست از طریق `enforce` است.
//! - چرخه حیات: Install → validate → check permissions → user approval →
//!   start → monitor → crash handling → quarantine/disable.
//! - گذارهای وضعیت سخت‌گیرانه و مبتنی بر جدول گذار هستند (خطای ۱۵۰۷).
//! - کرش متوالی بیشتر از آستانه (پیش‌فرض ۳) → قرنطینه؛ فعال‌سازی مجدد فقط دستی.
//! - رویدادها داخل همان تراکنش در outbox نوشته می‌شوند.

use crate::error::PluginError;
use crate::events::PluginEventFactory;
use crate::manifest::{PluginManifest, ResourcePolicy};
use crate::permissions::{Capability, PermissionSet};
use crate::semver::{SemVer, VersionRange};
use aria_contracts::{EventEnvelope, EventSource};
use aria_storage_engine::Database;
use rusqlite::{params, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// وضعیت پلاگین — منطبق با CHECK ستون status در جدول plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginStatus {
    Installed,
    Enabled,
    Running,
    Stopped,
    Crashed,
    Quarantined,
    Disabled,
}

impl PluginStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::Enabled => "enabled",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Crashed => "crashed",
            Self::Quarantined => "quarantined",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "installed" => Self::Installed,
            "enabled" => Self::Enabled,
            "running" => Self::Running,
            "stopped" => Self::Stopped,
            "crashed" => Self::Crashed,
            "quarantined" => Self::Quarantined,
            "disabled" => Self::Disabled,
            _ => return None,
        })
    }

    /// گذارهای مجاز از این وضعیت.
    fn allowed_transitions(&self) -> &'static [PluginStatus] {
        match self {
            Self::Installed => &[PluginStatus::Enabled, PluginStatus::Disabled],
            Self::Enabled => &[PluginStatus::Running, PluginStatus::Disabled],
            Self::Running => &[
                PluginStatus::Stopped,
                PluginStatus::Crashed,
                PluginStatus::Disabled,
            ],
            Self::Stopped => &[PluginStatus::Running, PluginStatus::Disabled],
            Self::Crashed => &[
                PluginStatus::Running,
                // حلقه خودی: کرش متوالی بدون راه‌اندازی موفق مجدداً گزارش می‌شود
                PluginStatus::Crashed,
                PluginStatus::Quarantined,
                PluginStatus::Disabled,
            ],
            Self::Quarantined => &[PluginStatus::Disabled],
            Self::Disabled => &[PluginStatus::Enabled],
        }
    }

    fn can_transition_to(&self, target: PluginStatus) -> bool {
        self.allowed_transitions().contains(&target)
    }
}

/// رکورد پلاگین در رجیستری.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PluginRecord {
    pub id: String,
    pub status: PluginStatus,
    pub crash_count: i64,
    pub last_error: Option<String>,
    pub installed_at: String,
    pub updated_at: String,
}

/// وضعیت سلامت — قابل مشاهده برای UI از طریق قرارداد.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PluginHealth {
    pub id: String,
    pub status: PluginStatus,
    pub crash_count: i64,
    pub last_error: Option<String>,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// سرویس موتور پلاگین.
pub struct PluginService<'a> {
    db: &'a Database,
    kernel_version: SemVer,
    api_version: SemVer,
}

impl<'a> PluginService<'a> {
    /// ساخت سرویس با نسخه کرنل و API جاری (برای بررسی سازگاری مانیفست‌ها).
    pub fn new(db: &'a Database, kernel_version: &str, api_version: &str) -> Result<Self, PluginError> {
        Ok(Self {
            db,
            kernel_version: SemVer::parse(kernel_version)
                .map_err(|e| PluginError::storage(format!("bad kernel version: {e}")))?,
            api_version: SemVer::parse(api_version)
                .map_err(|e| PluginError::storage(format!("bad api version: {e}")))?,
        })
    }

    // ==================== نصب ====================

    /// نصب پلاگین: اعتبارسنجی مانیفست → بررسی سازگاری نسخه → ثبت با وضعیت installed.
    pub fn install(&self, manifest_json: &str) -> Result<PluginManifest, PluginError> {
        let manifest = PluginManifest::parse(manifest_json)?;
        let mut guard = self.db.lock();
        let tx = guard
            .transaction()
            .map_err(|e| PluginError::storage(e.to_string()))?;
        let result = self.install_tx(&tx, &manifest)?;
        tx.commit().map_err(|e| PluginError::storage(e.to_string()))?;
        Ok(result)
    }

    fn install_tx(&self, tx: &Transaction<'_>, manifest: &PluginManifest) -> Result<PluginManifest, PluginError> {
        if self.exists_tx(tx, &manifest.id)? {
            return Err(PluginError::duplicate(&manifest.id));
        }
        // سازگاری نسخه کرنل
        let kernel_range = VersionRange::parse(&manifest.kernel_version_range)
            .map_err(|e| PluginError::manifest_invalid(format!("kernel_version_range: {e}")))?;
        if !kernel_range.matches(&self.kernel_version) {
            return Err(PluginError::incompatible(format!(
                "kernel {} not in range {}",
                self.kernel_version,
                kernel_range.as_str()
            )));
        }
        // سازگاری نسخه API
        let api_range = VersionRange::parse(&manifest.api_version_range)
            .map_err(|e| PluginError::manifest_invalid(format!("api_version_range: {e}")))?;
        if !api_range.matches(&self.api_version) {
            return Err(PluginError::incompatible(format!(
                "api {} not in range {}",
                self.api_version,
                api_range.as_str()
            )));
        }
        let now = now_iso();
        tx.execute(
            "INSERT INTO plugins (id, manifest_json, status, crash_count, last_error,
             installed_at, updated_at) VALUES (?1,?2,'installed',0,NULL,?3,?3)",
            params![manifest.id, serde_json::to_string(manifest).unwrap(), now],
        )?;
        let factory = PluginEventFactory::new(Uuid::new_v4());
        self.write_event(tx, &factory.installed(&manifest.id))?;
        Ok(manifest.clone())
    }

    // ==================== چرخه حیات ====================

    /// تأیید و فعال‌سازی کاربر — installed|disabled → enabled.
    pub fn enable(&self, plugin_id: &str) -> Result<(), PluginError> {
        self.transition(plugin_id, PluginStatus::Enabled, "enable")
    }

    /// راه‌اندازی — enabled|stopped|crashed → running.
    /// اگر پلاگین به network.access نیاز داشته باشد و کاربر تأیید نکرده باشد → ۱۵۰۹.
    /// بررسی وضعیت + مانیفست + تأیید شبکه در «یک» تراکنش انجام می‌شود تا
    /// پنجره‌ای برای لغو هم‌زمان تأیید (TOCTOU) باقی نماند.
    pub fn start(&self, plugin_id: &str) -> Result<(), PluginError> {
        let mut guard = self.db.lock();
        let tx = guard
            .transaction()
            .map_err(|e| PluginError::storage(e.to_string()))?;
        let manifest_json: String = tx
            .query_row(
                "SELECT manifest_json FROM plugins WHERE id = ?1",
                params![plugin_id],
                |r| r.get(0),
            )
            .map_err(|_| PluginError::plugin_not_found(plugin_id))?;
        let manifest = PluginManifest::parse(&manifest_json)?;
        if manifest.needs_network() {
            let approved: Option<String> = tx
                .query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    params![format!("plugin.network_approved.{plugin_id}")],
                    |r| r.get(0),
                )
                .map(Some)
                .or_else(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => Ok(None),
                    other => Err(other),
                })?;
            if approved.as_deref() != Some("true") {
                return Err(PluginError::network_not_approved(plugin_id));
            }
        }
        self.transition_tx(&tx, plugin_id, PluginStatus::Running, "start")?;
        tx.execute(
            "UPDATE plugins SET crash_count = 0, updated_at = ?1 WHERE id = ?2",
            params![now_iso(), plugin_id],
        )?;
        tx.commit().map_err(|e| PluginError::storage(e.to_string()))?;
        Ok(())
    }

    /// توقف — running → stopped.
    pub fn stop(&self, plugin_id: &str) -> Result<(), PluginError> {
        self.transition(plugin_id, PluginStatus::Stopped, "stop")
    }

    /// غیرفعال‌سازی — از هر وضعیت زنده مجاز (قرنطینه هم با اقدام دستی کاربر).
    pub fn disable(&self, plugin_id: &str) -> Result<(), PluginError> {
        self.transition(plugin_id, PluginStatus::Disabled, "disable")
    }

    /// گزارش کرش از موتور اجرا — شمارنده متوالی افزایش و در صورت عبور از آستانه، قرنطینه.
    pub fn report_crash(&self, plugin_id: &str, error: &str) -> Result<PluginStatus, PluginError> {
        let mut guard = self.db.lock();
        let tx = guard
            .transaction()
            .map_err(|e| PluginError::storage(e.to_string()))?;
        let (status, crash_count, manifest_json): (String, i64, String) = tx
            .query_row(
                "SELECT status, crash_count, manifest_json FROM plugins WHERE id = ?1",
                params![plugin_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|_| PluginError::plugin_not_found(plugin_id))?;
        let current = PluginStatus::parse(&status)
            .ok_or_else(|| PluginError::storage(format!("corrupt status: {status}")))?;
        if !current.can_transition_to(PluginStatus::Crashed) {
            return Err(PluginError::invalid_transition(format!(
                "crash reported from status {}",
                current.as_str()
            )));
        }
        let new_count = crash_count + 1;
        let manifest = PluginManifest::parse(&manifest_json)?;
        let threshold = i64::from(manifest.effective_health().max_consecutive_crashes);
        let now = now_iso();
        let factory = PluginEventFactory::new(Uuid::new_v4());
        let final_status = if new_count > threshold {
            tx.execute(
                "UPDATE plugins SET status = 'quarantined', crash_count = ?1,
                 last_error = ?2, updated_at = ?3 WHERE id = ?4",
                params![new_count, error, now, plugin_id],
            )?;
            self.write_event(&tx, &factory.quarantined(plugin_id, new_count))?;
            PluginStatus::Quarantined
        } else {
            tx.execute(
                "UPDATE plugins SET status = 'crashed', crash_count = ?1,
                 last_error = ?2, updated_at = ?3 WHERE id = ?4",
                params![new_count, error, now, plugin_id],
            )?;
            self.write_event(&tx, &factory.crashed(plugin_id, new_count, error))?;
            PluginStatus::Crashed
        };
        tx.commit().map_err(|e| PluginError::storage(e.to_string()))?;
        Ok(final_status)
    }

    /// خروج دستی کاربر از قرنطینه — quarantined → disabled (سپس enable دستی).
    /// فقط از وضعیت «quarantined» مجاز است (۱۵۰۷ در غیر این صورت).
    pub fn release_from_quarantine(&self, plugin_id: &str) -> Result<(), PluginError> {
        let mut guard = self.db.lock();
        let tx = guard
            .transaction()
            .map_err(|e| PluginError::storage(e.to_string()))?;
        let status: String = tx
            .query_row(
                "SELECT status FROM plugins WHERE id = ?1",
                params![plugin_id],
                |r| r.get(0),
            )
            .map_err(|_| PluginError::plugin_not_found(plugin_id))?;
        let current = PluginStatus::parse(&status)
            .ok_or_else(|| PluginError::storage(format!("corrupt status: {status}")))?;
        if current != PluginStatus::Quarantined {
            return Err(PluginError::invalid_transition(format!(
                "release_from_quarantine requires quarantined status, found {}",
                current.as_str()
            )));
        }
        self.transition_tx(&tx, plugin_id, PluginStatus::Disabled, "release_from_quarantine")?;
        // شمارنده کرش متوالی پس از اقدام دستی کاربر صفر می‌شود
        tx.execute(
            "UPDATE plugins SET crash_count = 0, updated_at = ?1 WHERE id = ?2",
            params![now_iso(), plugin_id],
        )?;
        let factory = PluginEventFactory::new(Uuid::new_v4());
        self.write_event(&tx, &factory.released(plugin_id))?;
        tx.commit().map_err(|e| PluginError::storage(e.to_string()))?;
        Ok(())
    }

    // ==================== تأیید دسترسی شبکه ====================

    /// تأیید صریح کاربر برای network.access.
    pub fn approve_network_access(&self, plugin_id: &str) -> Result<(), PluginError> {
        self.set_network_approval(plugin_id, true)
    }

    /// لغو تأیید دسترسی شبکه.
    pub fn revoke_network_access(&self, plugin_id: &str) -> Result<(), PluginError> {
        self.set_network_approval(plugin_id, false)
    }

    /// آیا کاربر دسترسی شبکه را تأیید کرده؟
    pub fn network_approved(&self, plugin_id: &str) -> Result<bool, PluginError> {
        let conn = self.db.lock();
        let value: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![format!("plugin.network_approved.{plugin_id}")],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?;
        Ok(value.as_deref() == Some("true"))
    }

    fn set_network_approval(&self, plugin_id: &str, approved: bool) -> Result<(), PluginError> {
        if !self.exists(plugin_id)? {
            return Err(PluginError::plugin_not_found(plugin_id));
        }
        let conn = self.db.lock();
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
            params![
                format!("plugin.network_approved.{plugin_id}"),
                if approved { "true" } else { "false" },
                now_iso()
            ],
        )?;
        Ok(())
    }

    // ==================== اجرای مجوز ====================

    /// اجرای مجوز برای فراخوانی RPC پلاگین — نقطه واحد اعمال قابلیت‌ها.
    /// پلاگین باید فعال (enabled/running) باشد؛ قرنطینه ۱۵۰۸ و بقیه ۱۵۰۶ می‌دهند.
    /// وضعیت + مانیفست + تأیید شبکه زیر «یک» قفل خوانده می‌شوند تا پنجره
    /// تغییر هم‌زمان (TOCTOU) وجود نداشته باشد.
    pub fn enforce(&self, plugin_id: &str, cap: Capability) -> Result<(), PluginError> {
        let conn = self.db.lock();
        let (status, manifest_json): (String, String) = conn
            .query_row(
                "SELECT status, manifest_json FROM plugins WHERE id = ?1",
                params![plugin_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| PluginError::plugin_not_found(plugin_id))?;
        let record_status = PluginStatus::parse(&status)
            .ok_or_else(|| PluginError::storage(format!("corrupt status: {status}")))?;
        match record_status {
            PluginStatus::Quarantined => return Err(PluginError::quarantined(plugin_id)),
            PluginStatus::Enabled | PluginStatus::Running => {}
            PluginStatus::Installed
            | PluginStatus::Stopped
            | PluginStatus::Crashed
            | PluginStatus::Disabled => {
                return Err(PluginError::permission_denied(format!(
                    "plugin not active (status: {})",
                    record_status.as_str()
                )))
            }
        }
        let manifest = PluginManifest::parse(&manifest_json)?;
        let perms = PermissionSet::from_capabilities(
            &manifest.capabilities.iter().filter_map(|c| Capability::parse(c)).collect::<Vec<_>>(),
        );
        perms.require(cap)?;
        // قابلیت ویژه شبکه — تأیید صریح کاربر لازم است حتی اگر در مانیفست باشد
        if cap == Capability::NetworkAccess {
            let approved: Option<String> = conn
                .query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    params![format!("plugin.network_approved.{plugin_id}")],
                    |r| r.get(0),
                )
                .map(Some)
                .or_else(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => Ok(None),
                    other => Err(other),
                })?;
            if approved.as_deref() != Some("true") {
                return Err(PluginError::network_not_approved(plugin_id));
            }
        }
        Ok(())
    }

    /// مجموعه قابلیت‌های پلاگین — برای نمایش مجوزها به کاربر.
    pub fn capabilities_of(&self, plugin_id: &str) -> Result<PermissionSet, PluginError> {
        let manifest = self.manifest_of(plugin_id)?;
        Ok(PermissionSet::from_capabilities(
            &manifest.capabilities.iter().filter_map(|c| Capability::parse(c)).collect::<Vec<_>>(),
        ))
    }

    // ==================== قرارداد موتور اجرا ====================

    /// سیاست منابع برای موتور اجرا (فاز ۱.۸) — از مانیفست با پیش‌فرض‌ها.
    pub fn runtime_limits(&self, plugin_id: &str) -> Result<ResourcePolicy, PluginError> {
        Ok(self.manifest_of(plugin_id)?.effective_resources())
    }

    // ==================== سلامت و خواندن ====================

    /// وضعیت سلامت پلاگین — قابل مشاهده برای UI.
    pub fn health(&self, plugin_id: &str) -> Result<PluginHealth, PluginError> {
        let record = self.record_of(plugin_id)?;
        Ok(PluginHealth {
            id: record.id,
            status: record.status,
            crash_count: record.crash_count,
            last_error: record.last_error,
        })
    }

    /// فهرست همه پلاگین‌های ثبت‌شده با وضعیت سلامت.
    pub fn list(&self) -> Result<Vec<PluginHealth>, PluginError> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, status, crash_count, last_error FROM plugins ORDER BY id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, status_text, crash_count, last_error) = row?;
            // وضعیت خراب را بی‌صدا پنهان نکن — خطا برگردان
            let status = PluginStatus::parse(&status_text)
                .ok_or_else(|| PluginError::storage(format!("corrupt status: {status_text}")))?;
            out.push(PluginHealth { id, status, crash_count, last_error });
        }
        Ok(out)
    }

    /// مانیفست پلاگین ثبت‌شده.
    pub fn manifest_of(&self, plugin_id: &str) -> Result<PluginManifest, PluginError> {
        let conn = self.db.lock();
        let json: String = conn
            .query_row(
                "SELECT manifest_json FROM plugins WHERE id = ?1",
                params![plugin_id],
                |r| r.get(0),
            )
            .map_err(|_| PluginError::plugin_not_found(plugin_id))?;
        PluginManifest::parse(&json)
    }

    fn record_of(&self, plugin_id: &str) -> Result<PluginRecord, PluginError> {
        let conn = self.db.lock();
        let (status, crash_count, last_error, installed_at, updated_at): (
            String,
            i64,
            Option<String>,
            String,
            String,
        ) = conn
            .query_row(
                "SELECT status, crash_count, last_error, installed_at, updated_at
                 FROM plugins WHERE id = ?1",
                params![plugin_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .map_err(|_| PluginError::plugin_not_found(plugin_id))?;
        Ok(PluginRecord {
            id: plugin_id.to_string(),
            status: PluginStatus::parse(&status)
                .ok_or_else(|| PluginError::storage(format!("corrupt status: {status}")))?,
            crash_count,
            last_error,
            installed_at,
            updated_at,
        })
    }

    fn exists(&self, plugin_id: &str) -> Result<bool, PluginError> {
        let conn = self.db.lock();
        let found: i64 = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM plugins WHERE id = ?1)",
            params![plugin_id],
            |r| r.get(0),
        )?;
        Ok(found != 0)
    }

    fn exists_tx(&self, tx: &Transaction<'_>, plugin_id: &str) -> Result<bool, PluginError> {
        let found: i64 = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM plugins WHERE id = ?1)",
            params![plugin_id],
            |r| r.get(0),
        )?;
        Ok(found != 0)
    }

    // ==================== گذار وضعیت ====================

    fn transition(&self, plugin_id: &str, target: PluginStatus, action: &str) -> Result<(), PluginError> {
        let mut guard = self.db.lock();
        let tx = guard
            .transaction()
            .map_err(|e| PluginError::storage(e.to_string()))?;
        self.transition_tx(&tx, plugin_id, target, action)?;
        tx.commit().map_err(|e| PluginError::storage(e.to_string()))?;
        Ok(())
    }

    /// گذار وضعیت با اعتبارسنجی جدول گذارها و نوشتن رویداد — در تراکنش جاری.
    fn transition_tx(&self, tx: &Transaction<'_>, plugin_id: &str, target: PluginStatus, action: &str) -> Result<(), PluginError> {
        let status: String = tx
            .query_row(
                "SELECT status FROM plugins WHERE id = ?1",
                params![plugin_id],
                |r| r.get(0),
            )
            .map_err(|_| PluginError::plugin_not_found(plugin_id))?;
        let current = PluginStatus::parse(&status)
            .ok_or_else(|| PluginError::storage(format!("corrupt status: {status}")))?;
        if !current.can_transition_to(target) {
            return Err(PluginError::invalid_transition(format!(
                "{action}: {} → {} not allowed",
                current.as_str(),
                target.as_str()
            )));
        }
        tx.execute(
            "UPDATE plugins SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![target.as_str(), now_iso(), plugin_id],
        )?;
        let factory = PluginEventFactory::new(Uuid::new_v4());
        let event = match target {
            PluginStatus::Enabled => factory.enabled(plugin_id),
            PluginStatus::Running => factory.started(plugin_id),
            PluginStatus::Stopped => factory.stopped(plugin_id),
            PluginStatus::Disabled => factory.disabled(plugin_id),
            _ => factory.disabled(plugin_id), // غیرقابل‌وقوع در گذار مستقیم
        };
        self.write_event(tx, &event)?;
        Ok(())
    }

    // ==================== outbox ====================

    fn write_event(&self, tx: &Transaction<'_>, ev: &EventEnvelope) -> Result<(), PluginError> {
        tx.execute(
            "INSERT INTO system_events (id, event_type, event_version, source, correlation_id,
             payload, published, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,0,?7)",
            params![
                ev.event_id.to_string(),
                ev.event_type,
                ev.event_version as i64,
                ev.source.as_str(),
                ev.correlation_id.to_string(),
                ev.payload.to_string(),
                now_iso()
            ],
        )?;
        Ok(())
    }

    /// رویدادهای منتشرنشده — برای انتشار پس از commit (الگوی مشترک کرنل).
    pub fn unpublished_events(&self, limit: usize) -> Result<Vec<EventEnvelope>, PluginError> {
        let conn = self.db.lock();
        // ترتیب قطعی: زمان ایجاد سپس ترتیب درج (rowid)؛ زمان رویداد از خود ردیف می‌آید
        let mut stmt = conn.prepare_cached(
            "SELECT id, event_type, event_version, source, correlation_id, payload, created_at
             FROM system_events WHERE published = 0 AND source = 'plugin_engine'
             ORDER BY created_at, rowid LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, event_type, version, source, correlation, payload, created_at) = row?;
            let timestamp = chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|d| d.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            out.push(EventEnvelope {
                event_id: Uuid::parse_str(&id)
                    .map_err(|e| PluginError::storage(format!("bad event_id: {e}")))?,
                event_type,
                event_version: version as u32,
                source: match source.as_str() {
                    "plugin_engine" => EventSource::PluginEngine,
                    other => EventSource::Other(other.to_string()),
                },
                timestamp,
                correlation_id: Uuid::parse_str(&correlation)
                    .map_err(|e| PluginError::storage(format!("bad correlation_id: {e}")))?,
                payload: serde_json::from_str(&payload)
                    .map_err(|e| PluginError::storage(format!("bad payload: {e}")))?,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        let db = Database::open_memory(Some("test-pass-1")).unwrap();
        {
            let mut conn = db.lock();
            aria_storage_engine::migrations::run_migrations(&mut conn, |_| Ok(())).unwrap();
        }
        db
    }

    fn svc(db: &Database) -> PluginService<'_> {
        PluginService::new(db, "1.2.0", "1.0.0").unwrap()
    }

    fn manifest_json(id: &str, caps: &[&str]) -> String {
        format!(
            r#"{{
                "id": "{id}",
                "name": "پلاگین آزمون",
                "version": "1.0.0",
                "api_version": "1.0.0",
                "entrypoint": "main.py",
                "runtime_mode": "out_of_process",
                "kernel_version_range": ">=1.0.0, <2.0.0",
                "api_version_range": "^1.0.0",
                "capabilities": [{}]
            }}"#,
            caps.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ")
        )
    }

    fn install(db: &Database, id: &str, caps: &[&str]) {
        svc(db).install(&manifest_json(id, caps)).unwrap();
    }

    /// نصب + تأیید کاربر + فعال‌سازی + راه‌اندازی → running.
    fn run_to_running(db: &Database, id: &str, caps: &[&str]) {
        install(db, id, caps);
        let s = svc(db);
        s.enable(id).unwrap();
        s.start(id).unwrap();
    }

    // ==================== نصب و اعتبارسنجی ====================

    #[test]
    fn install_valid_manifest_persists() {
        let db = db();
        install(&db, "com.test.alpha", &["trades.read"]);
        let s = svc(&db);
        let health = s.health("com.test.alpha").unwrap();
        assert_eq!(health.status, PluginStatus::Installed);
        assert_eq!(health.crash_count, 0);
        assert_eq!(s.list().unwrap().len(), 1);
        // رویداد نصب در outbox
        let events = s.unpublished_events(10).unwrap();
        assert_eq!(events[0].event_type, "plugin.installed");
    }

    #[test]
    fn install_invalid_manifest_rejected() {
        let db = db();
        let err = svc(&db).install("{ not json }").unwrap_err();
        assert_eq!(err.code(), 1501);
        // هیچ چیزی ثبت نشده
        assert_eq!(svc(&db).list().unwrap().len(), 0);
    }

    #[test]
    fn install_missing_required_field_rejected() {
        let db = db();
        let json = manifest_json("com.test.alpha", &[]).replace("\"entrypoint\": \"main.py\",", "");
        let err = svc(&db).install(&json).unwrap_err();
        assert_eq!(err.code(), 1501);
    }

    #[test]
    fn install_duplicate_rejected() {
        let db = db();
        install(&db, "com.test.alpha", &[]);
        let err = svc(&db).install(&manifest_json("com.test.alpha", &[])).unwrap_err();
        assert_eq!(err.code(), 1503);
    }

    #[test]
    fn install_incompatible_kernel_version_rejected() {
        let db = db();
        let json = manifest_json("com.test.alpha", &[])
            .replace(">=1.0.0, <2.0.0", ">=2.0.0");
        let err = svc(&db).install(&json).unwrap_err();
        assert_eq!(err.code(), 1504);
        let json = manifest_json("com.test.beta", &[]).replace("^1.0.0", "^9.0.0");
        let err = svc(&db).install(&json).unwrap_err();
        assert_eq!(err.code(), 1504);
        assert_eq!(svc(&db).list().unwrap().len(), 0);
    }

    // ==================== چرخه حیات ====================

    #[test]
    fn full_lifecycle_happy_path() {
        let db = db();
        let s = svc(&db);
        install(&db, "com.test.alpha", &[]);
        s.enable("com.test.alpha").unwrap();
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Enabled);
        s.start("com.test.alpha").unwrap();
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Running);
        s.stop("com.test.alpha").unwrap();
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Stopped);
        s.start("com.test.alpha").unwrap();
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Running);
        s.disable("com.test.alpha").unwrap();
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Disabled);
        // از غیرفعال دوباره قابل فعال‌سازی است
        s.enable("com.test.alpha").unwrap();
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Enabled);
    }

    #[test]
    fn invalid_transitions_rejected() {
        let db = db();
        let s = svc(&db);
        install(&db, "com.test.alpha", &[]);
        // نصب‌نشده-فعال نمی‌تواند مستقیم start شود
        let err = s.start("com.test.alpha").unwrap_err();
        assert_eq!(err.code(), 1507);
        // توقف قبل از راه‌اندازی ممنوع
        let err = s.stop("com.test.alpha").unwrap_err();
        assert_eq!(err.code(), 1507);
        // غیرموجود → ۱۵۰۲
        let err = s.enable("com.nope.zzz").unwrap_err();
        assert_eq!(err.code(), 1502);
    }

    #[test]
    fn crash_from_non_running_rejected() {
        let db = db();
        install(&db, "com.test.alpha", &[]);
        let err = svc(&db).report_crash("com.test.alpha", "boom").unwrap_err();
        assert_eq!(err.code(), 1507);
    }

    // ==================== قرنطینه ====================

    #[test]
    fn three_crashes_then_fourth_quarantines() {
        let db = db();
        run_to_running(&db, "com.test.alpha", &[]);
        let s = svc(&db);
        // سه کرش متوالی → crashed (هنوز نه قرنطینه)
        for i in 1..=3 {
            let st = s.report_crash("com.test.alpha", &format!("boom {i}")).unwrap();
            assert_eq!(st, PluginStatus::Crashed);
        }
        let h = s.health("com.test.alpha").unwrap();
        assert_eq!(h.crash_count, 3);
        assert_eq!(h.last_error.as_deref(), Some("boom 3"));
        // کرش چهارم → قرنطینه («بیشتر از ۳»)
        let st = s.report_crash("com.test.alpha", "boom 4").unwrap();
        assert_eq!(st, PluginStatus::Quarantined);
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Quarantined);
    }

    #[test]
    fn start_while_quarantined_blocked() {
        let db = db();
        run_to_running(&db, "com.test.alpha", &[]);
        let s = svc(&db);
        for i in 1..=4 {
            s.report_crash("com.test.alpha", &format!("boom {i}")).unwrap();
        }
        // بازگشت به running از crashed مجاز است ولی از quarantined خیر
        // (crashed بعد از سوم راه‌اندازی مجدد نمی‌گیریم — پس مستقیم start تلاش می‌کنیم)
        s.release_from_quarantine("com.test.alpha").unwrap();
        // پس از خروج → disabled؛ enable و start دستی
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Disabled);
        s.enable("com.test.alpha").unwrap();
        s.start("com.test.alpha").unwrap();
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Running);
    }

    #[test]
    fn start_from_crashed_is_allowed_but_quarantined_never() {
        let db = db();
        run_to_running(&db, "com.test.alpha", &[]);
        let s = svc(&db);
        s.report_crash("com.test.alpha", "boom").unwrap();
        // از crashed راه‌اندازی مجدد مجاز
        s.start("com.test.alpha").unwrap();
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Running);

        // حالا قرنطینه: ۴ کرش بدون راه‌اندازی مجدد
        for i in 1..=4 {
            s.report_crash("com.test.alpha", &format!("boom {i}")).unwrap();
        }
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Quarantined);
        // قرنطینه راه‌اندازی مجدد مستقیم ندارد — حتی enable
        let err = s.enable("com.test.alpha").unwrap_err();
        assert_eq!(err.code(), 1507);
        // راه‌اندازی از قرنطینه → چون گذار quarantined→running مجاز نیست
        let err = s.start("com.test.alpha").unwrap_err();
        assert_eq!(err.code(), 1507);
    }

    #[test]
    fn successful_start_resets_consecutive_crash_count() {
        let db = db();
        run_to_running(&db, "com.test.alpha", &[]);
        let s = svc(&db);
        s.report_crash("com.test.alpha", "boom 1").unwrap();
        s.report_crash("com.test.alpha", "boom 2").unwrap();
        s.start("com.test.alpha").unwrap(); // شمارنده صفر می‌شود
        assert_eq!(s.health("com.test.alpha").unwrap().crash_count, 0);
        // سه کرش دیگر → باز هم نه قرنطینه (شمارنده متوالی)
        for i in 1..=3 {
            s.report_crash("com.test.alpha", &format!("boom {i}")).unwrap();
        }
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Crashed);
        // چهارم → قرنطینه
        s.report_crash("com.test.alpha", "final").unwrap();
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Quarantined);
    }

    #[test]
    fn manifest_threshold_respected() {
        let db = db();
        let json = manifest_json("com.test.alpha", &[]).replace(
            "\"capabilities\": []",
            "\"capabilities\": [], \"health_policy\": {\"max_consecutive_crashes\": 2}",
        );
        svc(&db).install(&json).unwrap();
        let s = svc(&db);
        s.enable("com.test.alpha").unwrap();
        s.start("com.test.alpha").unwrap();
        s.report_crash("com.test.alpha", "a").unwrap();
        s.report_crash("com.test.alpha", "b").unwrap();
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Crashed);
        s.report_crash("com.test.alpha", "c").unwrap();
        assert_eq!(s.health("com.test.alpha").unwrap().status, PluginStatus::Quarantined);
    }

    // ==================== مجوزها ====================

    #[test]
    fn enforce_granted_capability_when_running() {
        let db = db();
        run_to_running(&db, "com.test.alpha", &["trades.read", "stats.read"]);
        let s = svc(&db);
        assert!(s.enforce("com.test.alpha", Capability::TradesRead).is_ok());
        assert!(s.enforce("com.test.alpha", Capability::StatsRead).is_ok());
        let err = s.enforce("com.test.alpha", Capability::TradesDelete).unwrap_err();
        assert_eq!(err.code(), 1506);
    }

    #[test]
    fn enforce_denied_when_not_active() {
        let db = db();
        install(&db, "com.test.alpha", &["trades.read"]);
        let s = svc(&db);
        // installed → مجوز ندارد
        let err = s.enforce("com.test.alpha", Capability::TradesRead).unwrap_err();
        assert_eq!(err.code(), 1506);
        s.enable("com.test.alpha").unwrap();
        assert!(s.enforce("com.test.alpha", Capability::TradesRead).is_ok());
        s.disable("com.test.alpha").unwrap();
        let err = s.enforce("com.test.alpha", Capability::TradesRead).unwrap_err();
        assert_eq!(err.code(), 1506);
    }

    #[test]
    fn enforce_on_quarantined_returns_quarantine_error() {
        let db = db();
        run_to_running(&db, "com.test.alpha", &["trades.read"]);
        let s = svc(&db);
        for i in 1..=4 {
            s.report_crash("com.test.alpha", &format!("boom {i}")).unwrap();
        }
        let err = s.enforce("com.test.alpha", Capability::TradesRead).unwrap_err();
        assert_eq!(err.code(), 1508);
    }

    #[test]
    fn network_access_denied_until_user_approval() {
        let db = db();
        // نصب با قابلیت شبکه — start بدون تأیید کاربر ممنوع
        install(&db, "com.test.alpha", &["network.access"]);
        let s = svc(&db);
        s.enable("com.test.alpha").unwrap();
        let err = s.start("com.test.alpha").unwrap_err();
        assert_eq!(err.code(), 1509);
        // تأیید صریح کاربر → راه‌اندازی و اجرای مجوز ممکن
        s.approve_network_access("com.test.alpha").unwrap();
        assert!(s.network_approved("com.test.alpha").unwrap());
        s.start("com.test.alpha").unwrap();
        assert!(s.enforce("com.test.alpha", Capability::NetworkAccess).is_ok());
        // لغو تأیید → دوباره ۱۵۰۹
        s.revoke_network_access("com.test.alpha").unwrap();
        let err = s.enforce("com.test.alpha", Capability::NetworkAccess).unwrap_err();
        assert_eq!(err.code(), 1509);
    }

    #[test]
    fn network_plugin_cannot_start_without_approval() {
        let db = db();
        install(&db, "com.test.alpha", &["network.access"]);
        let s = svc(&db);
        s.enable("com.test.alpha").unwrap();
        let err = s.start("com.test.alpha").unwrap_err();
        assert_eq!(err.code(), 1509);
        s.approve_network_access("com.test.alpha").unwrap();
        assert!(s.start("com.test.alpha").is_ok());
    }

    #[test]
    fn release_from_quarantine_rejects_non_quarantined_plugin() {
        let db = db();
        run_to_running(&db, "com.test.alpha", &[]);
        let s = svc(&db);
        // پلاگین running است — رهاسازی قرنطینه باید ۱۵۰۷ بدهد نه بی‌اثر بودن
        let err = s.release_from_quarantine("com.test.alpha").unwrap_err();
        assert_eq!(err.code(), 1507);
    }

    #[test]
    fn outbox_events_carry_row_timestamp_in_deterministic_order() {
        let db = db();
        run_to_running(&db, "com.test.alpha", &[]);
        let s = svc(&db);
        s.report_crash("com.test.alpha", "boom").unwrap();
        let events = s.unpublished_events(10).unwrap();
        assert!(!events.is_empty());
        // timestamp رویداد باید منطبق بر created_at ردیف outbox باشد (UTC ثانیه‌ای)
        {
            let conn = db.lock();
            let created: String = conn
                .query_row(
                    "SELECT created_at FROM system_events WHERE source = 'plugin_engine' LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            let parsed = chrono::DateTime::parse_from_rfc3339(&created).unwrap();
            assert_eq!(events[0].timestamp.to_rfc3339(), parsed.with_timezone(&chrono::Utc).to_rfc3339());
        }
    }

    #[test]
    fn capabilities_visible_to_user() {
        let db = db();
        install(&db, "com.test.alpha", &["trades.read", "ui.widget"]);
        let caps = svc(&db).capabilities_of("com.test.alpha").unwrap();
        assert_eq!(caps.list().len(), 2);
        assert!(caps.has(Capability::TradesRead));
        assert!(caps.has(Capability::UiWidget));
        assert!(!caps.has(Capability::BackupRestore));
    }

    // ==================== قرارداد موتور اجرا ====================

    #[test]
    fn runtime_limits_from_manifest_with_defaults() {
        let db = db();
        // بدون resources → پیش‌فرض‌ها
        install(&db, "com.test.defaults", &[]);
        let s = svc(&db);
        let r = s.runtime_limits("com.test.defaults").unwrap();
        assert_eq!((r.max_memory_mb, r.max_cpu_percent, r.timeout_seconds), (512, 50, 30));
        // با resources سفارشی
        let json = manifest_json("com.test.limits", &[]).replace(
            "\"capabilities\": []",
            "\"capabilities\": [], \"resources\": {\"max_memory_mb\": 1024, \"max_cpu_percent\": 20, \"timeout_seconds\": 120}",
        );
        svc(&db).install(&json).unwrap();
        let r = s.runtime_limits("com.test.limits").unwrap();
        assert_eq!((r.max_memory_mb, r.max_cpu_percent, r.timeout_seconds), (1024, 20, 120));
    }

    // ==================== سلامت ====================

    #[test]
    fn health_observable_with_last_error() {
        let db = db();
        run_to_running(&db, "com.test.alpha", &[]);
        let s = svc(&db);
        s.report_crash("com.test.alpha", "panic: index out of range").unwrap();
        let h = s.health("com.test.alpha").unwrap();
        assert_eq!(h.status, PluginStatus::Crashed);
        assert_eq!(h.crash_count, 1);
        assert_eq!(h.last_error.as_deref(), Some("panic: index out of range"));
    }

    #[test]
    fn health_of_unknown_plugin_not_found() {
        let db = db();
        let err = svc(&db).health("com.nope.zzz").unwrap_err();
        assert_eq!(err.code(), 1502);
    }

    // ==================== outbox ====================

    #[test]
    fn lifecycle_events_written_to_outbox() {
        let db = db();
        let s = svc(&db);
        install(&db, "com.test.alpha", &[]);
        s.enable("com.test.alpha").unwrap();
        s.start("com.test.alpha").unwrap();
        s.stop("com.test.alpha").unwrap();
        let events = s.unpublished_events(50).unwrap();
        let types: Vec<&str> = events.iter().map(|e| e.event_type.as_str()).collect();
        assert_eq!(
            types,
            vec![
                "plugin.installed",
                "plugin.enabled",
                "plugin.started",
                "plugin.stopped"
            ]
        );
        assert!(events.iter().all(|e| e.source.as_str() == "plugin_engine"));
    }
}
