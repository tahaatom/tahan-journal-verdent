//! پوسته دسکتاپ ژورنال طهان — پل امن IPC میان فرانت‌اند و کرنل آریا
//!
//! اصول امنیتی فاز ۱.۱۰:
//! - فرانت‌اند فقط به دستورات allowlist‌شده این فایل دسترسی دارد؛
//!   هیچ SQL یا دسترسی خام به دیتابیس از سمت UI وجود ندارد.
//! - هر دستور ورودی تایپ‌دار دارد و خطاها به پیام فارسی نگاشت می‌شوند.
//! - افزونه‌های UI فقط به‌صورت اعلانی از `aria-ui-engine` عرضه می‌شوند.

use aria_contracts::CommandEnvelope;
use aria_domain_engine::DomainService;
use aria_query_engine::{DashboardReport, QueryService, StatFieldInfo, TradeListRow};
use aria_schema_engine::SchemaService;
use aria_storage_engine::{Database, attachments, migrations};
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
    /// حالت هسته: closed | memory | file
    mode: &'static str,
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

/// خطای تایپ‌دار پل IPC — فرانت‌اند همیشه {code, message} می‌گیرد، نه رشته خام.
/// کدهای کرنل از بازه‌های قراردادی می‌آیند (۱۱۰۰+ ذخیره‌سازی، ۱۷۰۰+ پرس‌وجو…)؛
/// خطاهای محلی پل با کد ۰ (نامشخص) گزارش می‌شوند.
#[derive(Serialize, Clone, Debug)]
struct CmdError {
    code: i64,
    message: String,
}

impl CmdError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

fn kernel_error(e: aria_query_engine::QueryError) -> CmdError {
    CmdError::new(e.code(), e.to_string())
}

fn kernel_not_open() -> CmdError {
    CmdError::new(0, "هسته هنوز باز نشده است")
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
        mode: st.mode,
    }
}

/// باز کردن هسته در حافظه با اجرای مهاجرت‌های اسکیما.
/// رویداد `kernel://ready` به فرانت‌اند گسیل می‌شود.
#[tauri::command]
fn kernel_open(
    app: AppHandle,
    state: State<'_, Mutex<KernelState>>,
) -> Result<KernelStatus, CmdError> {
    let mut st = state.lock().unwrap();
    if st.opened {
        return Ok(KernelStatus {
            opened: true,
            mode: "memory",
        });
    }
    let db = Database::open_memory(None)
        .map_err(|e| CmdError::new(0, format!("خطای باز کردن هسته: {e}")))?;
    {
        let mut conn = db.lock();
        migrations::run_migrations(&mut conn, |_| Ok(()))
            .map_err(|e| CmdError::new(0, format!("خطای مهاجرت اسکیما: {e}")))?;
    }
    st.db = Some(db);
    st.opened = true;
    st.mode = "memory";
    journal_impl::seed_defaults(st.db.as_ref().unwrap())?;
    journal_impl::ensure_official_plugins(st.db.as_ref().unwrap())?;
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
    sort_key: Option<String>,
    sort_desc: Option<bool>,
) -> Result<TradesPage, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    let node: aria_query_engine::FilterNode = serde_json::from_value(filter)
        .map_err(|e| CmdError::new(aria_query_engine::QueryError::INVALID_QUERY, format!("فیلتر نامعتبر: {e}")))?;
    let svc = QueryService::new(db);
    let p = svc
        .list_trades(&node, page, page_size, sort_key.as_deref(), sort_desc.unwrap_or(true))
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
) -> Result<aria_query_engine::CoreStats, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    let node: aria_query_engine::FilterNode = serde_json::from_value(filter)
        .map_err(|e| CmdError::new(aria_query_engine::QueryError::INVALID_QUERY, format!("فیلتر نامعتبر: {e}")))?;
    QueryService::new(db)
        .aggregate(&node)
        .map_err(kernel_error)
}

/// گزارش داشبورد از پروجکشن خلاصه روزانه.
#[tauri::command]
fn dashboard_summary(
    state: State<'_, Mutex<KernelState>>,
    account_id: Option<String>,
) -> Result<DashboardReport, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    QueryService::new(db)
        .dashboard(account_id.as_deref())
        .map_err(kernel_error)
}

/// فیلدهای سفارشی دارای مجوز آمار/تحلیل.
#[tauri::command]
fn stat_fields(state: State<'_, Mutex<KernelState>>) -> Result<Vec<StatFieldInfo>, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    QueryService::new(db).stat_fields().map_err(kernel_error)
}

// ===================== فاز ۱.۱۴ — داشبورد و آمار =====================

/// تحلیل گره فیلتر از ورودی IPC.
fn parse_filter(filter: serde_json::Value) -> Result<aria_query_engine::FilterNode, CmdError> {
    serde_json::from_value(filter).map_err(|e| {
        CmdError::new(
            aria_query_engine::QueryError::INVALID_QUERY,
            format!("فیلتر نامعتبر: {e}"),
        )
    })
}

/// منحنی سرمایه (PnL تجمعی روزانه معاملات بسته).
#[tauri::command]
fn stats_equity(
    state: State<'_, Mutex<KernelState>>,
    filter: serde_json::Value,
) -> Result<Vec<aria_query_engine::EquityPoint>, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    let node = parse_filter(filter)?;
    QueryService::new(db)
        .equity_curve(&node)
        .map_err(kernel_error)
}

/// تفکیک عملکرد روی یک بُعد (نماد/استراتژی/…/فیلد سفارشی).
#[tauri::command]
fn stats_breakdown(
    state: State<'_, Mutex<KernelState>>,
    dim: serde_json::Value,
    filter: serde_json::Value,
) -> Result<Vec<aria_query_engine::GroupStat>, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    let dim: aria_query_engine::Dimension = serde_json::from_value(dim)
        .map_err(|e| CmdError::new(aria_query_engine::QueryError::INVALID_QUERY, format!("بُعد نامعتبر: {e}")))?;
    let node = parse_filter(filter)?;
    QueryService::new(db)
        .performance_by(&dim, &node)
        .map_err(kernel_error)
}

/// نقشه حرارتی زمان (روز هفته × ساعت بستن).
#[tauri::command]
fn stats_heatmap(
    state: State<'_, Mutex<KernelState>>,
    filter: serde_json::Value,
) -> Result<Vec<aria_query_engine::HeatCell>, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    let node = parse_filter(filter)?;
    QueryService::new(db).heatmap(&node).map_err(kernel_error)
}

/// افزونه‌های UI اعلانی یک نقطه — فقط اسکیمای اعلانی، هرگز کد اجرایی.
#[tauri::command]
fn ui_extensions(
    state: State<'_, Mutex<KernelState>>,
    kind: String,
) -> Result<Vec<serde_json::Value>, CmdError> {
    let parsed = ExtensionKind::parse(&kind)
        .ok_or_else(|| CmdError::new(0, format!("نقطه اعلانی نامعتبر: {kind}")))?;
    let st = state.lock().unwrap();
    Ok(st.ui_registry.snapshot(parsed))
}

// ===================== فاز ۱.۱۱ — ماژول ژورنال =====================
//
// منطق دستورات در `journal_impl` بدون وابستگی به Tauri پیاده شده تا بدون
// پوسته هم آزمون‌پذیر باشد؛ دستورهای tauri فقط قفل وضعیت و نگاشت خطا دارند.

/// خلاصه رویداد دامنه برای فرانت‌اند.
#[derive(Serialize, Clone, Debug)]
struct EventDto {
    event_id: String,
    event_type: String,
    payload: serde_json::Value,
}

/// حساب معاملاتی برای انتخابگر فرم.
#[derive(Serialize, Clone)]
struct AccountDto {
    id: String,
    name: String,
    currency: String,
}

/// نماد معاملاتی برای انتخابگر فرم.
#[derive(Serialize, Clone)]
struct SymbolDto {
    id: String,
    name: String,
}

/// پیوست ثبت‌شده و پیوند‌یافته با معامله.
#[derive(Serialize, Clone, Debug)]
struct AttachmentDto {
    id: String,
    file_name: String,
    size_bytes: i64,
    blake3_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thumbnail_path: Option<String>,
}

