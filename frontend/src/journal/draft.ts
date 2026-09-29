//! ذخیره خودکار و بازیابی پیش‌نویس فرم معامله در localStorage.
//!
//! بایت‌های پیوست‌ها سریال‌izable نیستند؛ فراداده آن‌ها (نام، نوع، پیوند)
//! ذخیره می‌شود و پس از بازیابی کاربر باید فایل را دوباره انتخاب کند.

import { emptyEntryLeg, type PendingAttachment, type TradeFormState } from "./types";

const DRAFT_STORAGE_KEY = "tahan.tradeDraft.v1";

/** نسخه ساختار پیش‌نویس — تغییر ناسازگار یعنی کلید جدید. */
export const DRAFT_VERSION = 1;

interface StoredDraft {
  version: number;
  savedAt: string;
  state: TradeFormState;
}

function sanitizeAttachments(list: PendingAttachment[]): PendingAttachment[] {
  return list.map((a) => ({ ...a, data: null }));
}

/** ذخیره وضعیت فرم — بایت‌های پیوست حذف می‌شوند. */
export function saveDraft(state: TradeFormState): void {
  try {
    const draft: StoredDraft = {
      version: DRAFT_VERSION,
      savedAt: new Date().toISOString(),
      state: { ...state, attachments: sanitizeAttachments(state.attachments) },
    };
    window.localStorage.setItem(DRAFT_STORAGE_KEY, JSON.stringify(draft));
  } catch {
    // حافظه در دسترس نیست — پیش‌نویس اختیاری است و هرگز نباید ثبت معامله را بشکند
  }
}

/**
 * بازیابی پیش‌نویس — فقط اگر نسخه همان نسخه فعلی باشد.
 * خروجی null یعنی پیش‌نویس معتبری وجود ندارد.
 */
export function loadDraft(): TradeFormState | null {
  try {
    const raw = window.localStorage.getItem(DRAFT_STORAGE_KEY);
    if (!raw) return null;
    const draft = JSON.parse(raw) as StoredDraft;
    if (draft.version !== DRAFT_VERSION || !draft.state || typeof draft.state !== "object") {
      return null;
    }
    // اطمینان از فیلدهای ساختاری جدید (تطابق رو به جلو)
    return {
      ...draft.state,
      entryLegs: draft.state.entryLegs?.length ? draft.state.entryLegs : [emptyEntryLeg()],
      exitLegs: draft.state.exitLegs ?? [],
      customValues: draft.state.customValues ?? {},
      attachments: sanitizeAttachments(draft.state.attachments ?? []),
    };
  } catch {
    return null;
  }
}

/** حذف پیش‌نویس — پس از ثبت موفق. */
export function clearDraft(): void {
  try {
    window.localStorage.removeItem(DRAFT_STORAGE_KEY);
  } catch {
    // بی‌اثر
  }
}

/** آیا پیش‌نویسی وجود دارد؟ (برای نمایش نوار بازیابی) */
export function hasDraft(): boolean {
  try {
    return window.localStorage.getItem(DRAFT_STORAGE_KEY) !== null;
  } catch {
    return false;
  }
}
