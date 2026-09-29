//! کلاینت TanStack Query سراسری — کش ویجت‌های آماری با کلید "stats".
//!
//! پس از هر تغییر داده (ثبت/ویرایش/حذف معامله) کافی است
//! `invalidateStats()` صدا زده شود تا همه ویجت‌ها نوسازی شوند.

import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // آمار سمت کرنل است و با رویداد باطل می‌شود؛ بازخوانی اضافه نمی‌خواهد
      staleTime: 60_000,
      retry: 1,
    },
  },
});

/** باطل‌سازی کل فضای کش آمار داشبورد. */
export function invalidateStats(): void {
  void queryClient.invalidateQueries({ queryKey: ["stats"] });
}