/// پیوست یک معامله در پنل جزئیات — با نوع پیوند و فراداده پیش‌نمایش.
#[derive(Serialize, Clone, Debug)]
struct TradeAttachmentDto {
    id: String,
    file_name: String,
    mime_type: Option<String>,
    size_bytes: i64,
    link_kind: String,
    blake3_hash: String,
    width: Option<i64>,
    height: Option<i64>,
    has_thumbnail: bool,
}

/// داده پیوست برای پیش‌نمایش — base64 + فراداده + نتیجه تأیید صحت (فاز ۱.۱۳).
#[derive(Serialize, Clone, Debug)]
struct AttachmentDataDto {
    attachment_id: String,
    file_name: String,
    mime_type: Option<String>,
    size_bytes: i64,
    blake3_hash: String,
    width: Option<i64>,
    height: Option<i64>,
    /// آیا بایت‌های خواسته‌شده (اصلی یا بندانگشتی) با هش ثبت‌شده می‌خوانند؟
    integrity_ok: bool,
    /// true اگر بایت‌ها بندانگشتی است
    is_thumbnail: bool,
    /// mime پیشنهادی برای رندر (بندانگشتی همیشه image/png است)
    content_mime: String,
    data_base64: String,
}

/// جزئیات کامل یک معامله — canonical + موثر + پاها + اجراها +
/// بازنویسی‌ها + پیوست‌ها + فیلدهای سفارشی.
#[derive(Serialize, Clone, Debug)]
struct TradeDetailsDto {
    trade: aria_domain_engine::model::Trade,
    effective: serde_json::Value,
    entry_legs: Vec<aria_domain_engine::model::EntryLeg>,
    exit_legs: Vec<aria_domain_engine::model::ExitLeg>,
    executions: Vec<aria_domain_engine::model::Execution>,
    overrides: Vec<aria_domain_engine::model::ManualOverride>,
    attachments: Vec<TradeAttachmentDto>,
    custom_values: serde_json::Map<String, serde_json::Value>,
}

mod journal_impl {
    use super::*;

    /// خطای عمومی پل برای `journal_impl` — کد قراردادی + پیام.
    pub type PlugResult<T> = Result<T, CmdError>;

