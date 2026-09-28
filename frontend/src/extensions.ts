/**
 * رندرگر اعلانی افزونه‌ها — آینه TS نگهبان `render.rs` در کرنل.
 *
 * تضمین امنیتی نسخه ۱: هیچ افزونه‌ای نمی‌تواند کامپوننت React تزریق کند.
 * این رندرگر فقط انواع سفید-لیست را به اتم‌های UI ثابت خودش نگاشت می‌کند؛
 * هر نوع یا کلید ناشناخته‌ای به‌جای اجرا، به‌صورت پیام خطای اعلانی رندر می‌شود
 * (بدون هیچ مسیر اجرای کد).
 */

export type ExtensionPoint =
  | "dashboard_widget"
  | "report_page"
  | "command_menu"
  | "form_field"
  | "plugin_settings";

/** انواع مجاز هر نقطه — هم‌ارز `allowed_types` در render.rs. */
export const ALLOWED_TYPES: Record<ExtensionPoint, readonly string[]> = {
  dashboard_widget: [
    "stat",
    "table",
    "chart_line",
    "chart_bar",
    "text",
    "divider",
    "list",
  ],
  report_page: ["table", "chart_line", "chart_bar", "text"],
  command_menu: ["command"],
  form_field: ["text", "number", "select", "boolean", "date", "textarea"],
  plugin_settings: ["text", "number", "boolean"],
};

/** کلیدهای ممنوع — هم‌ارز FORBIDDEN_KEYS در render.rs (هر دو سبک نام‌گذاری). */
const FORBIDDEN_KEYS: readonly string[] = [
  "component",
  "jsx",
  "script",
  "handler",
  "on_click",
  "onClick",
  "eval",
  "innerHTML",
];

export interface DeclarativeExtension {
  plugin_id: string;
  kind: ExtensionPoint;
  id: string;
  title: string;
  display_order: number;
  schema: Record<string, unknown>;
}

/** بررسی اعلانی بودن اسکیما در سمت فرانت (دفاع در عمق؛ مرجع: کرنل). */
export function schemaViolation(
  point: ExtensionPoint,
  schema: Record<string, unknown>,
): string | null {
  const type = schema["type"];
  if (typeof type !== "string") {
    return "invalid_schema";
  }
  if (!ALLOWED_TYPES[point].includes(type)) {
    return "injection_denied";
  }
  const found = findForbidden(schema, 0);
  return found ? "injection_denied" : null;
}

function findForbidden(value: unknown, depth: number): boolean {
  if (depth > 8) return true; // عمق غیرعادی → رد محافظه‌کارانه
  if (Array.isArray(value)) {
    return value.some((v) => findForbidden(v, depth + 1));
  }
  if (value !== null && typeof value === "object") {
    return Object.entries(value as Record<string, unknown>).some(
      ([k, v]) =>
        FORBIDDEN_KEYS.includes(k) || findForbidden(v, depth + 1),
    );
  }
  return false;
}

export interface RenderOutcome {
  /** نوع رندرشده؛ برای نوع نامعتبر "denied" است. */
  kind: "stat" | "table" | "text" | "divider" | "list" | "command" | "denied";
  /** کلید ترجمه خطا وقتی kind === "denied". */
  violation?: string;
}

/** نگاشت اسکیما به نوع رندر امن — بدون هیچ مسیر اجرای کد. */
export function renderOutcome(
  point: ExtensionPoint,
  ext: DeclarativeExtension,
): RenderOutcome {
  const violation = schemaViolation(point, ext.schema);
  if (violation) {
    return { kind: "denied", violation };
  }
  const type = ext.schema["type"] as string;
  if (type === "stat" || type.startsWith("chart_")) return { kind: "stat" };
  if (type === "table") return { kind: "table" };
  if (type === "text") return { kind: "text" };
  if (type === "divider") return { kind: "divider" };
  if (type === "list") return { kind: "list" };
  if (type === "command") return { kind: "command" };
  return { kind: "denied", violation: "invalid_schema" };
}
