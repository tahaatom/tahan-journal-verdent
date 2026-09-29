import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { PageId } from "../kernel";

export interface PaletteCommand {
  id: string;
  label: string;
  run: () => void;
}

/**
 * پالت فرمان با میان‌بر Ctrl+K — زیرساخت میان‌بر صفحه‌کلید نسخه ۱.
 * ناوبری با فلش‌ها، انتخاب با Enter، بستن با Escape.
 */
export function CommandPalette({
  open,
  onClose,
  onNavigate,
  onToggleTheme,
}: {
  open: boolean;
  onClose: () => void;
  onNavigate: (page: PageId) => void;
  onToggleTheme: () => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const commands: PaletteCommand[] = useMemo(
    () => [
      ...(
        [
          ["dashboard", "nav.dashboard"],
          ["journal", "nav.journal"],
          ["trades", "nav.tradeList"],
          ["fields", "nav.fieldManager"],
          ["plugins", "nav.pluginHealth"],
          ["backup", "nav.backup"],
          ["settings", "nav.settings"],
        ] as const
      ).map(([page, labelKey]) => ({
        id: `nav-${page}`,
        label: `${t("palette.openNav")} ${t(labelKey)}`,
        run: () => onNavigate(page as PageId),
      })),
      {
        id: "toggle-theme",
        label: t("palette.toggleTheme"),
        run: onToggleTheme,
      },
    ],
    [t, onNavigate, onToggleTheme],
  );

  const filtered = useMemo(() => {
    const q = query.trim();
    if (!q) return commands;
    return commands.filter((c) => c.label.includes(q));
  }, [commands, query]);

  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
      // تمرکز پس از رندر
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setActive((a) => Math.min(a + 1, Math.max(filtered.length - 1, 0)));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setActive((a) => Math.max(a - 1, 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        filtered[active]?.run();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, filtered, active, onClose]);

  if (!open) return null;

  return (
    <div
      data-testid="command-palette"
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 pt-24"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-lg border border-border bg-surface-raised p-2 shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          type="text"
          dir="rtl"
          aria-label={t("palette.placeholder")}
          data-testid="palette-input"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          placeholder={t("palette.placeholder")}
          className="w-full rounded bg-surface px-3 py-2 text-sm outline-none"
        />
        <ul className="mt-2 max-h-64 overflow-auto">
          {filtered.length === 0 && (
            <li className="px-3 py-2 text-sm text-text-muted" data-testid="palette-empty">
              {t("palette.noResults")}
            </li>
          )}
          {filtered.map((cmd, i) => (
            <li key={cmd.id}>
              <button
                type="button"
                data-testid={`palette-cmd-${cmd.id}`}
                data-active={i === active}
                onClick={() => {
                  cmd.run();
                  onClose();
                }}
                className={`w-full rounded px-3 py-2 text-start text-sm ${
                  i === active ? "bg-accent-soft text-accent" : "hover:bg-surface-alt"
                }`}
              >
                {cmd.label}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