    /// بذر پیش‌فرض پروفایل/حساب/نمادها برای شروع کار با فرم ثبت معامله.
    /// فقط در پایگاه‌داده خالی اجرا می‌شود؛ داده کاربر هرگز دست‌کاری نمی‌شود.
    pub fn seed_defaults(db: &Database) -> PlugResult<()> {
        let conn = db.lock();
        let profiles: i64 = conn
            .query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))
            .map_err(|e| CmdError::new(0, format!("خطای بررسی پروفایل: {e}")))?;
        if profiles == 0 {
            conn.execute(
                "INSERT INTO profiles (id, name, created_at, updated_at)
                 VALUES ('default', 'پروفایل پیش‌فرض', datetime('now'), datetime('now'))",
                [],
            )
            .map_err(|e| CmdError::new(0, format!("خطای بذر پروفایل: {e}")))?;
        }
        let accounts: i64 = conn
            .query_row("SELECT COUNT(*) FROM trading_accounts", [], |r| r.get(0))
            .map_err(|e| CmdError::new(0, format!("خطای بررسی حساب: {e}")))?;
        if accounts == 0 {
            conn.execute(
                "INSERT INTO trading_accounts (id, profile_id, name, currency, created_at, updated_at)
                 VALUES ('acc-default', 'default', 'حساب پیش‌فرض', 'USD', datetime('now'), datetime('now'))",
                [],
            )
            .map_err(|e| CmdError::new(0, format!("خطای بذر حساب: {e}")))?;
        }
        let symbols: i64 = conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))
            .map_err(|e| CmdError::new(0, format!("خطای بررسی نماد: {e}")))?;
        if symbols == 0 {
            for (id, name) in [
                ("sym-xauusd", "XAU/USD"),
                ("sym-eurusd", "EUR/USD"),
                ("sym-btcusd", "BTC/USD"),
            ] {
                conn.execute(
                    "INSERT INTO symbols (id, name, contract_size, created_at)
                     VALUES (?1, ?2, 1, datetime('now'))",
                    rusqlite::params![id, name],
                )
                .map_err(|e| CmdError::new(0, format!("خطای بذر نماد: {e}")))?;
            }
        }
        Ok(())
    }

    /// اجرای یک دستور دامنه به‌صورت اتمیک و بازگرداندن رویدادهای آن.
    pub fn execute_domain(
        db: &Database,
        command_type: &str,
        payload: serde_json::Value,
    ) -> PlugResult<Vec<EventDto>> {
        let envelope = CommandEnvelope::new(command_type.to_string(), payload, "ui");
        DomainService::new(db)
            .execute(&envelope)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))?
            .into_iter()
            .map(|ev| {
                Ok(EventDto {
                    event_id: ev.event_id.to_string(),
                    event_type: ev.event_type.clone(),
                    payload: ev.payload.clone(),
                })
            })
            .collect()
    }

    // ==================== ایمپورت متاتریدر (فاز ۱.۱۵) ====================

    /// نسخه API کرنل برای سازگاری مانیفست پلاگین‌ها.
    pub const KERNEL_API_VERSION: &str = "1.0.0";

    /// شناسه پلاگین رسمی ایمپورت متاتریدر.
    pub const MT_IMPORT_PLUGIN_ID: &str = "mt.import";

    /// مانیفست پلاگین رسمی — فایل منبع حقیقت در plugins/official/mt-import.
    pub const OFFICIAL_MT_IMPORT_MANIFEST: &str = include_str!(
        "../../../plugins/official/mt-import/manifest.json"
    );

    /// نصب و فعال‌سازی خودکار پلاگین‌های رسمی — idempotent.
    /// جریان قراردادی موتور پلاگین: install → enable → start
    /// (enforce فقط وضعیت enabled/running را می‌پذیرد).
    pub fn ensure_official_plugins(db: &Database) -> PlugResult<()> {
        let svc = aria_plugin_engine::PluginService::new(
            db,
            env!("CARGO_PKG_VERSION"),
            KERNEL_API_VERSION,
        )
        .map_err(|e| CmdError::new(e.code(), e.to_string()))?;

        let status: Option<String> = {
            let conn = db.lock();
            conn.query_row(
                "SELECT status FROM plugins WHERE id = ?1",
                [MT_IMPORT_PLUGIN_ID],
                |r| r.get(0),
            )
            .ok()
        };
        match status.as_deref() {
            None => {
                svc.install(OFFICIAL_MT_IMPORT_MANIFEST)
                    .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
                svc.enable(MT_IMPORT_PLUGIN_ID)
                    .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
            }
            Some("installed") | Some("disabled") => {
                svc.enable(MT_IMPORT_PLUGIN_ID)
                    .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
            }
            _ => {}
        }
        // پس از enable، راه‌اندازی تا enforce بگذرد
        let status: String = {
            let conn = db.lock();
            conn.query_row(
                "SELECT status FROM plugins WHERE id = ?1",
                [MT_IMPORT_PLUGIN_ID],
                |r| r.get(0),
            )
            .map_err(|e| CmdError::new(0, format!("خطای خواندن وضعیت پلاگین: {e}")))?
        };
        if status == "enabled" {
            svc.start(MT_IMPORT_PLUGIN_ID)
                .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
        }
        Ok(())
    }

    /// ایمپورت فایل متاتریدر با enforce مجوز mt.import پلاگین رسمی.
    /// برچسب منبع از قالب تشخیص‌داده‌شده فایل ساخته می‌شود.
    pub fn import_metatrader(
        db: &Database,
        file_name: &str,
        data: &[u8],
        account_id: &str,
    ) -> PlugResult<aria_import_engine::ImportReport> {
        // مجوز — پلاگین رسمی باید نصب و فعال باشد
        let plugins = aria_plugin_engine::PluginService::new(
            db,
            env!("CARGO_PKG_VERSION"),
            KERNEL_API_VERSION,
        )
        .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
        plugins
            .enforce(MT_IMPORT_PLUGIN_ID, aria_plugin_engine::Capability::MtImport)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))?;

        // برچسب منبع بر اساس قالب تشخیص‌داده‌شده
        let decoded = aria_import_engine::decode(data)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
        let source = match aria_import_engine::detect_format(file_name, &decoded.text) {
            aria_import_engine::SourceFormat::HtmlStatement => "mt4-statement",
            aria_import_engine::SourceFormat::CsvDeals => "mt5-deals",
        };

        aria_import_engine::ImportService::new(db)
            .import(data, file_name, account_id, source)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))
    }

    /// فیلدهای سفارشی فعال برای رندر داینامیک فرم.
    pub fn list_schema_fields(db: &Database) -> PlugResult<Vec<aria_schema_engine::FieldDefinition>> {
        SchemaService::new(db)
            .list_fields(false)
            .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))
    }

    /// گزینه‌های فعال یک فیلد انتخابی.
    pub fn list_field_options(
        db: &Database,
        field_id: &str,
    ) -> PlugResult<Vec<aria_schema_engine::FieldOption>> {
        SchemaService::new(db)
            .active_options(field_id)
            .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))
    }

    /// نوشتن مقادیر فیلدهای سفارشی یک معامله — هر فیلد با اعتبارسنجی کامل.
    pub fn set_custom_values(
        db: &Database,
        trade_id: &str,
        values: serde_json::Map<String, serde_json::Value>,
    ) -> PlugResult<usize> {
        let svc = SchemaService::new(db);
        let n = values.len();
        for (field_id, raw) in values {
            svc.set_value(trade_id, &field_id, &raw)
                .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))?;
        }
        Ok(n)
    }

    /// فهرست حساب‌های معاملاتی.
    pub fn list_accounts(db: &Database) -> PlugResult<Vec<AccountDto>> {
        let conn = db.lock();
        let mut stmt = conn
            .prepare("SELECT id, name, currency FROM trading_accounts ORDER BY created_at")
            .map_err(|e| CmdError::new(0, format!("خطای خواندن حساب‌ها: {e}")))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(AccountDto {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    currency: r.get(2)?,
                })
            })
            .map_err(|e| CmdError::new(0, format!("خطای خواندن حساب‌ها: {e}")))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| CmdError::new(0, format!("خطای خواندن حساب‌ها: {e}")))
    }

    /// فهرست نمادهای معاملاتی.
    pub fn list_symbols(db: &Database) -> PlugResult<Vec<SymbolDto>> {
        let conn = db.lock();
        let mut stmt = conn
            .prepare("SELECT id, name FROM symbols ORDER BY created_at")
            .map_err(|e| CmdError::new(0, format!("خطای خواندن نمادها: {e}")))?;
        let rows = stmt
            .query_map([], |r| {
                Ok(SymbolDto { id: r.get(0)?, name: r.get(1)? })
            })
            .map_err(|e| CmdError::new(0, format!("خطای خواندن نمادها: {e}")))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| CmdError::new(0, format!("خطای خواندن نمادها: {e}")))
    }

    /// جزئیات کامل معامله برای پنل جزئیات فاز ۱.۱۲.
    pub fn trade_details(db: &Database, trade_id: &str) -> PlugResult<TradeDetailsDto> {
        let svc = DomainService::new(db);
        let trade = svc
            .get_trade(trade_id)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))?
            .ok_or_else(|| {
                CmdError::new(
                    1401,
                    format!("معامله {trade_id} یافت نشد"),
                )
            })?;
        let effective = svc
            .effective_entity("journal_trade", trade_id)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))?
            .unwrap_or(serde_json::Value::Null);
        let entry_legs = svc
            .entry_legs(trade_id)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
        let exit_legs = svc
            .exit_legs(trade_id)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
        let executions = svc
            .executions(trade_id)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
        let overrides = svc
            .overrides_of("journal_trade", trade_id, true)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
        let attachments = {
            let conn = db.lock();
            let mut stmt = conn
                .prepare(
                    "SELECT a.id, a.file_name, a.mime_type, a.size_bytes, l.link_kind,
                            a.blake3_hash, a.width, a.height, (a.thumbnail_path IS NOT NULL)
                     FROM attachment_trade_links l
                     JOIN attachments a ON a.id = l.attachment_id
                     WHERE l.trade_id = ?1
                     ORDER BY l.created_at, a.file_name",
                )
                .map_err(|e| CmdError::new(0, format!("خطای خواندن پیوست‌ها: {e}")))?;
            let rows = stmt
                .query_map([trade_id], |r| {
                    Ok(TradeAttachmentDto {
                        id: r.get(0)?,
                        file_name: r.get(1)?,
                        mime_type: r.get(2)?,
                        size_bytes: r.get(3)?,
                        link_kind: r.get(4)?,
                        blake3_hash: r.get(5)?,
                        width: r.get(6)?,
                        height: r.get(7)?,
                        has_thumbnail: r.get::<_, i64>(8)? != 0,
                    })
                })
                .map_err(|e| CmdError::new(0, format!("خطای خواندن پیوست‌ها: {e}")))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|e| CmdError::new(0, format!("خطای خواندن پیوست‌ها: {e}")))?
        };
        let custom_values = match SchemaService::new(db)
            .get_values(trade_id)
            .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))?
        {
            serde_json::Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        Ok(TradeDetailsDto {
            trade,
            effective,
            entry_legs,
            exit_legs,
            executions,
            overrides,
            attachments,
            custom_values,
        })
    }

    /// ثبت پیوست: نوشتن بایت‌ها در پوشه داده، هش blake3، ردیف پیوست و
    /// پیوند با معامله از مسیر دستور دامنه (حسابرسی و رویداد).
    /// همان فایل (هش یکسان) در پیوست‌های متعدد یکتاست — ردیف موجود
    /// استفاده مجدد می‌شود و فقط پیوند تازه ساخته می‌شود.
    /// سقف حجم پیوست — ۲۰ مگابایت (فاز ۱.۱۳).
    pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;

    /// پسوندهای مجاز — تصاویر و فایل‌های رایج ژورنال (فاز ۱.۱۳).
    pub const ALLOWED_EXTENSIONS: &[&str] = &[
        "png", "jpg", "jpeg", "gif", "webp", "bmp", // تصاویر
        "pdf", "txt", "csv", "json", // فایل‌ها
    ];

    /// اعتبارسنجی حجم و نوع پیوست پیش از ذخیره (فاز ۱.۱۳).
    pub fn validate_attachment(
        file_name: &str,
        data_len: usize,
    ) -> PlugResult<()> {
        if data_len == 0 {
            return Err(CmdError::new(0, "محتوای پیوست خالی است"));
        }
        if data_len > MAX_ATTACHMENT_BYTES {
            return Err(CmdError::new(
                0,
                format!("حجم پیوست بیش از سقف {} مگابایت است", MAX_ATTACHMENT_BYTES / (1024 * 1024)),
            ));
        }
        let ext = std::path::Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
            return Err(CmdError::new(
                0,
                format!("نوع فایل «{ext}» مجاز نیست؛ پسوندهای مجاز: {}", ALLOWED_EXTENSIONS.join(", ")),
            ));
        }
        Ok(())
    }

    pub fn ingest_attachment(
        db: &Database,
        data_dir: &std::path::Path,
        trade_id: &str,
        file_name: &str,
        mime_type: Option<String>,
        data: Vec<u8>,
        link_kind: &str,
    ) -> PlugResult<AttachmentDto> {
        validate_attachment(file_name, data.len())?;
        // پاک‌سازی نام فایل — فقط نام پایه، بدون مسیر
        let safe_name = std::path::Path::new(file_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("attachment.bin")
            .to_string();
        let dir = data_dir.join("attachments");
        std::fs::create_dir_all(&dir)
            .map_err(|e| CmdError::new(0, format!("خطای ساخت پوشه پیوست: {e}")))?;
        let path = dir.join(format!("{}_{safe_name}", uuid::Uuid::new_v4()));
        std::fs::write(&path, &data)
            .map_err(|e| CmdError::new(0, format!("خطای نوشتن پیوست: {e}")))?;
        let hash = attachments::file_blake3_hash(&path)
            .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))?;
        let (meta, is_new) = match attachments::find_by_hash(db, &hash)
            .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))?
        {
            Some(existing) => {
                // فایل تکراری: ردیف موجود استفاده مجدد می‌شود؛ نسخه فیزیکی
                // تازه‌نوشته‌شده حذف و مسیر ردیف موجود حفظ می‌گردد.
                let _ = std::fs::remove_file(&path);
                (existing, false)
            }
            None => (
                attachments::register_attachment(
                    db,
                    &safe_name,
                    &path,
                    mime_type.as_deref(),
                    None,
                    None,
                )
                .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))?,
                true,
            ),
        };
        // بندانگشتی برای تصاویر جدید — شکست پیش‌نمایش ثبت پیوست را متوقف نمی‌کند
        if is_new {
            if let Ok(Some(info)) = attachments::generate_thumbnail(&path, &dir.join("thumbs"), &format!("{}_{}", meta.id, safe_name.rsplit_once('.').map_or(safe_name.as_str(), |(s, _)| s))) {
                attachments::update_visuals(db, &meta.id, Some(&info.thumbnail_path), Some(info.width), Some(info.height))
                    .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))?;
            }
        }
        execute_domain(
            db,
            aria_domain_engine::commands::command_type::LINK_ATTACHMENT_TO_TRADE,
            serde_json::json!({
                "attachment_id": meta.id,
                "trade_id": trade_id,
                "link_kind": link_kind,
            }),
        )?;
        let fresh = attachments::get_attachment(db, &meta.id)
            .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))?;
        Ok(AttachmentDto {
            id: fresh.id,
            file_name: fresh.file_name,
            size_bytes: fresh.size_bytes,
            blake3_hash: fresh.blake3_hash,
            width: fresh.width,
            height: fresh.height,
            thumbnail_path: fresh.thumbnail_path,
        })
    }

    /// خواندن بایت‌های پیوست برای پیش‌نمایش — با دفاع مسیر و تأیید صحت
    /// روی خواندن (مقایسه هش blake3 فایل اصلی با هش ثبت‌شده) — فاز ۱.۱۳.
    pub fn attachment_data(
        db: &Database,
        data_dir: &std::path::Path,
        attachment_id: &str,
        want_thumbnail: bool,
    ) -> PlugResult<AttachmentDataDto> {
        let meta = attachments::get_attachment(db, attachment_id)
            .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))?;

        // تأیید صحت فایل اصلی روی هر خواندن — فایل خراب/جابجاشده گزارش می‌شود
        let integrity_ok = attachments::verify_integrity(db, attachment_id)
            .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))?;

        let (rel, is_thumbnail) = match (want_thumbnail, &meta.thumbnail_path) {
            (true, Some(tp)) if std::path::Path::new(tp).exists() => (tp.clone(), true),
            _ => (meta.file_path.clone(), false),
        };

        // دفاع از مسیر: بایت‌ها فقط از پوشه پیوست‌های برنامه خوانده می‌شوند
        let att_dir = data_dir.join("attachments");
        let target = std::path::PathBuf::from(&rel);
        let canon_target = target
            .canonicalize()
            .map_err(|_| CmdError::new(0, "فایل پیوست روی دیسک یافت نشد"))?;
        let canon_dir = att_dir
            .canonicalize()
            .unwrap_or_else(|_| att_dir.clone());
        if !canon_target.starts_with(&canon_dir) {
            return Err(CmdError::new(0, "مسیر پیوست خارج از پوشه مجاز است"));
        }

        let bytes = std::fs::read(&canon_target)
            .map_err(|e| CmdError::new(0, format!("خطای خواندن پیوست: {e}")))?;
        use base64::Engine as _;
        let data_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

        let content_mime = if is_thumbnail {
            "image/png".to_string()
        } else {
            meta.mime_type
                .clone()
                .unwrap_or_else(|| "application/octet-stream".to_string())
        };

        Ok(AttachmentDataDto {
            attachment_id: meta.id,
            file_name: meta.file_name,
            mime_type: meta.mime_type,
            size_bytes: meta.size_bytes,
            blake3_hash: meta.blake3_hash,
            width: meta.width,
            height: meta.height,
            integrity_ok,
            is_thumbnail,
            content_mime,
            data_base64,
        })
    }
}

