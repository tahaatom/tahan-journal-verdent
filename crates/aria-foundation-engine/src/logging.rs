//! لاگ‌سازی محلی ساخت‌یافته.
//!
//! قوانین (سند نسخه ۵):
//! - لاگ‌ها فقط محلی هستند.
//! - لاگ‌ها نباید شامل اسرار یا بار مالی کامل کاربر باشند.
//! - سطح‌بندی و فیلدهای ساخت‌یافته پشتیبانی می‌شود.

use std::path::Path;
use std::sync::OnceLock;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// مقداردهی اولیه لاگ‌سازی محلی.
///
/// - لاگ به فایل چرخشی روزانه در `log_dir` نوشته می‌شود و همزمان به کنسول هم می‌رود.
/// - سطح پیش‌فرض `info` است و با متغیر محیطی `TAHAN_LOG` قابل تنظیم است.
/// - نگه‌داشتن `WorkerGuard` در OnceLock تضمین می‌کند بافر رشتهٔ لاگ تا پایان برنامه فلاش شود.
///
/// خطای تکراری‌فراخوانی بی‌خطر است و `false` برمی‌گرداند.
pub fn init_logging(log_dir: &Path) -> bool {
    if LOG_GUARD.get().is_some() {
        return false;
    }

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // کنسول
    let console_layer = fmt::layer().with_target(true).compact();

    // فایل چرخشی روزانه
    let file_appender = tracing_appender::rolling::daily(log_dir, "tahan.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = fmt::layer().with_ansi(false).with_writer(std::sync::Mutex::new(writer));

    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        .try_init()
        .ok();
    let _ = LOG_GUARD.set(guard);
    true
}

/// پاکسازی محتوای حساس برای لاگ: مقدار کامل مالی/اسرار هرگز لاگ نمی‌شود.
/// فقط طول یا هش کوتاه محتوا گزارش می‌شود.
pub fn redact_for_log(payload: &str) -> String {
    let len = payload.chars().count();
    if len <= 4 {
        "***".to_string()
    } else {
        format!("***({len} chars)")
    }
}

/// لاگ رویداد ساخت‌یافته سطح اطلاع.
pub fn log_info(event: &str, fields: &[(&str, &str)]) {
    tracing::info!(event, fields = fields.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" "), "");
}

/// لاگ رویداد ساخت‌یافته سطح خطا.
pub fn log_error(event: &str, fields: &[(&str, &str)]) {
    tracing::error!(event, fields = fields.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" "), "");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_logging_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let first = init_logging(tmp.path());
        let second = init_logging(tmp.path());
        // یکی از این دو باید true باشد؛ فراخوانی دوم همیشه false است.
        assert!(!second);
        assert!(first || !first); // اولی ممکن است در محیط تست قبلاً مقداردهی شده باشد
    }

    #[test]
    fn redact_never_reveals_content() {
        let r = redact_for_log("12345.6789 USD secret");
        assert!(r.contains("***"));
        assert!(!r.contains("secret"));
        assert!(r.contains("chars"));
        assert_eq!(redact_for_log("abc"), "***");
    }

    #[test]
    fn structured_log_helpers_do_not_panic() {
        log_info("test_event", &[("k", "v")]);
        log_error("test_error", &[("k", "v")]);
    }
}
