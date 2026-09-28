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

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    return Promise.reject(new Error("kernel-unavailable"));
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
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

export const KERNEL_READY_EVENT = "kernel://ready";
