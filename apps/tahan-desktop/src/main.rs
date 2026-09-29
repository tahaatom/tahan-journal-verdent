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
        mode: if st.opened { "memory" } else { "closed" },
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
    journal_impl::seed_defaults(st.db.as_ref().unwrap())?;
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
) -> Result<TradesPage, CmdError> {
    let st = state.lock().unwrap();
    let db = st.db.as_ref().ok_or_else(kernel_not_open)?;
    let node: aria_query_engine::FilterNode = serde_json::from_value(filter)
        .map_err(|e| CmdError::new(aria_query_engine::QueryError::INVALID_QUERY, format!("فیلتر نامعتبر: {e}")))?;
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

    /// ثبت پیوست: نوشتن بایت‌ها در پوشه داده، هش blake3، ردیف پیوست و
    /// پیوند با معامله از مسیر دستور دامنه (حسابرسی و رویداد).
    /// همان فایل (هش یکسان) در پیوست‌های متعدد یکتاست — ردیف موجود
    /// استفاده مجدد می‌شود و فقط پیوند تازه ساخته می‌شود.
    pub fn ingest_attachment(
        db: &Database,
        data_dir: &std::path::Path,
        trade_id: &str,
        file_name: &str,
        mime_type: Option<String>,
        data: Vec<u8>,
        link_kind: &str,
    ) -> PlugResult<AttachmentDto> {
        if data.is_empty() {
            return Err(CmdError::new(0, "محتوای پیوست خالی است"));
        }
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
        let meta = match attachments::find_by_hash(db, &hash)
            .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))?
        {
            Some(existing) => {
                // فایل تکراری: ردیف موجود استفاده مجدد می‌شود؛ نسخه فیزیکی
                // تازه‌نوشته‌شده حذف و مسیر ردیف موجود حفظ می‌گردد.
                let _ = std::fs::remove_file(&path);
                existing
            }
            None => attachments::register_attachment(
                db,
                &safe_name,
                &path,
                mime_type.as_deref(),
                None,
                None,
            )
            .map_err(|e| CmdError::new(i64::from(e.code()), e.to_string()))?,
        };
        execute_domain(
            db,
            aria_domain_engine::commands::command_type::LINK_ATTACHMENT_TO_TRADE,
            serde_json::json!({
                "attachment_id": meta.id,
                "trade_id": trade_id,
                "link_kind": link_kind,
            }),
        )?;
        Ok(AttachmentDto {
            id: meta.id,
            file_name: meta.file_name,
            size_bytes: meta.size_bytes,
            blake3_hash: meta.blake3_hash,
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
            ui_extensions,
            domain_execute,
            schema_list_fields,
            schema_field_options,
            schema_set_values,
            accounts_list,
            symbols_list,
            attachment_ingest
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
}
