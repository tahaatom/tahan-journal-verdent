//! پل پنل بکاپ و بازیابی (فاز ۱.۱۶) — جداسازی IPC از کامپوننت برای آزمون‌پذیری.
//! همان الگوی `journal/import/bridge.ts`.

import { useMemo } from "react";
import {
  backupCreate,
  backupInspect,
  backupRestore,
  backupVerify,
  type BackupManifestInfo,
  type BackupReport,
  type RestoreReport,
} from "../kernel";

/** پل بکاپ/بازیابی — تزریق‌پذیر در آزمون‌ها. */
export interface BackupBridge {
  /** ساخت بکاپ رمزنگاری‌شده. */
  create(args: { password: string; includeSettings: boolean }): Promise<BackupReport>;
  /** فراداده بسته بدون گذرواژه — پس از انتخاب فایل. */
  inspect(args: { fileName: string; data: Uint8Array }): Promise<BackupManifestInfo>;
  /** بازیابی بسته از بایت‌های فایل انتخاب‌شده. */
  restore(args: { fileName: string; data: Uint8Array; password: string }): Promise<RestoreReport>;
  /** تأیید کامل صحت بسته (رمزگشایی + چک‌سام‌ها). */
  verify(args: { fileName: string; data: Uint8Array; password: string }): Promise<BackupManifestInfo>;
}

/** پیاده‌سازی پل روی دستورات IPC کرنل. */
export function useKernelBackupBridge(): BackupBridge {
  return useMemo(
    () => ({
      create: (args) => backupCreate(args),
      inspect: (args) => backupInspect(args),
      restore: (args) => backupRestore(args),
      verify: (args) => backupVerify(args),
    }),
    [],
  );
}
