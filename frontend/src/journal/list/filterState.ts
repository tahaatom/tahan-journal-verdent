//! وضعیت فیلتر فهرست معاملات و تبدیل خالص آن به JSON اعلانی `FilterNode`.
//!
//! همه اعتبارسنجی و ساخت شرط در سمت کرنل انجام می‌شود؛ این ماژول فقط
//! وضعیت UI را به شکل JSON موتور پرس‌وجو ترجمه می‌کند و مقادیر نامعتبر
//! (مثل عدد غیریکتفا برای فیلد عددی) را قبل از ارسال رد می‌کند.

import type { CustomFieldDefLike } from "../fieldTypes";

/** یک شرط فیلد سفارشی در پنل فیلتر. */
export interface CustomFilterRow {
  /** technical_key فیلد */
  fieldKey: string;
  op: "equals" | "not_equals" | "contains" | "min" | "max" | "exists";
  /** مقدار رشته‌ای فرم — برای exists بی‌معناست */
  value: string;
}

/** وضعیت پنل فیلتر. */
export interface FilterState {
  accountId: string;
  symbolId: string;
  direction: "" | "buy" | "sell";
  status: "" | "open" | "closed" | "cancelled";
  strategy: string;
  timeframe: string;
  session: string;
  tags: string;
  emotions: string;
  mistakes: string;
  entryFrom: string;
  entryTo: string;
  /** جست‌وجوی متنی روی note و tags */
  search: string;
  customRows: CustomFilterRow[];
  /** نحوه ترکیب شرط‌های ساده با شرط‌های فیلد سفارشی */
  combine: "and" | "or";
}

/** وضعیت خالی — بدون هیچ شرطی. */
export function emptyFilterState(): FilterState {
  return {
    accountId: "",
    symbolId: "",
    direction: "",
    status: "",
    strategy: "",
    timeframe: "",
    session: "",
    tags: "",
    emotions: "",
    mistakes: "",
    entryFrom: "",
    entryTo: "",
    search: "",
    customRows: [],
    combine: "and",
  };
}

/** آیا هیچ شرطی فعال است؟ */
export function isEmptyFilter(s: FilterState): boolean {
  return (
    !s.accountId &&
    !s.symbolId &&
    !s.direction &&
    !s.status &&
    !s.strategy &&
    !s.timeframe &&
    !s.session &&
    !s.tags &&
    !s.emotions &&
    !s.mistakes &&
    !s.entryFrom &&
    !s.entryTo &&
    !s.search &&
    s.customRows.length === 0
  );
}

/**
 * مقدار تایپ‌دار فیلد سفارشی (`CustomValue` کرنل) — null یعنی مقدار
 * برای نوع فیلد نامعتبر است و شرط باید رد شود.
 */
export function typedCustomValue(
  field: Pick<CustomFieldDefLike, "storage_type">,
  raw: string,
): Record<string, unknown> | null {
  const t = raw.trim();
  if (t === "") return null;
  switch (field.storage_type) {
    case "integer":
    case "rating": {
      const n = Number(t);
      return Number.isInteger(n) ? { kind: "integer", value: n } : null;
    }
    case "decimal": {
      const n = Number(t);
      return Number.isFinite(n) ? { kind: "decimal", value: n } : null;
    }
    case "boolean": {
      if (t === "true" || t === "بله") return { kind: "boolean", value: true };
      if (t === "false" || t === "خیر") return { kind: "boolean", value: false };
      return null;
    }
    default:
      return { kind: "text", value: t };
  }
}

/** خطاهای اعتبارسنجی وضعیت فیلتر — کلید = ایندکس ردیف سفارشی. */
export function validateFilterState(
  s: FilterState,
  fields: CustomFieldDefLike[],
): Record<string, string> {
  const errors: Record<string, string> = {};
  s.customRows.forEach((row, i) => {
    const field = fields.find((f) => f.technical_key === row.fieldKey);
    if (!field) {
      errors[String(i)] = "فیلد سفارشی ناشناخته";
      return;
    }
    if (row.op === "exists") return;
    if (typedCustomValue(field, row.value) === null) {
      errors[String(i)] = `مقدار نامعتبر برای «${field.display_label}»`;
    }
  });
  return errors;
}

/** شرط JSON یک ردیف فیلد سفارشی — null یعنی رد شرط. */
function customRowNode(
  row: CustomFilterRow,
  fields: CustomFieldDefLike[],
): Record<string, unknown> | null {
  const field = fields.find((f) => f.technical_key === row.fieldKey);
  if (!field) return null;
  if (row.op === "exists") {
    return { type: "custom", field_key: row.fieldKey, op: { op: "exists" } };
  }
  const value = typedCustomValue(field, row.value);
  if (value === null) return null;
  return {
    type: "custom",
    field_key: row.fieldKey,
    op: { op: row.op, value },
  };
}

/**
 * ساخت `FilterNode` JSON از وضعیت پنل.
 * بدون هیچ شرطی: `{"type":"all","children":[]}` (بدون قید).
 * شرط‌های ساده همیشه در یک `Simple` جمع می‌شوند و با شرط‌های سفارشی
 * بر اساس `combine` در All/Any ترکیب می‌شوند.
 */
export function buildFilterNode(
  s: FilterState,
  fields: CustomFieldDefLike[],
): Record<string, unknown> {
  const simple: Record<string, unknown> = {};
  if (s.accountId) simple["account_id"] = s.accountId;
  if (s.symbolId) simple["symbol_id"] = s.symbolId;
  if (s.direction) simple["direction"] = s.direction;
  if (s.status) simple["status"] = s.status;
  if (s.strategy.trim()) simple["strategy"] = s.strategy.trim();
  if (s.timeframe.trim()) simple["timeframe"] = s.timeframe.trim();
  if (s.session.trim()) simple["session"] = s.session.trim();
  if (s.tags.trim()) simple["tags"] = s.tags.trim();
  if (s.emotions.trim()) simple["emotions"] = s.emotions.trim();
  if (s.mistakes.trim()) simple["mistakes"] = s.mistakes.trim();
  if (s.entryFrom.trim()) simple["entry_from"] = s.entryFrom.trim();
  if (s.entryTo.trim()) simple["entry_to"] = s.entryTo.trim();
  if (s.search.trim()) simple["search"] = s.search.trim();

  const customNodes = s.customRows
    .map((r) => customRowNode(r, fields))
    .filter((n): n is Record<string, unknown> => n !== null);

  const hasSimple = Object.keys(simple).length > 0;
  if (!hasSimple && customNodes.length === 0) {
    return { type: "all", children: [] };
  }
  if (customNodes.length === 0) {
    return { type: "simple", ...simple };
  }
  const simpleNode: Record<string, unknown> = hasSimple
    ? { type: "simple", ...simple }
    : { type: "all", children: [] };
  const children = [simpleNode, ...customNodes];
  return s.combine === "or"
    ? { type: "any", children }
    : { type: "all", children };
}