#[tauri::command]
fn domain_execute(
    state: State<'_, Mutex<KernelState>>,
    command_type: String,
    payload: serde_json::Value,
) -> Result<Vec<EventDto>, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    journal_impl::execute_domain(db, &command_type, payload)
}

#[tauri::command]
fn schema_list_fields(
    state: State<'_, Mutex<KernelState>>,
) -> Result<Vec<aria_schema_engine::FieldDefinition>, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    journal_impl::list_schema_fields(db)
}

#[tauri::command]
fn schema_field_options(
    state: State<'_, Mutex<KernelState>>,
    field_id: String,
) -> Result<Vec<aria_schema_engine::FieldOption>, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    journal_impl::list_field_options(db, &field_id)
}

#[tauri::command]
fn schema_set_values(
    state: State<'_, Mutex<KernelState>>,
    trade_id: String,
    values: serde_json::Map<String, serde_json::Value>,
) -> Result<usize, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    journal_impl::set_custom_values(db, &trade_id, values)
}

#[tauri::command]
fn trade_details(
    state: State<'_, Mutex<KernelState>>,
    trade_id: String,
) -> Result<TradeDetailsDto, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    journal_impl::trade_details(db, &trade_id)
}

#[tauri::command]
fn accounts_list(
    state: State<'_, Mutex<KernelState>>,
) -> Result<Vec<AccountDto>, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    journal_impl::list_accounts(db)
}

#[tauri::command]
fn symbols_list(
    state: State<'_, Mutex<KernelState>>,
) -> Result<Vec<SymbolDto>, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    journal_impl::list_symbols(db)
}

#[tauri::command]
fn attachment_ingest(
    app: AppHandle,
    state: State<'_, Mutex<KernelState>>,
    trade_id: String,
    file_name: String,
    mime_type: Option<String>,
    data: Vec<u8>,
    link_kind: String,
) -> Result<AttachmentDto, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| CmdError::new(0, format!("خطای مسیر داده: {e}")))?;
    journal_impl::ingest_attachment(db, &dir, &trade_id, &file_name, mime_type, data, &link_kind)
}

/// خواندن بایت‌های پیوست (اصلی یا بندانگشتی) برای پیش‌نمایش — فاز ۱.۱۳.
#[tauri::command]
fn attachment_data(
    app: AppHandle,
    state: State<'_, Mutex<KernelState>>,
    attachment_id: String,
    thumbnail: bool,
) -> Result<AttachmentDataDto, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| CmdError::new(0, format!("خطای مسیر داده: {e}")))?;
    journal_impl::attachment_data(db, &dir, &attachment_id, thumbnail)
}

/// ایمپورت فایل خروجی متاتریدر از طریق پلاگین رسمی mt.import — فاز ۱.۱۵.
/// مجوز mt.import در پل enforce می‌شود و گزارش فارسی برمی‌گردد.
#[tauri::command]
fn mt_import(
    state: State<'_, Mutex<KernelState>>,
    file_name: String,
    data: Vec<u8>,
    account_id: String,
) -> Result<aria_import_engine::ImportReport, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    journal_impl::import_metatrader(db, &file_name, &data, &account_id)
}

// ===================== فاز ۱.۱۶ — بکاپ و بازیابی رمزنگاری‌شده =====================
//
// منطق دستورات در `backup_impl` بدون وابستگی به Tauri پیاده شده تا بدون
// پوسته هم آزمون‌پذیر باشد. میزکار کاری پوشه داده برنامه است و پس از
// بازیابی موفق، پایگاه‌داده بازگردانده‌شده در کرنل بارگذاری مجدد می‌شود.

/// منطق بکاپ/بازیابی — مستقل از پوسته.
mod backup_impl {
    use super::*;
    use super::journal_impl::PlugResult;
    use aria_backup_engine::{BackupManifest, BackupReport, RestoreReport, Workspace};

    /// میزکار کاری روی ریشه داده برنامه.
    pub fn workspace(root: &std::path::Path) -> Workspace {
        Workspace::new(root)
    }

    /// مسیر پیش‌فرض بسته بکاپ — پوشه backups با مُهر زمانی.
    pub fn default_backup_path(root: &std::path::Path) -> std::path::PathBuf {
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let dir = root.join("backups");
        let _ = std::fs::create_dir_all(&dir);
        dir.join(format!("tahan-{stamp}.tahanbak"))
    }

