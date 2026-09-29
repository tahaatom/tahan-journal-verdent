//! صفحه ژورنال — میزبان فرم ثبت معامله با پیاده‌سازی واقعی پل کرنل.

import { useCallback, useMemo } from "react";
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
import type { TradeFormBridge } from "../journal/types";
import { TradeForm } from "../journal/TradeForm";

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

/**
 * صفحه ثبت معامله — فاز ۱.۱۱ فقط فرم ثبت؛ لیست و جزئیات در فاز ۱.۱۲.
 */
export function JournalPage() {
  const { t } = useTranslation();
  const bridge = useKernelBridge();

  const onSaved = useCallback(() => {
    // فاز ۱.۱۲: به‌روزرسانی لیست معاملات و آمار داشبورد
  }, []);

  return (
    <div data-testid="page-journal" className="flex flex-col gap-4">
      <p className="text-xs text-text-muted">{t("journal.hint")}</p>
      <TradeForm bridge={bridge} onSaved={onSaved} />
    </div>
  );
}
