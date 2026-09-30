//! صفحه ژورنال — تب فهرست معاملات (فاز ۱.۱۲)، فرم ثبت معامله (فاز ۱.۱۱)
//! و پنل ایمپورت متاتریدر (فاز ۱.۱۵).

import { useCallback, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  attachmentIngest,
  domainExecute,
  schemaFieldOptions,
  schemaListFields,
  schemaSetValues,
  accountsList,
  symbolsList,
} from "../kernel";
import { invalidateStats } from "../queryClient";
import type { TradeFormBridge } from "../journal/types";
import { TradeForm } from "../journal/TradeForm";
import { TradeListPage } from "../journal/list/TradeListPage";
import { MtImportPanel } from "../journal/import/MtImportPanel";
import { useKernelImportBridge, type MtImportBridge } from "../journal/import/bridge";

/** پیاده‌سازی پل فرم روی دستورات IPC کرنل. */
function useKernelBridge(): TradeFormBridge {
  return useMemo(
    () => ({
      executeCommand: (commandType, payload) => domainExecute(commandType, payload),
      setCustomValues: (tradeId, values) =>
        schemaSetValues(tradeId, values).then(() => undefined),
      ingestAttachment: (args) => attachmentIngest(args).then(() => undefined),
      listAccounts: () => accountsList(),
      listSymbols: () => symbolsList(),
      listFields: () => schemaListFields(),
      fieldOptions: (fieldId) =>
        schemaFieldOptions(fieldId).then((opts) =>
          opts.map((o) => ({ value: o.value, label: o.label })),
        ),
    }),
    [],
  );
}

type JournalTab = "list" | "register" | "import";

/** صفحه ژورنال — فهرست/جزئیات معاملات، ثبت معامله جدید و ایمپورت متاتریدر. */
export function JournalPage() {
  const { t } = useTranslation();
  const bridge = useKernelBridge();
  const importBridge: MtImportBridge = useKernelImportBridge();
  const [tab, setTab] = useState<JournalTab>("list");
  /** پس از هر ثبت/ایمپورت، فهرست دوباره خوانده می‌شود. */
  const [savedTick, setSavedTick] = useState(0);

  const onSaved = useCallback(() => {
    setSavedTick((n) => n + 1);
    setTab("list");
    // کش آمار داشبورد باطل می‌شود تا ویجت‌ها داده تازه بخوانند (فاز ۱.۱۴)
    invalidateStats();
  }, []);

  /** پس از ایمپورت موفق — فهرست نوسازی می‌شود؛ کاربر در تب گزارش می‌ماند. */
  const onImported = useCallback(() => {
    setSavedTick((n) => n + 1);
    invalidateStats();
  }, []);

  const tabBtn = (active: boolean) =>
    `rounded px-4 py-1.5 text-sm font-bold ${
      active ? "bg-accent text-surface" : "border border-border hover:bg-accent-soft"
    }`;

  return (
    <div data-testid="page-journal" className="flex flex-col gap-4">
      <div role="tablist" className="flex gap-2" dir="rtl">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "list"}
          data-testid="journal-tab-list"
          onClick={() => setTab("list")}
          className={tabBtn(tab === "list")}
        >
          {t("list.tabList")}
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "register"}
          data-testid="journal-tab-register"
          onClick={() => setTab("register")}
          className={tabBtn(tab === "register")}
        >
          {t("list.tabRegister")}
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "import"}
          data-testid="journal-tab-import"
          onClick={() => setTab("import")}
          className={tabBtn(tab === "import")}
        >
          {t("list.tabImport")}
        </button>
      </div>

      {tab === "list" ? (
        <TradeListPage refreshSignal={savedTick} />
      ) : tab === "register" ? (
        <>
          <p className="text-xs text-text-muted">{t("journal.hint")}</p>
          <TradeForm bridge={bridge} onSaved={onSaved} />
        </>
      ) : (
        <MtImportPanel bridge={importBridge} onImported={onImported} />
      )}
    </div>
  );
}
