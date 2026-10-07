import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { QueryClientProvider } from "@tanstack/react-query";
import type { PageId } from "./kernel";
import { isTauri, kernelOpen } from "./kernel";
import { queryClient } from "./queryClient";
import { CommandPalette } from "./components/CommandPalette";
import { DashboardPage } from "./pages/DashboardPage";
import { JournalPage } from "./pages/JournalPage";
import { BackupPanel } from "./backup/BackupPanel";
import { EmptyState } from "./components/States";
import { useTheme } from "./theme";

const navItems = [
  { key: "nav.dashboard", id: "dashboard" },
  { key: "nav.journal", id: "journal" },
  { key: "nav.tradeList", id: "trades" },
  { key: "nav.fieldManager", id: "fields" },
  { key: "nav.pluginHealth", id: "plugins" },
  { key: "nav.backup", id: "backup" },
  { key: "nav.settings", id: "settings" },
] as const;

function PageStub({ message }: { message: string }) {
  return <EmptyState message={message} />;
}

/** عنوان صفحه برای رندر سربرگ. */
function usePageTitle(): Record<PageId, string> {
  const { t } = useTranslation();
  return useMemo(
    () => ({
      dashboard: t("page.dashboard"),
      journal: t("page.journal"),
      trades: t("page.trades"),
      fields: t("page.fields"),
      plugins: t("page.plugins"),
      backup: t("page.backup"),
      settings: t("page.settings"),
    }),
    [t],
  );
}

/**
 * پوسته برنامه — تماماً فارسی و راست‌به‌چپ با ویژگی‌های منطقی Tailwind.
 */
export default function App() {
  const { t } = useTranslation();
  const { theme, toggle } = useTheme();
  const [page, setPage] = useState<PageId>("dashboard");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const titles = usePageTitle();

  // راه‌اندازی هسته هنگام اجرای پوسته — در مرورگر/تست بی‌اثر است
  useEffect(() => {
    if (isTauri()) {
      kernelOpen().catch(() => {
        // خطای راه‌اندازی در فاز ۱.۱۰ صرفاً ثبت می‌شود؛ پوسته پایدار می‌ماند
      });
    }
  }, []);

  // میان‌بر سراسری Ctrl+K برای پالت فرمان
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <QueryClientProvider client={queryClient}>
      <div dir="rtl" className="flex h-full bg-surface text-text-main">
        <aside
          aria-label="sidebar"
          data-testid="sidebar"
          className="w-56 shrink-0 border-s border-border bg-surface-alt p-4"
        >
          <h1 className="mb-1 text-lg font-bold">{t("app.title")}</h1>
          <p className="mb-6 text-xs text-text-muted">{t("app.subtitle")}</p>
          <nav className="flex flex-col gap-1">
            {navItems.map((item) => (
              <button
                key={item.id}
                type="button"
                data-testid={`nav-${item.id}`}
                data-active={page === item.id}
                onClick={() => setPage(item.id as PageId)}
                className={`rounded px-3 py-2 text-start text-sm hover:bg-accent-soft ${
                  page === item.id ? "bg-accent-soft font-bold text-accent" : ""
                }`}
              >
                {t(item.key)}
              </button>
            ))}
          </nav>
          <button
            type="button"
            data-testid="theme-toggle"
            onClick={toggle}
            className="mt-6 w-full rounded border border-border px-3 py-2 text-xs hover:bg-accent-soft"
          >
            {theme === "dark" ? t("theme.light") : t("theme.dark")}
          </button>
        </aside>
        <main data-testid="main-area" className="flex-1 overflow-auto p-6">
          <header className="mb-4 flex items-center justify-between">
            <h2 data-testid="page-title" className="text-xl font-bold">
              {titles[page]}
            </h2>
            <button
              type="button"
              data-testid="palette-open"
              onClick={() => setPaletteOpen(true)}
              className="rounded border border-border px-3 py-1.5 text-xs text-text-muted hover:bg-accent-soft"
            >
              {t("palette.open")}
            </button>
          </header>
          {page === "dashboard" && <DashboardPage />}
          {page === "journal" && <JournalPage />}
          {page === "trades" && <PageStub message={t("placeholder.trades")} />}
          {page === "fields" && <PageStub message={t("placeholder.fields")} />}
          {page === "plugins" && <PageStub message={t("placeholder.plugins")} />}
          {page === "backup" && <BackupPanel />}
          {page === "settings" && (
            <div data-testid="page-settings" className="flex flex-col gap-3 text-sm">
              <p>{t("theme.light")} / {t("theme.dark")}</p>
              <button
                type="button"
                data-testid="settings-theme-toggle"
                onClick={toggle}
                className="w-fit rounded border border-border px-4 py-2 hover:bg-accent-soft"
              >
                {t("palette.toggleTheme")}
              </button>
            </div>
          )}
        </main>
        <CommandPalette
          open={paletteOpen}
          onClose={() => setPaletteOpen(false)}
          onNavigate={setPage}
          onToggleTheme={toggle}
        />
      </div>
    </QueryClientProvider>
  );
}
