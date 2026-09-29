//! کلاینت تایپ‌دار IPC کرنل — تنها مسیر دسترسی فرانت‌اند به کرنل آریا.
//!
//! در پوسته Tauri از `invoke` امن استفاده می‌کند (دستورات allowlist در
//! `apps/tahan-desktop/src/main.rs`)؛ در محیط مرورگر (تست/توسعه UI) به‌صورت
//! خاموش کاهش می‌یابد تا پوسته بدون کرنل نیز پایدار بماند.

export type PageId =
  | "dashboard"
  | "journal"
  | "trades"
  | "fields"
  | "plugins"
  | "backup"
  | "settings";

/** تشخیص اجرا در پوسته Tauri (و نه مرورگر/تست). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** خطای تایپ‌دار پل IPC — مطابق `CmdError` سمت کرنل (main.rs). */
export interface CmdError {
  /** کد قراردادی کرنل (۱۱۰۰+ ذخیره‌سازی، ۱۷۰۰+ پرس‌وجو…)؛ ۰ = خطای محلی پل/نامشخص */
  code: number;
  message: string;
}

/** نرمال‌سازی هر رد‌شدنی به خطای تایپ‌دار — هرگز رشته خام به UI نمی‌رسد. */
export function parseCmdError(raw: unknown): CmdError {
  if (raw && typeof raw === "object" && "code" in raw && "message" in raw) {
    const e = raw as { code?: unknown; message?: unknown };
    if (typeof e.code === "number" && typeof e.message === "string") {
      return { code: e.code, message: e.message };
    }
  }
  if (raw instanceof Error) return { code: 0, message: raw.message };
  return { code: 0, message: String(raw) };
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    return Promise.reject(parseCmdError("kernel-unavailable"));
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args).catch((e) => Promise.reject(parseCmdError(e)));
}

export interface CoreStats {
  total_trades: number;
  closed_trades: number;
  wins: number;
  losses: number;
  breakevens: number;
  win_rate: number | null;
  avg_r: number | null;
  total_pnl: number | null;
  max_drawdown: number | null;
}

export interface KernelStatus {
  opened: boolean;
  mode: string;
}

export interface AppInfo {
  app_name: string;
  kernel: string;
  ui_decl_version: number;
}

/** باز کردن هسته (در حافظه، فاز ۱.۱۰). */
export function kernelOpen(): Promise<KernelStatus> {
  return invoke<KernelStatus>("kernel_open");
}

export function kernelStatus(): Promise<KernelStatus> {
  return invoke<KernelStatus>("kernel_status");
}

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

/** آمار مرکبی با فیلتر خالی (کل معاملات). */
export function coreStats(): Promise<CoreStats> {
  return invoke<CoreStats>("core_stats", {
    filter: { type: "all", children: [] },
  });
}

/** افزونه‌های UI اعلانی یک نقطه — فقط اسکیمای اعلانی. */
export function uiExtensions(kind: string): Promise<unknown[]> {
  return invoke<unknown[]>("ui_extensions", { kind });
}

// ===================== فاز ۱.۱۱ — ماژول ژورنال =====================

/** خلاصه رویداد دامنه — مطابق `EventDto` سمت پل. */
export interface DomainEvent {
  event_id: string;
  event_type: string;
  payload: Record<string, unknown>;
}

/** حساب معاملاتی برای انتخابگر فرم. */
export interface AccountDto {
  id: string;
  name: string;
  currency: string;
}

/** نماد معاملاتی برای انتخابگر فرم. */
export interface SymbolDto {
  id: string;
  name: string;
}

/** پیوست ثبت‌شده و پیوند‌یافته با معامله. */
export interface AttachmentDto {
  id: string;
  file_name: string;
  size_bytes: number;
  blake3_hash: string;
}

/**
 * اجرای دستور دامنه (مثل domain.create_trade) به‌صورت اتمیک.
 * issuer همیشه "ui" است — فرانت‌اند هیچ مسیر دیگری به دامنه ندارد.
 */
export function domainExecute(
  commandType: string,
  payload: Record<string, unknown>,
): Promise<DomainEvent[]> {
  return invoke<DomainEvent[]>("domain_execute", { commandType, payload });
}

/** تعریف فیلد سفارشی برای رندر داینامیک فرم (فیلدهای فعال). */
export interface CustomFieldDef {
  id: string;
  technical_key: string;
  display_label: string;
  description: string | null;
  storage_type: string;
  semantic_type: string;
  unit: string | null;
  default_value: unknown;
  required: boolean;
  active: boolean;
  display_order: number;
  form_group: string | null;
  validation_rules: {
    min?: number | null;
    max?: number | null;
    min_length?: number | null;
    max_length?: number | null;
  };
}

/** گزینه فعال فیلد انتخابی. */
export interface FieldOptionDto {
  id: string;
  field_id: string;
  value: string;
  label: string;
  sort_order: number;
}

export function schemaListFields(): Promise<CustomFieldDef[]> {
  return invoke<CustomFieldDef[]>("schema_list_fields");
}

export function schemaFieldOptions(fieldId: string): Promise<FieldOptionDto[]> {
  return invoke<FieldOptionDto[]>("schema_field_options", { fieldId });
}

