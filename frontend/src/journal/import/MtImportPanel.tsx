//! پنل ایمپورت متاتریدر (فاز ۱.۱۵) — انتخاب فایل CSV/HTML، حساب مقصد،
//! اجرای ایمپورت از طریق پلاگین رسمی mt.import و نمایش گزارش فارسی.

import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { parseCmdError, ALLOWED_IMPORT_EXTENSIONS, MAX_IMPORT_BYTES, type AccountDto, type ImportReport } from "../../kernel";
import { useKernelImportBridge, type MtImportBridge } from "./bridge";

interface MtImportPanelProps {
  /** پل تزریقی (تست) — در نبود آن از پل کرنل استفاده می‌شود. */
  bridge?: MtImportBridge;
  /** پس از ایمپورت موفق صدا زده می‌شود — برای نوسازی فهرست و داشبورد. */
  onImported?: (report: ImportReport) => void;
}

/** اعتبارسنجی سمت کلاینت — نام و حجم فایل؛ بایت‌ها به کرنل می‌روند. */
function validateFile(file: File): string | null {
  const ext = file.name.split(".").pop()?.toLowerCase() ?? "";
  if (!ALLOWED_IMPORT_EXTENSIONS.includes(ext as (typeof ALLOWED_IMPORT_EXTENSIONS)[number])) {
    return `typeNotAllowed:${ext}`;
  }
  if (file.size === 0) return "noData";
  if (file.size > MAX_IMPORT_BYTES) return "tooBig";
  return null;
}

/** پنل ایمپورت متاتریدر. */
export function MtImportPanel({ bridge: bridgeProp, onImported }: MtImportPanelProps) {
  const { t } = useTranslation();
  const kernelBridge = useKernelImportBridge();
  const bridge = bridgeProp ?? kernelBridge;

  const [accounts, setAccounts] = useState<AccountDto[]>([]);
  const [accountId, setAccountId] = useState<string>("");
  const [file, setFile] = useState<File | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let alive = true;
    bridge
      .listAccounts()
      .then((xs) => {
        if (!alive) return;
        setAccounts(xs);
        if (xs.length > 0) setAccountId((prev) => prev || xs[0].id);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [bridge]);

  const onPickFile = useCallback((f: File | null) => {
    setReport(null);
    setError(null);
    setFile(f);
  }, []);

  const submit = useCallback(async () => {
    if (!file || !accountId || busy) return;
    const problem = validateFile(file);
    if (problem) {
      if (problem.startsWith("typeNotAllowed:")) {
        setError(t("mtImport.typeNotAllowed", { ext: problem.slice("typeNotAllowed:".length) }));
      } else if (problem === "tooBig") {
        setError(t("mtImport.tooBig", { max: MAX_IMPORT_BYTES / (1024 * 1024) }));
      } else {
        setError(t("mtImport.noData"));
      }
      return;
    }
    setBusy(true);
    setError(null);
    setReport(null);
    try {
      const data = new Uint8Array(await file.arrayBuffer());
      const r = await bridge.import({ fileName: file.name, data, accountId });
      setReport(r);
      onImported?.(r);
    } catch (e) {
      setError(parseCmdError(e).message);
    } finally {
      setBusy(false);
    }
  }, [accountId, bridge, busy, file, onImported, t]);

  const num = (n: number) => n.toLocaleString("fa-IR");

  return (
    <div data-testid="mt-import-panel" dir="rtl" className="flex flex-col gap-4">
      <h2 className="text-base font-bold">{t("mtImport.title")}</h2>
      <p className="text-xs text-text-muted">{t("mtImport.hint")}</p>

      <div className="flex flex-wrap items-end gap-4">
        <label className="flex flex-col gap-1 text-sm">
          <span>{t("mtImport.account")}</span>
          <select
            data-testid="mt-import-account"
            value={accountId}
            onChange={(e) => setAccountId(e.target.value)}
            className="rounded border border-border bg-surface px-3 py-1.5"
          >
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col gap-1 text-sm">
          <span>{t("mtImport.file")}</span>
          <input
            ref={inputRef}
            data-testid="mt-import-file"
            type="file"
            accept=".csv,.tsv,.html,.htm"
            onChange={(e) => onPickFile(e.target.files?.[0] ?? null)}
            className="rounded border border-border bg-surface px-3 py-1.5 text-sm"
          />
          <span className="text-xs text-text-muted">
            {file ? file.name : t("mtImport.noFile")}
          </span>
        </label>

        <button
          type="button"
          data-testid="mt-import-submit"
          onClick={() => void submit()}
          disabled={busy || !file || !accountId}
          className="rounded bg-accent px-4 py-1.5 text-sm font-bold text-surface disabled:opacity-50"
        >
          {busy ? t("mtImport.importing") : t("mtImport.submit")}
        </button>
      </div>

      {error && (
        <p
          role="alert"
          data-testid="mt-import-error"
          className="rounded bg-red-100 px-3 py-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200"
        >
          {t("mtImport.error")} — {error}
        </p>
      )}

      {report && (
        <div data-testid="mt-import-report" className="flex flex-col gap-2 rounded border border-border p-3">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-bold">{t("mtImport.reportTitle")}</h3>
            {!report.file_duplicate && report.errors === 0 && (
              <span className="text-xs font-bold text-green-700 dark:text-green-400">
                {t("mtImport.done")}
              </span>
            )}
          </div>

          {report.file_duplicate && (
            <p
              data-testid="mt-import-duplicate"
              className="rounded bg-yellow-100 px-3 py-2 text-xs text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200"
            >
              {t("mtImport.fileDuplicate")}
            </p>
          )}

          <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm sm:grid-cols-3" dir="rtl">
            <div className="flex justify-between gap-2">
              <dt className="text-text-muted">{t("mtImport.totalRows")}:</dt>
              <dd data-testid="mt-import-total">{num(report.total_rows)}</dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt className="text-text-muted">{t("mtImport.skipped")}:</dt>
              <dd data-testid="mt-import-skipped">{num(report.skipped)}</dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt className="text-text-muted">{t("mtImport.imported")}:</dt>
              <dd data-testid="mt-import-imported">{num(report.imported)}</dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt className="text-text-muted">{t("mtImport.tradesCreated")}:</dt>
              <dd data-testid="mt-import-trades">{num(report.trades_created)}</dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt className="text-text-muted">{t("mtImport.duplicates")}:</dt>
              <dd data-testid="mt-import-duplicates">{num(report.duplicates)}</dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt className="text-text-muted">{t("mtImport.errors")}:</dt>
              <dd data-testid="mt-import-errors">{num(report.errors)}</dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt className="text-text-muted">{t("mtImport.needsAssignment")}:</dt>
              <dd data-testid="mt-import-needs">{num(report.needs_assignment)}</dd>
            </div>
          </dl>

          {report.warnings.length > 0 && (
            <div className="flex flex-col gap-1">
              <span className="text-xs font-bold text-text-muted">{t("mtImport.warnings")}</span>
              <ul className="list-inside list-disc text-xs text-text-muted">
                {report.warnings.map((w, i) => (
                  <li key={i}>{w}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