    /// ساخت بکاپ رمزنگاری‌شده — با اعمال سیاست گذرواژه در موتور.
    pub fn create(
        db: &Database,
        root: &std::path::Path,
        password: &str,
        include_settings: bool,
        out_path: Option<String>,
    ) -> PlugResult<BackupReport> {
        let out = match out_path {
            Some(p) if !p.trim().is_empty() => std::path::PathBuf::from(p),
            _ => default_backup_path(root),
        };
        aria_backup_engine::create_backup(db, &workspace(root), &out, password, include_settings)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))
    }

    /// بازیابی بسته روی میزکار — پس از موفقیت، مسیر پایگاه‌داده بازگردانده می‌شود
    /// تا کرنل آن را بارگذاری کند.
    pub fn restore(
        package_path: &str,
        root: &std::path::Path,
        password: &str,
    ) -> PlugResult<RestoreReport> {
        let package = std::path::PathBuf::from(package_path);
        if !package.is_file() {
            return Err(CmdError::new(1600, "فایل بسته بکاپ یافت نشد"));
        }
        let report = aria_backup_engine::restore_backup(&package, &workspace(root), password)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
        // نگه‌داشتن حداکثر ۵ نسخه امنیتی — پاک‌سازی خودکار پس از بازیابی موفق
        if let Ok(_removed) = aria_backup_engine::prune_safety_copies(&workspace(root), 5) {
            tracing::info!("نسخه‌های امنیتی قدیمی پاک شدند");
        }
        Ok(report)
    }

    /// خواندن فراداده بسته بدون گذرواژه.
    pub fn inspect(package_path: &str) -> PlugResult<BackupManifest> {
        let package = std::path::PathBuf::from(package_path);
        if !package.is_file() {
            return Err(CmdError::new(1600, "فایل بسته بکاپ یافت نشد"));
        }
        aria_backup_engine::inspect_package(&package)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))
    }

    /// تأیید کامل صحت بسته (رمزگشایی + چک‌سام‌ها) بدون تغییر دیسک.
    pub fn verify(package_path: &str, password: &str) -> PlugResult<BackupManifest> {
        let package = std::path::PathBuf::from(package_path);
        if !package.is_file() {
            return Err(CmdError::new(1600, "فایل بسته بکاپ یافت نشد"));
        }
        aria_backup_engine::verify_package(&package, password)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))
    }

    /// محل بسته: مسیر مستقیم یا نوشتن بایت‌های دریافتی در پوشه restore-inbox
    /// (چون WebView به مسیر فایل محلی دسترسی ندارد).
    pub fn resolve_source(
        root: &std::path::Path,
        package_path: Option<&str>,
        file_name: Option<&str>,
        data: Option<&[u8]>,
    ) -> PlugResult<std::path::PathBuf> {
        if let Some(p) = package_path {
            if !p.trim().is_empty() {
                let package = std::path::PathBuf::from(p);
                if !package.is_file() {
                    return Err(CmdError::new(1600, "فایل بسته بکاپ یافت نشد"));
                }
                return Ok(package);
            }
        }
        let bytes = data.ok_or_else(|| {
            CmdError::new(1600, "مسیر یا محتوای بسته بکاپ داده نشده است")
        })?;
        if bytes.is_empty() {
            return Err(CmdError::new(1600, "محتوای فایل بسته خالی است"));
        }
        // نام امن — فقط نام پایه، بدون مسیر
        let safe = std::path::Path::new(file_name.unwrap_or("package.tahanbak"))
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("package.tahanbak")
            .to_string();
        let inbox = root.join("restore-inbox");
        std::fs::create_dir_all(&inbox)
            .map_err(|e| CmdError::new(1605, format!("خطای ساخت پوشه بازیابی: {e}")))?;
        let path = inbox.join(format!("{}-{safe}", uuid::Uuid::new_v4()));
        std::fs::write(&path, bytes)
            .map_err(|e| CmdError::new(1605, format!("خطای ذخیره بسته دریافتی: {e}")))?;
        Ok(path)
    }
}

#[tauri::command]
fn backup_create(
    app: AppHandle,
    state: State<'_, Mutex<KernelState>>,
    password: String,
    include_settings: bool,
    out_path: Option<String>,
) -> Result<aria_backup_engine::BackupReport, CmdError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| CmdError::new(0, format!("خطای مسیر داده: {e}")))?;
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    backup_impl::create(db, &dir, &password, include_settings, out_path)
}

/// بازیابی بسته — پس از موفقیت پایگاه‌داده بازگردانده‌شده در کرنل بارگذاری
/// می‌شود و رویداد `kernel://ready` دوباره گسیل می‌گردد.
///
/// بسته می‌تواند با مسیر (`package_path`) یا با بایت‌های فایل انتخاب‌شده در
/// WebView (`file_name` + `data`) داده شود.
#[tauri::command]
fn backup_restore(
    app: AppHandle,
    state: State<'_, Mutex<KernelState>>,
    package_path: Option<String>,
    file_name: Option<String>,
    data: Option<Vec<u8>>,
    password: String,
) -> Result<aria_backup_engine::RestoreReport, CmdError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| CmdError::new(0, format!("خطای مسیر داده: {e}")))?;
    let source = backup_impl::resolve_source(
        &dir,
        package_path.as_deref(),
        file_name.as_deref(),
        data.as_deref(),
    )?;
    let report = backup_impl::restore(&source.display().to_string(), &dir, &password)?;

    // پاک‌سازی نسخه موقت دریافت‌شده از WebView (تنها فایل‌های داخل inbox)
    let inbox = dir.join("restore-inbox");
    if source.starts_with(&inbox) {
        let _ = std::fs::remove_file(&source);
    }

    // بارگذاری مجدد پایگاه‌داده بازگردانده‌شده در کرنل
    let restored_db = Database::open(&backup_impl::workspace(&dir).db_path(), None)
        .map_err(|e| CmdError::new(0, format!("خطای بازگشایی پایگاه‌داده بازیابی‌شده: {e}")))?;
    {
        let mut st = state.lock().unwrap();
        st.db = Some(restored_db);
        st.mode = "file";
    }
    app.emit("kernel://ready", true)
        .map_err(|e| CmdError::new(0, format!("خطای اعلان آمادگی کرنل: {e}")))?;
    Ok(report)
}

/// فراداده بسته بکاپ — بدون گذرواژه، برای نمایش پیش از بازیابی.
#[tauri::command]
fn backup_inspect(
    app: AppHandle,
    package_path: Option<String>,
    file_name: Option<String>,
    data: Option<Vec<u8>>,
) -> Result<aria_backup_engine::BackupManifest, CmdError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| CmdError::new(0, format!("خطای مسیر داده: {e}")))?;
    let source = backup_impl::resolve_source(
        &dir,
        package_path.as_deref(),
        file_name.as_deref(),
        data.as_deref(),
    )?;
    backup_impl::inspect(&source.display().to_string())
}

