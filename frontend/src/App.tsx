import { useTranslation } from "react-i18next";

const navItems = [
  { key: "nav.dashboard", id: "dashboard" },
  { key: "nav.journal", id: "journal" },
  { key: "nav.tradeList", id: "trades" },
  { key: "nav.fieldManager", id: "fields" },
  { key: "nav.pluginHealth", id: "plugins" },
  { key: "nav.backup", id: "backup" },
  { key: "nav.settings", id: "settings" },
] as const;

export default function App() {
  const { t } = useTranslation();

  return (
    <div dir="rtl" className="flex h-full bg-surface text-text-main">
      <aside
        aria-label="sidebar"
        data-testid="sidebar"
        className="w-56 shrink-0 border-l border-border bg-surface-alt p-4"
      >
        <h1 className="mb-1 text-lg font-bold">{t("app.title")}</h1>
        <p className="mb-6 text-xs text-text-muted">{t("app.subtitle")}</p>
        <nav className="flex flex-col gap-1">
          {navItems.map((item) => (
            <button
              key={item.id}
              type="button"
              data-testid={`nav-${item.id}`}
              className="rounded px-3 py-2 text-start text-sm hover:bg-accent-soft"
            >
              {t(item.key)}
            </button>
          ))}
        </nav>
      </aside>
      <main data-testid="main-area" className="flex-1 p-6">
        <p className="text-sm text-text-muted">{t("placeholder.mainArea")}</p>
      </main>
    </div>
  );
}
