//! پوسته دسکتاپ ژورنال طهان — پل امن IPC میان فرانت‌اند و کرنل آریا
//!
//! اصول امنیتی فاز ۱.۱۰:
//! - فرانت‌اند فقط به دستورات allowlist‌شده این فایل دسترسی دارد؛
//!   هیچ SQL یا دسترسی خام به دیتابیس از سمت UI وجود ندارد.
//! - هر دستور ورودی تایپ‌دار دارد و خطاها به پیام فارسی نگاشت می‌شوند.
//! - افزونه‌های UI فقط به‌صورت اعلانی از `aria-ui-engine` عرضه می‌شوند.

use aria_query_engine::{DashboardReport, QueryService, StatFieldInfo, TradeListRow};
use aria_storage_engine::{Database, migrations};
use aria_ui_engine::{ExtensionKind, UiExtensionRegistry};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

/// وضعیت کرنل در فرایند پوسته.
#[derive(Default)]
struct KernelState {
    /// هسته داده — تا فاز بکاپ/تنظیمات فقط حافظه‌ای؛ فایل رمزنگاری‌شده در فازهای بعد
    db: Option<Database>,
    /// رجیستری افزونه‌های UI اعلانی
    ui_registry: UiExtensionRegistry,
    /// آیا هسته باز شده است؟
    opened: bool,
}

/// اطلاعات کلی برنامه برای فرانت‌اند.
#[derive(Serialize)]
struct AppInfo {
    app_name: String,
    kernel: String,
    ui_decl_version: u32,
}

/// وضعیت اتصال به کرنل.
#[derive(Serialize)]
struct KernelStatus {
    opened: bool,
    mode: &'static str,
}

/// خروجی صفحه‌بندی‌شده فهرست معاملات برای فرانت‌اند.
#[derive(Serialize)]
struct TradesPage {
    items: Vec<TradeListRow>,
    total: i64,
    page: u32,
    page_size: u32,
}

fn kernel_error(e: aria_query_engine::QueryError) -> String {
    format!("{} (کد {})", e, e.code())
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        app_name: "ژورنال طهان".into(),
        kernel: format!("آریا v{}", env!("CARGO_PKG_VERSION")),
        ui_decl_version: aria_ui_engine::UI_ENGINE_DECL_VERSION,
    }
}

#[tauri::command]
fn kernel_status(state: State<'_, Mutex<KernelState>>) -> KernelStatus {
    let st = state.lock().unwrap();
    KernelStatus {
        opened: st.opened,
        mode: if st.opened { "memory" } else { "closed" },
    }
}

/// باز کردن هسته در حافظه با اجرای مهاجرت‌های اسکیما.
/// رویداد `kernel://ready` به فرانت‌اند گسیل می‌شود.
#[tauri::command]
fn kernel_open(
    app: AppHandle,
    state: State<'_, Mutex<KernelState>>,
) -> Result<KernelStatus, String> {
    let mut st = state.lock().unwrap();
    if st.opened {
        return Ok(KernelStatus {
            opened: true,
            mode: "memory",
        });
    }
    let db = Database::open_memory(None).map_err(|e| format!("خطای باز کردن هسته: {e}"))?;
    {
        let mut conn = db.lock();
        migrations::run_migrations(&mut conn, |_| Ok(()))
            .map_err(|e| format!("خطای مهاجرت اسکیما: {e}"))?;
    }
    st.db = Some(db);
    st.opened = true;
    drop(st);
    let _ = app.emit("kernel://ready", true);
    Ok(KernelStatus {
        opened: true,
        mode: "memory",
    })
}

/// فهرست صفحه‌بندی‌شده معاملات — فیلتر به‌صورت JSON اعلانی.
#[tauri::command]
fn query_trades(
    state: State<'_, Mutex<KernelState>>,
    filter: serde_json::Value,
    page: u32,
    page_size: u32,
) -> Result<TradesPage, String> {
    let st = state.lock().unwrap();
    let db = st
        .db
        .as_ref()
        .ok_or_else(|| "هسته هنوز باز نشده است".to_string())?;
    let node: aria_query_engine::FilterNode =
        serde_json::from_value(filter).map_err(|e| format!("فیلتر نامعتبر: {e}"))?;
    let svc = QueryService::new(db);
    let p = svc
        .list_trades(&node, page, page_size)
        .map_err(kernel_error)?;
    Ok(TradesPage {
        items: p.items,
        total: p.total,
        page: p.page,
        page_size: p.page_size,
    })
}

/// آمار مرکبی (نرخ برد، میانگین R، افت سرمایه و …).
#[tauri::command]
fn core_stats(
    state: State<'_, Mutex<KernelState>>,
    filter: serde_json::Value,
) -> Result<aria_query_engine::CoreStats, String> {
    let st = state.lock().unwrap();
    let db = st
        .db
        .as_ref()
        .ok_or_else(|| "هسته هنوز باز نشده است".to_string())?;
    let node: aria_query_engine::FilterNode =
        serde_json::from_value(filter).map_err(|e| format!("فیلتر نامعتبر: {e}"))?;
    QueryService::new(db)
        .aggregate(&node)
        .map_err(kernel_error)
}

/// گزارش داشبورد از پروجکشن خلاصه روزانه.
#[tauri::command]
fn dashboard_summary(
    state: State<'_, Mutex<KernelState>>,
    account_id: Option<String>,
) -> Result<DashboardReport, String> {
    let st = state.lock().unwrap();
    let db = st
        .db
        .as_ref()
        .ok_or_else(|| "هسته هنوز باز نشده است".to_string())?;
    QueryService::new(db)
        .dashboard(account_id.as_deref())
        .map_err(kernel_error)
}

/// فیلدهای سفارشی دارای مجوز آمار/تحلیل.
#[tauri::command]
fn stat_fields(state: State<'_, Mutex<KernelState>>) -> Result<Vec<StatFieldInfo>, String> {
    let st = state.lock().unwrap();
    let db = st
        .db
        .as_ref()
        .ok_or_else(|| "هسته هنوز باز نشده است".to_string())?;
    QueryService::new(db).stat_fields().map_err(kernel_error)
}

/// افزونه‌های UI اعلانی یک نقطه — فقط اسکیمای اعلانی، هرگز کد اجرایی.
#[tauri::command]
fn ui_extensions(
    state: State<'_, Mutex<KernelState>>,
    kind: String,
) -> Result<Vec<serde_json::Value>, String> {
    let parsed =
        ExtensionKind::parse(&kind).ok_or_else(|| format!("نقطه اعلانی نامعتبر: {kind}"))?;
    let st = state.lock().unwrap();
    Ok(st.ui_registry.snapshot(parsed))
}

fn main() {
    tauri::Builder::default()
        .manage(Mutex::new(KernelState::default()))
        .invoke_handler(tauri::generate_handler![
            app_info,
            kernel_status,
            kernel_open,
            query_trades,
            core_stats,
            dashboard_summary,
            stat_fields,
            ui_extensions
        ])
        .setup(|app| {
            // در محیط توسعه مسیر داده تضمین می‌شود (برای فازهای بکاپ/تنظیمات)
            let _ = app.path().app_data_dir();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("خطا در اجرای ژورنال طهان");
}
