import { useCallback, useEffect, useState } from "react";

export type Theme = "light" | "dark";

const STORAGE_KEY = "tahan.theme";

/** خواندن تم ذخیره‌شده؛ پیش‌فرض روشن. */
export function readStoredTheme(): Theme {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return v === "dark" ? "dark" : "light";
  } catch {
    return "light";
  }
}

/** اعمال تم روی ریشه سند (کلاس `.dark` متغیرهای CSS را عوض می‌کند). */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  root.classList.toggle("dark", theme === "dark");
}

/** هوک تم با ذخیره‌سازی پایدار. */
export function useTheme(): {
  theme: Theme;
  toggle: () => void;
} {
  const [theme, setTheme] = useState<Theme>(readStoredTheme);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const toggle = useCallback(() => {
    setTheme((prev) => {
      const next: Theme = prev === "dark" ? "light" : "dark";
      try {
        localStorage.setItem(STORAGE_KEY, next);
      } catch {
        // محیط بدون localStorage (بخشی از تست‌ها) — تم فقط برای نشست اعمال می‌شود
      }
      return next;
    });
  }, []);

  return { theme, toggle };
}
