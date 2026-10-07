//! پنل بکاپ و بازیابی رمزنگاری‌شده (فاز ۱.۱۶) — ساخت بسته با گذرواژه،
//! انتخاب فایل `.tahanbak`، نمایش فراداده بدون گذرواژه و بازیابی اتمیک.

import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { parseCmdError, type BackupManifestInfo, type BackupReport, type RestoreReport } from "../kernel";
import { useKernelBackupBridge, type BackupBridge } from "./bridge";

interface BackupPanelProps {
  /** پل تزریقی (تست) — در نبود آن از پل کرنل استفاده می‌شود. */
  bridge?: BackupBridge;
}

/** قالب‌بندی بایت به‌صورت فارسی. */
function bytes(n: number): string {
  if (n < 1024) return n.toLocaleString("fa-IR") + " بایت";
  if (n < 1024 * 1024) return (n / 1024).toLocaleString("fa-IR", { maximumFractionDigits: 1 }) + " کیلوبایت";
  return (n / (1024 * 1024)).toLocaleString("fa-IR", { maximumFractionDigits: 2 }) + " مگابایت";
}

/** زمان UTC به نمایش فارسی ساده. */
function when(iso: string): string {
  return new Date(iso).toLocaleString("fa-IR");
}

/** پنل بکاپ و بازیابی. */
export function BackupPanel({ bridge: bridgeProp }: BackupPanelProps) {
  const { t } = useTranslation();
  const kernelBridge = useKernelBackupBridge();
  const bridge = bridgeProp ?? kernelBridge;

  // ===== ساخت بکاپ
  const [password, setPassword] = useState("");
  const [password2, setPassword2] = useState("");
  const [includeSettings, setIncludeSettings] = useState(true);
  const [creating, setCreating] = useState(false);
  const [backupReport, setBackupReport] = useState<BackupReport | null>(null);

  // ===== بازیابی
  const [file, setFile] = useState<File | null>(null);
  const [meta, setMeta] = useState<BackupManifestInfo | null>(null);
  const [restorePassword, setRestorePassword] = useState("");
  const [restoring, setRestoring] = useState(false);
  const [verifying, setVerifying] = useState(false);
  const [verified, setVerified] = useState<"ok" | null>(null);
  const [restoreReport, setRestoreReport] = useState<RestoreReport | null>(null);

  const [error, setError] = useState<string | null>(null);

  const onPickFile = useCallback(
    async (f: File | null) => {
      setMeta(null);
      setVerified(null);
      setRestoreReport(null);
      setError(null);
      setFile(f);
      if (!f) return;
      try {
        const data = new Uint8Array(await f.arrayBuffer());
        const m = await bridge.inspect({ fileName: f.name, data });
        setMeta(m);
      } catch (e) {
        setError(parseCmdError(e).message);
      }
    },
    [bridge],
  );

  const submitCreate = useCallback(async () => {
    if (creating) return;
    if (password.length < 8 || !/[a-zA-Z]/.test(password) || !/\d/.test(password)) {
      setError(t("backup.weakPasswordLocal"));
      return;
    }
    if (password !== password2) {
      setError(t("backup.passwordMismatch"));
      return;
    }
    setCreating(true);
    setError(null);
    setBackupReport(null);
    try {
      setBackupReport(await bridge.create({ password, includeSettings }));
      setPassword("");
      setPassword2("");
    } catch (e) {
      setError(parseCmdError(e).message);
    } finally {
      setCreating(false);
    }
  }, [bridge, creating, includeSettings, password, password2, t]);

  const submitVerify = useCallback(async () => {
    if (!file || verifying) return;
    setVerifying(true);
    setError(null);
    setVerified(null);
    try {
      const data = new Uint8Array(await file.arrayBuffer());
      await bridge.verify({ fileName: file.name, data, password: restorePassword });
      setVerified("ok");
    } catch (e) {
      setError(parseCmdError(e).message);
    } finally {
      setVerifying(false);
    }
  }, [bridge, file, restorePassword, verifying]);

  const submitRestore = useCallback(async () => {
    if (!file || restoring) return;
    if (!restorePassword) {
      setError(t("backup.passwordRequired"));
      return;
    }
    setRestoring(true);
    setError(null);
    setRestoreReport(null);
    try {
      const data = new Uint8Array(await file.arrayBuffer());
      const r = await bridge.restore({ fileName: file.name, data, password: restorePassword });
      setRestoreReport(r);
      setRestorePassword("");
    } catch (e) {
      setError(parseCmdError(e).message);
    } finally {
      setRestoring(false);
    }
  }, [bridge, file, restorePassword, restoring, t]);

  return (
    <div data-testid="backup-panel" dir="rtl" className="flex flex-col gap-6">
      <h2 className="text-base font-bold">{t("backup.title")}</h2>

      {/* ===== ساخت بکاپ ===== */}
      <section data-testid="backup-create" className="flex flex-col gap-3 rounded border border-border p-4">
        <h3 className="text-sm font-bold">{t("backup.createTitle")}</h3>
        <p className="text-xs text-text-muted">{t("backup.createHint")}</p>

        <div className="flex flex-wrap items-end gap-4">
          <label className="flex flex-col gap-1 text-sm">
            <span>{t("backup.password")}</span>
            <input
              data-testid="backup-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="new-password"
              className="rounded border border-border bg-surface px-3 py-1.5"
            />
          </label>
          <label className="flex flex-col gap-1 text-sm">
            <span>{t("backup.passwordRepeat")}</span>
            <input
              data-testid="backup-password2"
              type="password"
              value={password2}
              onChange={(e) => setPassword2(e.target.value)}
              autoComplete="new-password"
              className="rounded border border-border bg-surface px-3 py-1.5"
            />
          </label>
          <label className="flex items-center gap-2 text-sm">
            <input
              data-testid="backup-include-settings"
              type="checkbox"
              checked={includeSettings}
              onChange={(e) => setIncludeSettings(e.target.checked)}
            />
            <span>{t("backup.includeSettings")}</span>
          </label>
          <button
            type="button"
            data-testid="backup-create-submit"
            onClick={() => void submitCreate()}
            disabled={creating || !password}
            className="rounded bg-accent px-4 py-1.5 text-sm font-bold text-surface disabled:opacity-50"
          >
            {creating ? t("backup.creating") : t("backup.createSubmit")}
          </button>
        </div>

        {backupReport && (
          <div data-testid="backup-report" className="flex flex-col gap-2 rounded bg-green-50 p-3 dark:bg-green-950">
            <div className="flex items-center justify-between">
              <h4 className="text-sm font-bold">{t("backup.done")}</h4>
              <span data-testid="backup-report-size" className="text-xs text-text-muted">
                {bytes(backupReport.size_bytes)}
              </span>
            </div>
            <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm sm:grid-cols-3">
              <div className="flex justify-between gap-2">
                <dt className="text-text-muted">{t("backup.file")}:</dt>
                <dd data-testid="backup-report-path" className="truncate" title={backupReport.out_path}>
                  {backupReport.out_path.split(/[\\/]/).pop()}
                </dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-text-muted">{t("backup.when")}:</dt>
                <dd>{when(backupReport.created_at)}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-text-muted">{t("backup.entries")}:</dt>
                <dd data-testid="backup-report-entries">{backupReport.entries.toLocaleString("fa-IR")}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-text-muted">{t("backup.attachments")}:</dt>
                <dd>{backupReport.attachments_count.toLocaleString("fa-IR")}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-text-muted">{t("backup.database")}:</dt>
                <dd>{bytes(backupReport.database_bytes)}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-text-muted">{t("backup.settings")}:</dt>
                <dd>{backupReport.includes_settings ? t("backup.yes") : t("backup.no")}</dd>
              </div>
            </dl>
            <p className="text-xs text-text-muted">{t("backup.storeSafely")}</p>
          </div>
        )}
      </section>

      {/* ===== بازیابی ===== */}
      <section data-testid="backup-restore" className="flex flex-col gap-3 rounded border border-border p-4">
        <h3 className="text-sm font-bold">{t("backup.restoreTitle")}</h3>
        <p className="text-xs text-text-muted">{t("backup.restoreHint")}</p>

        <div className="flex flex-wrap items-end gap-4">
          <label className="flex flex-col gap-1 text-sm">
            <span>{t("backup.package")}</span>
            <input
              data-testid="backup-file"
              type="file"
              accept=".tahanbak"
              onChange={(e) => void onPickFile(e.target.files?.[0] ?? null)}
              className="rounded border border-border bg-surface px-3 py-1.5 text-sm"
            />
            <span className="text-xs text-text-muted">{file ? file.name : t("backup.noFile")}</span>
          </label>
        </div>

        {meta && (
          <div data-testid="backup-meta" className="flex flex-col gap-1 rounded bg-surface-alt p-3 text-sm">
            <div className="flex justify-between gap-2">
              <span className="text-text-muted">{t("backup.createdAt")}:</span>
              <span>{when(meta.created_at)}</span>
            </div>
            <div className="flex justify-between gap-2">
              <span className="text-text-muted">{t("backup.appVersion")}:</span>
              <span>{meta.app_version}</span>
            </div>
            <div className="flex justify-between gap-2">
              <span className="text-text-muted">{t("backup.schemaVersion")}:</span>
              <span>{meta.schema_version.toLocaleString("fa-IR")}</span>
            </div>
            <div className="flex justify-between gap-2">
              <span className="text-text-muted">{t("backup.encryption")}:</span>
              <span>{meta.encryption_summary}</span>
            </div>
            <div className="flex justify-between gap-2">
              <span className="text-text-muted">{t("backup.attachments")}:</span>
              <span>{meta.attachments_count.toLocaleString("fa-IR")}</span>
            </div>
            <div className="flex justify-between gap-2">
              <span className="text-text-muted">{t("backup.settings")}:</span>
              <span>{meta.includes_settings ? t("backup.yes") : t("backup.no")}</span>
            </div>
            <div className="flex justify-between gap-2">
              <span className="text-text-muted">{t("backup.files")}:</span>
              <span>{meta.entries.length.toLocaleString("fa-IR")}</span>
            </div>
          </div>
        )}

        <div className="flex flex-wrap items-end gap-4">
          <label className="flex flex-col gap-1 text-sm">
            <span>{t("backup.password")}</span>
            <input
              data-testid="backup-restore-password"
              type="password"
              value={restorePassword}
              onChange={(e) => setRestorePassword(e.target.value)}
              autoComplete="off"
              className="rounded border border-border bg-surface px-3 py-1.5"
            />
          </label>
          <button
            type="button"
            data-testid="backup-verify-submit"
            onClick={() => void submitVerify()}
            disabled={verifying || !file}
            className="rounded border border-border px-4 py-1.5 text-sm font-bold hover:bg-accent-soft disabled:opacity-50"
          >
            {verifying ? t("backup.verifying") : t("backup.verifySubmit")}
          </button>
          <button
            type="button"
            data-testid="backup-restore-submit"
            onClick={() => void submitRestore()}
            disabled={restoring || !file || !restorePassword}
            className="rounded bg-accent px-4 py-1.5 text-sm font-bold text-surface disabled:opacity-50"
          >
            {restoring ? t("backup.restoring") : t("backup.restoreSubmit")}
          </button>
        </div>

        {verified === "ok" && (
          <p
            data-testid="backup-verified"
            className="rounded bg-green-100 px-3 py-2 text-sm text-green-700 dark:bg-green-900 dark:text-green-200"
          >
            {t("backup.verifiedOk")}
          </p>
        )}

        {restoreReport && (
          <div data-testid="restore-report" className="flex flex-col gap-2 rounded bg-green-50 p-3 dark:bg-green-950">
            <h4 className="text-sm font-bold">{t("backup.restoreDone")}</h4>
            <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm sm:grid-cols-3">
              <div className="flex justify-between gap-2">
                <dt className="text-text-muted">{t("backup.entriesRestored")}:</dt>
                <dd data-testid="restore-report-entries">{restoreReport.entries_restored.toLocaleString("fa-IR")}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-text-muted">{t("backup.attachmentsRestored")}:</dt>
                <dd>{restoreReport.attachments_restored.toLocaleString("fa-IR")}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-text-muted">{t("backup.database")}:</dt>
                <dd>{bytes(restoreReport.database_bytes)}</dd>
              </div>
              <div className="flex justify-between gap-2">
                <dt className="text-text-muted">{t("backup.safetyCopy")}:</dt>
                <dd data-testid="restore-report-safety">
                  {restoreReport.safety_copy ? t("backup.safetyTaken") : t("backup.no")}
                </dd>
              </div>
            </dl>
            <p className="text-xs text-text-muted">{t("backup.restoreReloaded")}</p>
          </div>
        )}
      </section>

      {error && (
        <p
          role="alert"
          data-testid="backup-error"
          className="rounded bg-red-100 px-3 py-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200"
        >
          {t("backup.error")} — {error}
        </p>
      )}
    </div>
  );
}
