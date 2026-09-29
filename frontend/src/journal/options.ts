//! گزینه‌های پیشنهادی چیپی فرم ثبت معامله — فارسی، برای حداقل‌سازی تایپ.
//!
//! مقادیر متادیتای آزاد (استراتژی، تایم‌فریم و…) از استفاده‌های پیشین
//! (`lastUsed`) و پیشنهادهای ثابت زیر پر می‌شوند؛ هیچ‌کدام الزامی نیستند.

/** پیشنهادهای ثابت برای فیلدهای متنی آزاد. */
export const SUGGESTIONS: Record<string, string[]> = {
  strategy: ["برک‌اوت", "روند", "برگشتی", "اسکالپ", "پوزیشن", "نوسان‌گیری"],
  timeframe: ["M1", "M5", "M15", "M30", "H1", "H4", "D1", "W1"],
  session: ["سیدنی", "توکیو", "لندن", "نیویورک", "لندن-نیویورک"],
  marketCondition: ["روند صعودی", "روند نزولی", "رنج", "نوسان بالا", "نوسان پایین"],
  entryType: ["مارکت", "لیمیت", "استاپ", "بریک‌اوت"],
  tags: ["A+", "A", "B", "آزمایشی", "میل به میانگین", "اخبار"],
  emotions: ["آرام", "اعتمادبه‌نفس", "طمع", "ترس", "بی‌حوصلگی", "انتقام‌جویی"],
  mistakes: ["ورود زودهنگام", "خروج زودهنگام", "حجم زیاد", "بدون SL", "میانگین‌گیری ضرر"],
};

/** کلیدهای فیلد که «آخرین استفاده» آن‌ها ذخیره و به‌عنوان چیپ پیشنهاد می‌شود. */
export const LAST_USED_KEYS = [
  "strategy",
  "timeframe",
  "session",
  "marketCondition",
  "entryType",
] as const;

export type LastUsedKey = (typeof LAST_USED_KEYS)[number];

const LAST_USED_STORAGE_KEY = "tahan.lastUsed.v1";

/** خواندن مقادیر آخرین استفاده — خرابی خواندن همیشه به {} می‌رسد. */
export function loadLastUsed(): Partial<Record<LastUsedKey, string>> {
  try {
    const raw = window.localStorage.getItem(LAST_USED_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const out: Partial<Record<LastUsedKey, string>> = {};
    for (const key of LAST_USED_KEYS) {
      const v = parsed[key];
      if (typeof v === "string" && v.length > 0) out[key] = v;
    }
    return out;
  } catch {
    return {};
  }
}

/** ثبت مقدار جدید یک فیلد به‌عنوان «آخرین استفاده». */
export function saveLastUsed(key: LastUsedKey, value: string): void {
  if (!value.trim()) return;
  try {
    const current = loadLastUsed();
    current[key] = value.trim();
    window.localStorage.setItem(LAST_USED_STORAGE_KEY, JSON.stringify(current));
  } catch {
    // حافظه در دسترس نیست (حالت حریم خصوصی) — چیپ‌ها فقط در همین نشست می‌مانند
  }
}

/** انواع پیوند پیوست — هم‌تراز `LINK_KINDS` کرنل. */
export const LINK_KINDS = [
  { value: "before_trade", label: "قبل معامله" },
  { value: "after_trade", label: "بعد معامله" },
  { value: "chart", label: "نمودار" },
  { value: "news", label: "اخبار" },
  { value: "other", label: "سایر" },
] as const;
