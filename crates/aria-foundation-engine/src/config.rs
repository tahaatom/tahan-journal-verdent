//! پیکربندی کرنل (KernelConfig).
//!
//! - مسیرهای استاندارد اپلیکیشن با crate `dirs` تعیین می‌شوند.
//! - قالب فایل پیکربندی JSON است (انتخاب ساده‌ترین تفسیر — فرض A-002).

use crate::error::FoundationError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// پیش‌فرض قفل خودکار: ۱۵ دقیقه (بر حسب ثانیه).
pub const DEFAULT_AUTO_LOCK_TIMEOUT_SECS: u64 = 15 * 60;

/// پیکربندی کرنل آریا.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct KernelConfig {
    /// مسیر ریشه داده‌های اپلیکیشن
    pub data_directory: PathBuf,
    /// مسیر فایل پایگاه‌داده
    pub database_path: PathBuf,
    /// مسیر پیوست‌ها
    pub attachments_directory: PathBuf,
    /// مسیر بکاپ‌ها
    pub backup_directory: PathBuf,
    /// مسیر لاگ‌ها
    pub log_directory: PathBuf,
    /// مسیر پلاگین‌ها
    pub plugin_directory: PathBuf,
    /// زبان رابط کاربری (نسخه ۱: فقط fa)
    pub locale: String,
    /// تم (light | dark | system)
    pub theme: String,
    /// مهلت قفل خودکار (ثانیه)
    pub auto_lock_timeout_secs: u64,
}

impl Default for KernelConfig {
    fn default() -> Self {
        let base = default_data_dir();
        Self {
            data_directory: base.clone(),
            database_path: base.join("data/tahan.db"),
            attachments_directory: base.join("attachments"),
            backup_directory: base.join("backups"),
            log_directory: base.join("logs"),
            plugin_directory: base.join("plugins"),
            locale: "fa".to_string(),
            theme: "system".to_string(),
            auto_lock_timeout_secs: DEFAULT_AUTO_LOCK_TIMEOUT_SECS,
        }
    }
}

impl KernelConfig {
    /// مسیر ریشه پیش‌فرض بر اساس سیستم‌عامل.
    pub fn default_data_dir() -> PathBuf {
        default_data_dir()
    }

    /// اعتبارسنجی مقادیر پیکربندی.
    pub fn validate(&self) -> Result<(), FoundationError> {
        if self.locale != "fa" {
            return Err(FoundationError::invalid_config_value("locale"));
        }
        if !matches!(self.theme.as_str(), "light" | "dark" | "system") {
            return Err(FoundationError::invalid_config_value("theme"));
        }
        if self.auto_lock_timeout_secs == 0 {
            return Err(FoundationError::invalid_config_value("auto_lock_timeout_secs"));
        }
        Ok(())
    }

    /// بارگذاری پیکربندی از مسیر مشخص. اگر فایل نباشد، پیش‌فرض برمی‌گردد.
    pub fn load_from(path: &Path) -> Result<Self, FoundationError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .map_err(|_| FoundationError::config_load(path.display().to_string()))?;
        // خطای پارس با کد متمایز 1002
        serde_json::from_str::<Self>(&content).map_err(Self::map_parse_error)
    }

    fn map_parse_error(e: serde_json::Error) -> FoundationError {
        FoundationError::ConfigParseFailed { reason: e.to_string(), code: 1002 }
    }

    /// بارگذاری از مسیر استاندارد پیکربندی.
    pub fn load() -> Result<Self, FoundationError> {
        Self::load_from(&standard_config_path())
    }

    /// ذخیره پیکربندی در مسیر مشخص — نوشتن اتمیک (فایل موقت + تغییرنام).
    /// خرابی در میانه نوشتن هرگز فایل پیکربندی موجود را خراب نمی‌کند.
    pub fn save_to(&self, path: &Path) -> Result<(), FoundationError> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| {
                FoundationError::ConfigSaveFailed { path: parent.display().to_string(), code: 1003 }
            })?;
        }
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| FoundationError::config_parse(e.to_string()))?;
        // فایل موقت در همان پوشه (برای اتمیک بودن rename روی همان فایل‌سیستم)
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, content).map_err(|_| {
            FoundationError::ConfigSaveFailed { path: tmp_path.display().to_string(), code: 1003 }
        })?;
        std::fs::rename(&tmp_path, path).map_err(|_| {
            // تلاش برای پاک‌سازی فایل موقت؛ شکست پاک‌سازی اهمیتی ندارد
            let _ = std::fs::remove_file(&tmp_path);
            FoundationError::ConfigSaveFailed { path: path.display().to_string(), code: 1003 }
        })?;
        Ok(())
    }

    /// ذخیره در مسیر استاندارد پیکربندی.
    pub fn save(&self) -> Result<(), FoundationError> {
        self.save_to(&standard_config_path())
    }

    /// اطمینان از وجود پوشه‌های داده (داده، پیوست، بکاپ، لاگ، پلاگین).
    pub fn ensure_directories(&self) -> Result<(), FoundationError> {
        for dir in [
            &self.data_directory,
            &self.attachments_directory,
            &self.backup_directory,
            &self.log_directory,
            &self.plugin_directory,
        ] {
            std::fs::create_dir_all(dir).map_err(|e| FoundationError::io(format!(
                "cannot create directory {}: {e}",
                dir.display()
            )))?;
        }
        if let Some(parent) = self.database_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| FoundationError::io(e.to_string()))?;
        }
        Ok(())
    }
}

