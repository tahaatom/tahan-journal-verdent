//! صفحه ژورنال — تب فهرست معاملات (فاز ۱.۱۲) و فرم ثبت معامله (فاز ۱.۱۱).

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

type JournalTab = "list" | "register";

/** صفحه ژورنال — فهرست/جزئیات معاملات و ثبت معامله جدید. */
export function JournalPage() {
  const { t } = useTranslation();
  const bridge = useKernelBridge();
  const [tab, setTab] = useState<JournalTab>("list");
  /** پس از هر ثبت، فهرست دوباره خوانده می‌شود. */
  const [savedTick, setSavedTick] = useState(0);

  const onSaved = useCallback(() => {
    setSavedTick((n) => n + 1);
    setTab("list");
    // کش آمار داشبورد باطل می‌شود تا ویجت‌ها داده تازه بخوانند (فاز ۱.۱۴)
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
      </div>

      {tab === "list" ? (
        <TradeListPage refreshSignal={savedTick} />
      ) : (
        <>
          <p className="text-xs text-text-muted">{t("journal.hint")}</p>
          <TradeForm bridge={bridge} onSaved={onSaved} />
        </>
      )}
    </div>
  );
}