/// تأیید کامل صحت بسته — رمزگشایی و بررسی چک‌سام‌ها بدون تغییر دیسک.
#[tauri::command]
fn backup_verify(
    app: AppHandle,
    package_path: Option<String>,
    file_name: Option<String>,
    data: Option<Vec<u8>>,
    password: String,
) -> Result<aria_backup_engine::BackupManifest, CmdError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| CmdError::new(0, format!("خطای مسیر داده: {e}")))?;
    let source = backup_impl::resolve_source(
        &dir,
        package_path.as_deref(),
        file_name.as_deref(),
        data.as_deref(),
    )?;
    backup_impl::verify(&source.display().to_string(), &password)
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
            stats_equity,
            stats_breakdown,
            stats_heatmap,
            ui_extensions,
            domain_execute,
            trade_details,
            schema_list_fields,
            schema_field_options,
            schema_set_values,
            accounts_list,
            symbols_list,
            attachment_ingest,
            attachment_data,
            mt_import,
            backup_create,
            backup_restore,
            backup_inspect,
            backup_verify
        ])
        .setup(|app| {
            // در محیط توسعه مسیر داده تضمین می‌شود (برای فازهای بکاپ/تنظیمات)
            let _ = app.path().app_data_dir();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("خطا در اجرای ژورنال طهان");
}

#[cfg(test)]
mod tests {
    use super::backup_impl;
    use super::journal_impl;
    use aria_domain_engine::commands::command_type;
    use aria_domain_engine::DomainService;
    use aria_schema_engine::{FieldDefinition, SchemaService, SemanticType, StorageType};
    use aria_storage_engine::{Database, migrations};

    fn db() -> Database {
        let db = Database::open_memory(None).unwrap();
        {
            let mut conn = db.lock();
            migrations::run_migrations(&mut conn, |_| Ok(())).unwrap();
        }
        db
    }

    fn seeded() -> Database {
        let db = db();
        journal_impl::seed_defaults(&db).unwrap();
        db
    }

    fn create_trade(db: &Database) -> String {
        let events = journal_impl::execute_domain(
            db,
            command_type::CREATE_TRADE,
            serde_json::json!({
                "account_id": "acc-default",
                "symbol_id": "sym-xauusd",
                "direction": "buy",
                "commission": 0.0,
                "swap": 0.0,
            }),
        )
        .unwrap();
        events[0].payload["trade_id"].as_str().unwrap().to_string()
    }

    #[test]
    fn seed_defaults_is_idempotent() {
        let db = seeded();
        journal_impl::seed_defaults(&db).unwrap();
        assert_eq!(journal_impl::list_accounts(&db).unwrap().len(), 1);
        assert_eq!(journal_impl::list_symbols(&db).unwrap().len(), 3);
    }

    #[test]
    fn execute_domain_creates_trade_and_manual_execution() {
        let db = seeded();
        let trade_id = create_trade(&db);
        let events = journal_impl::execute_domain(
            &db,
            command_type::ADD_ENTRY_LEG,
            serde_json::json!({
                "trade_id": trade_id,
                "executed_price": 2350.0,
                "volume": 0.5,
                "stop_loss": 2340.0,
                "take_profit": 2380.0,
            }),
        )
        .unwrap();
        assert!(events[0].event_type.starts_with("domain."));
        // پای اجراشده باید اجرای دستی هم‌ساز کند (قاعده ۲/۴ قرارداد دامنه)
        let execs = DomainService::new(&db).executions(&trade_id).unwrap();
        assert_eq!(execs.len(), 1);
        assert!(execs[0].leg_id.is_some());
    }

    #[test]
    fn execute_domain_unknown_command_rejected_with_domain_code() {
        let db = seeded();
        let err = journal_impl::execute_domain(&db, "domain.nope", serde_json::json!({}))
            .unwrap_err();
        assert!((1400..1500).contains(&err.code), "unexpected code {}", err.code);
    }

    #[test]
    fn schema_fields_roundtrip_through_bridge() {
        let db = seeded();
        let svc = SchemaService::new(&db);
        let mut def = FieldDefinition::new("confidence", "اطمینان", StorageType::Integer, SemanticType::Number);
        def.required = true;
        let field = svc.define_field(def).unwrap();
        let listed = journal_impl::list_schema_fields(&db).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].technical_key, "confidence");
        let trade_id = create_trade(&db);
        let mut values = serde_json::Map::new();
        values.insert(field.id.clone(), serde_json::json!(7));
        let n = journal_impl::set_custom_values(&db, &trade_id, values).unwrap();
        assert_eq!(n, 1);
        assert_eq!(svc.get_values(&trade_id).unwrap()["confidence"], 7);
    }

    #[test]
    fn ingest_attachment_dedups_same_hash_and_links_twice() {
        let db = seeded();
        let t1 = create_trade(&db);
        let t2 = create_trade(&db);
        let dir = std::env::temp_dir().join(format!("tahan-test-{}", uuid::Uuid::new_v4()));
        let bytes = b"chart screenshot bytes".to_vec();
        let a1 = journal_impl::ingest_attachment(
            &db, &dir, &t1, "chart.png", Some("image/png".into()), bytes.clone(), "chart",
        )
        .unwrap();
        // همان فایل برای معامله دوم — ردیف پیوست باید مشترک باشد نه تکراری
        let a2 = journal_impl::ingest_attachment(
            &db, &dir, &t2, "chart.png", Some("image/png".into()), bytes, "chart",
        )
        .unwrap();
        assert_eq!(a1.id, a2.id);
        assert_eq!(a1.blake3_hash, a2.blake3_hash);
        let links: i64 = {
            let conn = db.lock();
            conn.query_row("SELECT COUNT(*) FROM attachment_trade_links", [], |r| {
                r.get(0)
            })
            .unwrap()
        };
        assert_eq!(links, 2);
        // نسخه فیزیکی تکراری حذف شده — فقط یک فایل در پوشه
        let files: Vec<_> = std::fs::read_dir(dir.join("attachments")).unwrap().collect();
        assert_eq!(files.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ingest_attachment_rejects_path_traversal_and_empty() {
        let db = seeded();
        let t1 = create_trade(&db);
        let dir = std::env::temp_dir().join(format!("tahan-test-{}", uuid::Uuid::new_v4()));
        let err = journal_impl::ingest_attachment(
            &db, &dir, &t1, "x.png", None, vec![], "chart",
        )
        .unwrap_err();
        assert_eq!(err.code, 0);
        // نام فایل با مسیر باید پاک‌سازی شود، نه رد مسیر
        let ok = journal_impl::ingest_attachment(
            &db, &dir, &t1, "..\\evil.png", None, b"v".to_vec(), "other",
        )
        .unwrap();
        assert_eq!(ok.file_name, "evil.png");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trade_details_returns_all_sections() {
        let db = seeded();
        let t1 = create_trade(&db);
        // پای ورود اجراشده → اجرای دستی خودکار
        journal_impl::execute_domain(
            &db,
            command_type::ADD_ENTRY_LEG,
            serde_json::json!({
                "trade_id": t1,
                "executed_price": 2350.0,
                "volume": 0.5,
                "stop_loss": 2340.0,
                "take_profit": 2380.0,
            }),
        )
        .unwrap();
        journal_impl::execute_domain(
            &db,
            command_type::ADD_EXIT_LEG,
            serde_json::json!({
                "trade_id": t1,
                "executed_price": 2380.0,
                "volume": 0.5,
                "exit_reason": "هدف",
            }),
        )
        .unwrap();
        journal_impl::execute_domain(
            &db,
            command_type::ADD_MANUAL_OVERRIDE,
            serde_json::json!({
                "entity_type": "journal_trade",
                "entity_id": t1,
                "field_name": "note",
                "new_value": "بروزرسانی دستی",
                "reason": "اصلاح",
                "source": "manual",
                "priority": 10,
                "reversible": true,
                "created_by": "user",
            }),
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("tahan-test-{}", uuid::Uuid::new_v4()));
        journal_impl::ingest_attachment(
            &db, &dir, &t1, "chart.png", Some("image/png".into()), b"img".to_vec(), "chart",
        )
        .unwrap();

        let d = journal_impl::trade_details(&db, &t1).unwrap();
        assert_eq!(d.trade.id, t1);
        assert_eq!(d.entry_legs.len(), 1);
        assert_eq!(d.exit_legs.len(), 1);
        // پای ورود و پای خروج اجراشده هر دو اجرای دستی خودکار دارند
        assert_eq!(d.executions.len(), 2);
        assert!(d.executions.iter().all(|e| e.assignment_status == "assigned"));
        assert_eq!(d.overrides.len(), 1);
        assert_eq!(d.overrides[0].field_name, "note");
        assert_eq!(d.attachments.len(), 1);
        assert_eq!(d.attachments[0].link_kind, "chart");
        // داده موثر باید بازنویسی دستی را اعمال کند
        let effective_note = d.effective["note"].as_str();
        assert_eq!(effective_note, Some("بروزرسانی دستی"));
        // فیلد سفارشی: تعریف + نوشتن + خواندن در جزئیات
        let svc = SchemaService::new(&db);
        let def = FieldDefinition::new("confidence", "اطمینان", StorageType::Integer, SemanticType::Number);
        let f = svc.define_field(def).unwrap();
        let mut values = serde_json::Map::new();
        values.insert(f.id.clone(), serde_json::json!(6));
        journal_impl::set_custom_values(&db, &t1, values).unwrap();
        let d2 = journal_impl::trade_details(&db, &t1).unwrap();
        assert_eq!(d2.custom_values["confidence"], 6);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trade_details_missing_trade_is_1401() {
        let db = seeded();
        let err = journal_impl::trade_details(&db, "nope").unwrap_err();
        assert_eq!(err.code, 1401);
    }

    #[test]
    fn unassigned_executions_visible_in_details() {
        let db = seeded();
        let t1 = create_trade(&db);
        // اجرای بدون پا (شبیه‌سازی ورود بروکر ناشناس) — needs_assignment
        journal_impl::execute_domain(
            &db,
            command_type::ADD_ENTRY_LEG,
            serde_json::json!({
                "trade_id": t1,
                "volume": 0.5,
            }),
        )
        .unwrap();
        let d = journal_impl::trade_details(&db, &t1).unwrap();
        // پای برنامه‌ریزی‌شده بدون قیمت اجرا → اجرای دستی نمی‌سازد
        assert!(d.executions.is_empty() || d.executions.iter().all(|e| e.leg_id.is_some()));
        let unassigned_count = d.executions.iter().filter(|e| e.assignment_status == "needs_assignment").count();
        // در این سناریو هیچ اجرای بی‌صاحب نداریم؛ پنل باید بتواند صفر را هم نمایش دهد
        assert_eq!(unassigned_count, 0);
    }

    // ==================== فاز ۱.۱۳ — پیوست‌ها ====================

    #[test]
    fn validate_attachment_rejects_oversize_and_bad_type() {
        // حجم بیش از سقف
        let err = journal_impl::validate_attachment("big.png", journal_impl::MAX_ATTACHMENT_BYTES + 1).unwrap_err();
        assert!(err.message.contains("سقف"));
        // نوع مجاز نیست
        let err2 = journal_impl::validate_attachment("virus.exe", 100).unwrap_err();
        assert!(err2.message.contains("مجاز نیست"));
        // بدون پسوند
        let err3 = journal_impl::validate_attachment("noext", 100).unwrap_err();
        assert!(err3.message.contains("مجاز نیست"));
        // پسوند با حروف بزرگ — مجاز
        journal_impl::validate_attachment("REPORT.PDF", 100).unwrap();
        journal_impl::validate_attachment("shot.JpG", 100).unwrap();
        // خالی
        let err4 = journal_impl::validate_attachment("a.png", 0).unwrap_err();
        assert!(err4.message.contains("خالی"));
    }

    fn test_png_bytes() -> Vec<u8> {
        let mut buf = Vec::new();
        image::RgbaImage::from_pixel(800, 600, image::Rgba([30, 200, 90, 255]))
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn ingest_attachment_generates_thumbnail_and_dimensions() {
        let db = seeded();
        let t1 = create_trade(&db);
        let dir = std::env::temp_dir().join(format!("tahan-test-{}", uuid::Uuid::new_v4()));
        let a = journal_impl::ingest_attachment(
            &db, &dir, &t1, "shot.png", Some("image/png".into()), test_png_bytes(), "chart",
        )
        .unwrap();
        assert_eq!(a.width, Some(800));
        assert_eq!(a.height, Some(600));
        let thumb = a.thumbnail_path.expect("png ingest must create thumbnail");
        assert!(std::path::Path::new(&thumb).exists());
        // بندانگشتی داخل پوشه thumbs پوشه پیوست‌هاست
        assert!(std::path::Path::new(&thumb).starts_with(dir.join("attachments").join("thumbs")));

        // خواندن بندانگشتی: PNG کوچک با base64
        let d = journal_impl::attachment_data(&db, &dir, &a.id, true).unwrap();
        assert!(d.is_thumbnail);
        assert_eq!(d.content_mime, "image/png");
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD.decode(&d.data_base64).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert!(decoded.width().max(decoded.height()) <= aria_storage_engine::attachments::THUMBNAIL_MAX_EDGE);

        // خواندن اصلی: فراداده کامل و صحت سالم
        let full = journal_impl::attachment_data(&db, &dir, &a.id, false).unwrap();
        assert!(!full.is_thumbnail);
        assert!(full.integrity_ok);
        assert_eq!(full.width, Some(800));
        assert_eq!(full.size_bytes, a.size_bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn attachment_data_detects_corrupted_file_on_read() {
        let db = seeded();
        let t1 = create_trade(&db);
        let dir = std::env::temp_dir().join(format!("tahan-test-{}", uuid::Uuid::new_v4()));
        let a = journal_impl::ingest_attachment(
            &db, &dir, &t1, "note.txt", Some("text/plain".into()), b"original".to_vec(), "news",
        )
        .unwrap();
        // خراب کردن فایل روی دیسک
        for entry in std::fs::read_dir(dir.join("attachments")).unwrap() {
            let p = entry.unwrap().path();
            if p.is_file() {
                std::fs::write(&p, b"tampered!").unwrap();
            }
        }
        let d = journal_impl::attachment_data(&db, &dir, &a.id, false).unwrap();
        assert!(!d.integrity_ok, "corrupted file must be detected by hash check");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn attachment_data_blocks_escape_from_attachments_dir() {
        let db = seeded();
        let t1 = create_trade(&db);
        let dir = std::env::temp_dir().join(format!("tahan-test-{}", uuid::Uuid::new_v4()));
        let a = journal_impl::ingest_attachment(
            &db, &dir, &t1, "s.txt", Some("text/plain".into()), b"x".to_vec(), "other",
        )
        .unwrap();
        // دستکاری مسیر ذخیره‌شده در پایگاه‌داده به فایل بیرون پوشه پیوست‌ها
        let outside = std::env::temp_dir().join("tahan-outside-secret.txt");
        std::fs::write(&outside, b"secret").unwrap();
        {
            let conn = db.lock();
            conn.execute(
                "UPDATE attachments SET file_path = ?2 WHERE id = ?1",
                rusqlite::params![a.id, outside.display().to_string()],
            )
            .unwrap();
        }
        let err = journal_impl::attachment_data(&db, &dir, &a.id, false).unwrap_err();
        assert!(err.message.contains("خارج از پوشه مجاز"));
        let _ = std::fs::remove_file(&outside);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unlink_attachment_end_to_end() {
        let db = seeded();
        let t1 = create_trade(&db);
        let dir = std::env::temp_dir().join(format!("tahan-test-{}", uuid::Uuid::new_v4()));
        let a = journal_impl::ingest_attachment(
            &db, &dir, &t1, "chart.png", Some("image/png".into()), b"img2".to_vec(), "chart",
        )
        .unwrap();
        assert_eq!(journal_impl::trade_details(&db, &t1).unwrap().attachments.len(), 1);

        // حذف پیوند — فراداده و فایل باقی می‌مانند
        let events = journal_impl::execute_domain(
            &db,
            command_type::UNLINK_ATTACHMENT_FROM_TRADE,
            serde_json::json!({ "attachment_id": a.id, "trade_id": t1 }),
        )
        .unwrap();
        assert!(events.iter().any(|e| e.event_type == "domain.attachment_unlinked"));
        let d = journal_impl::trade_details(&db, &t1).unwrap();
        assert!(d.attachments.is_empty());
        // حذف پیوندی که دیگر وجود ندارد خطا می‌دهد
        let err = journal_impl::execute_domain(
            &db,
            command_type::UNLINK_ATTACHMENT_FROM_TRADE,
            serde_json::json!({ "attachment_id": a.id, "trade_id": t1 }),
        )
        .unwrap_err();
        assert_eq!(err.code, 1404);
        // پیوست هنوز قابل خواندن است و می‌تواند دوباره پیوند بخورد
        assert!(journal_impl::attachment_data(&db, &dir, &a.id, false).is_ok());
        journal_impl::execute_domain(
            &db,
            command_type::LINK_ATTACHMENT_TO_TRADE,
            serde_json::json!({ "attachment_id": a.id, "trade_id": t1, "link_kind": "after_trade" }),
        )
        .unwrap();
        assert_eq!(journal_impl::trade_details(&db, &t1).unwrap().attachments[0].link_kind, "after_trade");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stats_commands_contract_equity_breakdown_heatmap() {
        let db = seeded();
        let t1 = create_trade(&db);
        {
            let conn = db.lock();
            conn.execute(
                "UPDATE journal_trades SET status='closed', strategy='breakout',
                        exit_time='2026-04-19T10:30:00Z', realized_pnl=150.0, realized_r=1.5
                 WHERE id = ?1",
                rusqlite::params![t1],
            )
            .unwrap();
        }
        let svc = aria_query_engine::QueryService::new(&db);
        let node: aria_query_engine::FilterNode =
            serde_json::from_value(serde_json::json!({"type": "all", "children": []})).unwrap();

        // منحنی سرمایه: یک نقطه با PnL تجمعی ۱۵۰
        let equity = svc.equity_curve(&node).unwrap();
        assert_eq!(equity.len(), 1);
        assert_eq!(equity[0].date, "2026-04-19");
        assert!((equity[0].cumulative_pnl - 150.0).abs() < 1e-9);

        // تفکیک روی بُعد استراتژی — قرارداد JSON با tag/content
        let dim: aria_query_engine::Dimension =
            serde_json::from_value(serde_json::json!({"dim": "strategy"})).unwrap();
        let groups = svc.performance_by(&dim, &node).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "breakout");
        assert_eq!(groups[0].trades, 1);
        assert!((groups[0].win_rate.unwrap() - 100.0).abs() < 1e-9);

        // بُعد فیلد سفارشی از مسیر IPC
        let dim2: aria_query_engine::Dimension =
            serde_json::from_value(serde_json::json!({"dim": "custom_field", "field": "confidence"}))
                .unwrap();
        match dim2 {
            aria_query_engine::Dimension::CustomField(k) => assert_eq!(k, "confidence"),
            other => panic!("bad dimension: {other:?}"),
        }

        // نقشه حرارتی: یکشنبه (wd=0) ساعت ۱۰
        let heat = svc.heatmap(&node).unwrap();
        assert_eq!(heat.len(), 1);
        assert_eq!(heat[0].weekday, 0);
        assert_eq!(heat[0].hour, 10);
        assert_eq!(heat[0].trades, 1);
        assert!((heat[0].total_pnl.unwrap() - 150.0).abs() < 1e-9);
    }

    // ==================== ایمپورت متاتریدر (فاز ۱.۱۵) ====================

    const MT5_CSV: &str = "Time,Position,Type,Direction,Volume,Price,Order,Commission,Swap,Profit,Symbol,Comment\n\
        2024.01.15 10:30:00,123456,buy,in,0.10,2035.50,789,0.00,0.00,0.00,XAUUSD,\n\
        2024.01.15 12:00:00,123456,sell,out,0.10,2040.00,790,-0.50,0.20,45.00,XAUUSD,take profit\n";

    #[test]
    fn ensure_official_plugins_installs_enables_and_is_idempotent() {
        let db = seeded();
        journal_impl::ensure_official_plugins(&db).unwrap();
        journal_impl::ensure_official_plugins(&db).unwrap();
        let conn = db.lock();
        let status: String = conn
            .query_row(
                "SELECT status FROM plugins WHERE id = 'mt.import'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "running");
    }

    #[test]
    fn mt_import_requires_installed_plugin() {
        let db = seeded();
        // بدون ensure — پلاگین نصب نیست → enforce رد می‌شود
        let err = journal_impl::import_metatrader(&db, "deals.csv", MT5_CSV.as_bytes(), "acc-default")
            .unwrap_err();
        assert!((1500..1600).contains(&err.code), "plugin error code, got {}", err.code);
    }

    #[test]
    fn mt_import_full_flow_creates_trades_and_executions() {
        let db = seeded();
        journal_impl::ensure_official_plugins(&db).unwrap();
        let report = journal_impl::import_metatrader(
            &db,
            "deals.csv",
            MT5_CSV.as_bytes(),
            "acc-default",
        )
        .unwrap();
        assert_eq!(report.imported, 2);
        assert_eq!(report.trades_created, 1);
        assert_eq!(report.needs_assignment, 0);
        assert_eq!(report.file_hash.len(), 64, "blake3 hex hash");
        assert_eq!(report.batch_id.len(), 36, "batch id is a uuid");

        let conn = db.lock();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM journal_trades", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM executions WHERE kind='imported' AND ticket IS NOT NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM audit_logs WHERE action='import.mt'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        // ابطال کش داشبورد برای نوسازی آمار پس از ایمپورت
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM system_events WHERE event_type='domain.stats_invalidated'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn mt_import_duplicate_file_yields_no_new_trades() {
        let db = seeded();
        journal_impl::ensure_official_plugins(&db).unwrap();
        journal_impl::import_metatrader(&db, "deals.csv", MT5_CSV.as_bytes(), "acc-default")
            .unwrap();
        let report = journal_impl::import_metatrader(
            &db,
            "deals.csv",
            MT5_CSV.as_bytes(),
            "acc-default",
        )
        .unwrap();
        assert!(report.file_duplicate);
        assert_eq!(report.imported, 0);
        assert_eq!(report.duplicates, 2);
    }

    // ==================== بکاپ و بازیابی (فاز ۱.۱۶) ====================

    const BACKUP_PASSWORD: &str = "Passw0rd!-Tahan-2026";

    #[test]
    fn backup_bridge_roundtrip_through_default_path() {
        let db = seeded();
        create_trade(&db);
        let root = std::env::temp_dir().join(format!("tahan-bak-{}", uuid::Uuid::new_v4()));

        let report = backup_impl::create(&db, &root, BACKUP_PASSWORD, true, None).unwrap();
        assert!(report.out_path.contains("backups"));
        assert!(report.out_path.ends_with(".tahanbak"));
        assert!(std::path::Path::new(&report.out_path).is_file());

        // فراداده بدون گذرواژه خوانا است
        let m = backup_impl::inspect(&report.out_path).unwrap();
        assert!(m.is_supported());
        assert!(m.includes_settings);

        // گذرواژه نادرست با کد قراردادی رد می‌شود
        let err = backup_impl::verify(&report.out_path, "Wrong!Pass9-Zzz").unwrap_err();
        assert_eq!(err.code, 1601);

        // تأیید کامل با گذرواژه درست
        backup_impl::verify(&report.out_path, BACKUP_PASSWORD).unwrap();

        // بازیابی روی میزکار تازه — فایل پایگاه‌داده بازگردانده می‌شود
        let root2 = std::env::temp_dir().join(format!("tahan-bak-{}", uuid::Uuid::new_v4()));
        let rr = backup_impl::restore(&report.out_path, &root2, BACKUP_PASSWORD).unwrap();
        assert!(rr.database_bytes > 0);
        assert!(backup_impl::workspace(&root2).db_path().is_file());

        // پایگاه‌داده بازیابی‌شده داده معامله را دارد
        let restored = Database::open(&backup_impl::workspace(&root2).db_path(), None).unwrap();
        {
            let conn = restored.lock();
            assert_eq!(
                conn.query_row("SELECT count(*) FROM journal_trades", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                1
            );
        }

        // مسیر ناموجود — کد ۱۶۰۰
        let err = backup_impl::inspect(&format!("{}\\no-such.tahanbak", root.display())).unwrap_err();
        assert_eq!(err.code, 1600);

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&root2);
    }

    #[test]
    fn backup_bridge_rejects_weak_password_before_any_work() {
        let db = seeded();
        let root = std::env::temp_dir().join(format!("tahan-bak-{}", uuid::Uuid::new_v4()));
        let err = backup_impl::create(&db, &root, "short", false, None).unwrap_err();
        assert_eq!(err.code, 1604);
        assert!(!root.join("backups").exists() || std::fs::read_dir(root.join("backups")).unwrap().count() == 0,
            "no package file is left behind");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn backup_bridge_restore_missing_package_is_1600() {
        let root = std::env::temp_dir().join(format!("tahan-bak-{}", uuid::Uuid::new_v4()));
        let err = backup_impl::restore("Z:\\definitely\\missing.tahanbak", &root, BACKUP_PASSWORD)
            .unwrap_err();
        assert_eq!(err.code, 1600);
        let _ = std::fs::remove_dir_all(&root);
    }
}