/// مسیر پیش‌فرض داده‌ها بر اساس سیستم‌عامل.
pub fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("TahanJournal")
}

/// مسیر استاندارد فایل پیکربندی.
pub fn standard_config_path() -> PathBuf {
    default_data_dir().join("config.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_consistent() {
        let c = KernelConfig::default();
        assert_eq!(c.locale, "fa");
        assert_eq!(c.theme, "system");
        assert_eq!(c.auto_lock_timeout_secs, DEFAULT_AUTO_LOCK_TIMEOUT_SECS);
        c.validate().unwrap();
        assert!(c.database_path.starts_with(&c.data_directory));
    }

    #[test]
    fn config_roundtrip_json() {
        let c = KernelConfig::default();
        let json = serde_json::to_string(&c).unwrap();
        let back: KernelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("config.json");
        let c = KernelConfig::load_from(&p).unwrap();
        assert_eq!(c, KernelConfig::default());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("cfg/config.json");
        let c = KernelConfig { theme: "dark".to_string(), auto_lock_timeout_secs: 300, ..Default::default() };
        c.save_to(&p).unwrap();
        let loaded = KernelConfig::load_from(&p).unwrap();
        assert_eq!(c, loaded);
    }

    #[test]
    fn invalid_values_rejected() {
        let mut c = KernelConfig { locale: "en".to_string(), ..Default::default() };
        assert!(matches!(c.validate(), Err(FoundationError::InvalidConfigValue { .. })));
        c.locale = "fa".to_string();
        c.theme = "blue".to_string();
        assert!(matches!(c.validate(), Err(FoundationError::InvalidConfigValue { .. })));
        c.theme = "dark".to_string();
        c.auto_lock_timeout_secs = 0;
        assert!(matches!(c.validate(), Err(FoundationError::InvalidConfigValue { .. })));
    }

    #[test]
    fn save_to_is_atomic_no_tmp_left_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("cfg/config.json");
        let c = KernelConfig { theme: "dark".to_string(), ..Default::default() };
        c.save_to(&p).unwrap();
        // فایل موقت باید بعد از نوشتن موفق حذف شده باشد
        let tmp_file = p.with_extension("json.tmp");
        assert!(!tmp_file.exists());
        assert!(p.exists());
        // ذخیره دوباره روی فایل موجود — مسیر rename جایگزین هم کار می‌کند
        c.save_to(&p).unwrap();
        assert!(KernelConfig::load_from(&p).unwrap() == c);
    }

    #[test]
    fn ensure_directories_creates_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = KernelConfig::default();
        c.data_directory = tmp.path().join("data");
        c.database_path = c.data_directory.join("tahan.db");
        c.attachments_directory = tmp.path().join("att");
        c.backup_directory = tmp.path().join("bk");
        c.log_directory = tmp.path().join("log");
        c.plugin_directory = tmp.path().join("plg");
        c.ensure_directories().unwrap();
        assert!(c.attachments_directory.exists());
        assert!(c.database_path.parent().unwrap().exists());
    }
}
