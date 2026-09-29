//! محاسبه خالص تفاوت داده موثر با canonical — برای نمایش «تغییر کرده‌ها»
//! در پنل جزئیات (موثر = canonical + بازنویسی‌های فعال).

import type { TradeDetails } from "../../kernel";

/** یک فیلد متفاوت بین canonical و موثر. */
export interface EffectiveDiffItem {
  field: string;
  canonical: unknown;
  effective: unknown;
}

const META_FIELDS = new Set([
  "created_at",
  "updated_at",
  "deleted_at",
  "id",
  "account_id",
  "symbol_id",
]);

function scalarEqual(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/**
 * فیلدهایی که مقدار موثر آن‌ها با canonical فرق دارد — بدون فیلدهای
 * متادیتایی. null یعنی داده موثر موجود نیست (بازنویسی فعال نداریم).
 */
export function effectiveDiff(details: TradeDetails): EffectiveDiffItem[] | null {
  if (!details.effective || typeof details.effective !== "object") return null;
  const eff = details.effective as Record<string, unknown>;
  const canonical = details.trade as unknown as Record<string, unknown>;
  const out: EffectiveDiffItem[] = [];
  for (const [field, value] of Object.entries(eff)) {
    if (META_FIELDS.has(field)) continue;
    if (!(field in canonical) || !scalarEqual(canonical[field], value)) {
      out.push({ field, canonical: canonical[field] ?? null, effective: value });
    }
  }
  return out;
}

/** نمایش مقدار برای UI — آبجکت‌ها JSON می‌شوند. */
export function displayValue(v: unknown): string {
  if (v === null || v === undefined) return "—";
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}
