//! پل پنل ایمپورت متاتریدر (فاز ۱.۱۵) — جداسازی IPC از کامپوننت برای آزمون‌پذیری.
//! همان الگوی `journal/list/bridge.ts`.

import { useMemo } from "react";
import { accountsList, mtImport, type AccountDto, type ImportReport } from "../../kernel";

/** پل ایمپورت — تزریق‌پذیر در آزمون‌ها. */
export interface MtImportBridge {
  /** ایمپورت فایل خروجی متاتریدر از طریق پلاگین رسمی mt.import. */
  import(args: { fileName: string; data: Uint8Array; accountId: string }): Promise<ImportReport>;
  /** فهرست حساب‌های معاملاتی برای انتخاب مقصد ایمپورت. */
  listAccounts(): Promise<AccountDto[]>;
}

/** پیاده‌سازی پل روی دستورات IPC کرنل. */
export function useKernelImportBridge(): MtImportBridge {
  return useMemo(
    () => ({
      import: (args) => mtImport(args),
      listAccounts: () => accountsList(),
    }),
    [],
  );
}
