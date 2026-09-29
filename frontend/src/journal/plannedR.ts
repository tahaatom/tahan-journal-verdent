//! محاسبه خالص R برنامه‌ریزی‌شده (PlannedR) برای پیش‌نمایش زنده فرم.
//!
//! R نسبت سود بالقوه به ریسک است: برای خرید (ریسک = ورود − SL) و
//! (پاداش = TP − ورود)؛ برای فروش برعکس. ورودی ناقص یعنی بدون پیش‌نمایش.

export interface PlannedRResult {
  /** R محاسبه‌شده — null یعنی قابل محاسبه نیست */
  value: number | null;
  /** هشدار SL ناموجود */
  slMissing: boolean;
  /** خطای معنایی: SL سمت اشتباه ورود یا TP نامعقول */
  invalid: string | null;
}

function num(v: string | undefined | null): number | null {
  if (v === undefined || v === null || v.trim() === "") return null;
  const n = Number(v);
  return Number.isFinite(n) && n > 0 ? n : null;
}

/**
 * محاسبه R برنامه‌ریزی‌شده از قیمت ورود، حد ضرر و حد سود.
 * مقدار منفی یا صفر R یعنی نسبت ریسک/پاداش معکوس است و خطا محسوب می‌شود.
 */
export function plannedR(
  entry: string | null,
  sl: string | null,
  tp: string | null,
  direction: "buy" | "sell" | "",
): PlannedRResult {
  const e = num(entry);
  const s = num(sl);
  const t = num(tp);
  const slMissing = s === null;
  if (e === null || s === null || t === null || direction === "") {
    return { value: null, slMissing, invalid: null };
  }
  const risk = direction === "buy" ? e - s : s - e;
  const reward = direction === "buy" ? t - e : e - t;
  if (risk <= 0) {
    return {
      value: null,
      slMissing,
      invalid:
        direction === "buy"
          ? "حد ضرر باید پایین‌تر از قیمت ورود باشد"
          : "حد ضرر باید بالاتر از قیمت ورود باشد",
    };
  }
  if (reward <= 0) {
    return {
      value: null,
      slMissing,
      invalid:
        direction === "buy"
          ? "حد سود باید بالاتر از قیمت ورود باشد"
          : "حد سود باید پایین‌تر از قیمت ورود باشد",
    };
  }
  return { value: Math.round((reward / risk) * 100) / 100, slMissing, invalid: null };
}