export function schemaSetValues(
  tradeId: string,
  values: Record<string, unknown>,
): Promise<number> {
  return invoke<number>("schema_set_values", { tradeId, values });
}

export function accountsList(): Promise<AccountDto[]> {
  return invoke<AccountDto[]>("accounts_list");
}

export function symbolsList(): Promise<SymbolDto[]> {
  return invoke<SymbolDto[]>("symbols_list");
}

/** ثبت پیوست و پیوند آن با معامله (لینک: before_trade/after_trade/chart/news/other). */
export function attachmentIngest(args: {
  tradeId: string;
  fileName: string;
  mimeType: string | null;
  data: Uint8Array;
  linkKind: string;
}): Promise<AttachmentDto> {
  return invoke<AttachmentDto>("attachment_ingest", {
    tradeId: args.tradeId,
    fileName: args.fileName,
    mimeType: args.mimeType,
    data: Array.from(args.data),
    linkKind: args.linkKind,
  });
}

// ===================== فاز ۱.۱۲ — فهرست و جزئیات =====================

/** سطر خلاصه فهرست معاملات — مطابق `TradeListRow` سمت کرنل. */
export interface TradeListRow {
  id: string;
  account_id: string;
  symbol_id: string;
  direction: string;
  status: string;
  strategy: string | null;
  timeframe: string | null;
  session: string | null;
  entry_time: string | null;
  exit_time: string | null;
  realized_pnl: number | null;
  realized_r: number | null;
}

/** صفحه‌بندی سمت کرنل. */
export interface PagedTrades {
  items: TradeListRow[];
  total: number;
  page: number;
  page_size: number;
}

/** کلیدهای مجاز مرتب‌سازی — هم‌تراز whitelist کرنل. */
export const SORT_KEYS = ["entry_time", "exit_time", "realized_pnl", "realized_r"] as const;
export type SortKey = (typeof SORT_KEYS)[number];

export function queryTrades(args: {
  filter: unknown;
  page: number;
  pageSize: number;
  sortKey?: SortKey | null;
  sortDesc?: boolean;
}): Promise<PagedTrades> {
  return invoke<PagedTrades>("query_trades", {
    filter: args.filter,
    page: args.page,
    pageSize: args.pageSize,
    sortKey: args.sortKey ?? null,
    sortDesc: args.sortDesc ?? true,
  });
}

/** پای ورود در پنل جزئیات. */
export interface EntryLegInfo {
  id: string;
  trade_id: string;
  planned_price: number | null;
  executed_price: number | null;
  volume: number;
  stop_loss: number | null;
  take_profit: number | null;
  entry_time: string | null;
  note: string | null;
  created_at: string;
  updated_at: string;
}

/** پای خروج در پنل جزئیات. */
export interface ExitLegInfo {
  id: string;
  trade_id: string;
  exit_reason: string | null;
  executed_price: number | null;
  volume: number;
  exit_time: string | null;
  note: string | null;
  created_at: string;
  updated_at: string;
}

/** اجرا — فیل بروکر یا دستی. */
export interface ExecutionInfo {
  id: string;
  trade_id: string | null;
  leg_id: string | null;
  leg_kind: string | null;
  kind: string;
  direction: string;
  price: number;
  volume: number;
  executed_at: string | null;
  assignment_status: string;
  created_at: string;
}

/** بازنویسی دستی. */
export interface OverrideInfo {
  id: string;
  entity_type: string;
  entity_id: string;
  field_name: string;
  previous_value: string | null;
  new_value: string | null;
  reason: string | null;
  source: string;
  priority: number;
  reversible: boolean;
  created_by: string;
  created_at: string;
  reverted_at: string | null;
}

/** پیوست یک معامله در پنل جزئیات. */
export interface TradeAttachmentInfo {
  id: string;
  file_name: string;
  mime_type: string | null;
  size_bytes: number;
  link_kind: string;
}

/** معامله کامل — canonical. */
export interface TradeFull {
  id: string;
  account_id: string;
  symbol_id: string;
  direction: string;
  status: string;
  strategy: string | null;
  timeframe: string | null;
  session: string | null;
  market_condition: string | null;
  entry_type: string | null;
  note: string | null;
  tags: string | null;
  emotions: string | null;
  mistakes: string | null;
  entry_time: string | null;
  exit_time: string | null;
  initial_stop_loss: number | null;
  take_profit: number | null;
  manual_risk: number | null;
  risk_calculation_status: string;
  planned_r: number | null;
  realized_pnl: number | null;
  realized_r: number | null;
  trade_r: number | null;
  commission: number;
  swap: number;
}

/** جزئیات کامل معامله — مطابق `TradeDetailsDto` سمت پل. */
export interface TradeDetails {
  trade: TradeFull;
  effective: Record<string, unknown> | null;
  entry_legs: EntryLegInfo[];
  exit_legs: ExitLegInfo[];
  executions: ExecutionInfo[];
  overrides: OverrideInfo[];
  attachments: TradeAttachmentInfo[];
  /** مقادیر فیلدهای سفارشی کلیدگذاری‌شده با technical_key */
  custom_values: Record<string, unknown>;
}

export function tradeDetails(tradeId: string): Promise<TradeDetails> {
  return invoke<TradeDetails>("trade_details", { tradeId });
}

export const KERNEL_READY_EVENT = "kernel://ready";
