import { expect, it } from "vitest";
import i18n from "../i18n";

const keys = [
  "backup.title","backup.createTitle","backup.createHint",
  "backup.password","backup.passwordRepeat","backup.includeSettings",
  "backup.createSubmit","backup.creating","backup.done",
  "backup.file","backup.when","backup.entries","backup.attachments",
  "backup.database","backup.settings","backup.yes","backup.no","backup.storeSafely",
  "backup.restoreTitle","backup.restoreHint","backup.package","backup.noFile",
  "backup.createdAt","backup.appVersion","backup.schemaVersion","backup.encryption","backup.files",
  "backup.verifySubmit","backup.verifying","backup.verifiedOk",
  "backup.restoreSubmit","backup.restoring","backup.restoreDone",
  "backup.entriesRestored","backup.attachmentsRestored",
  "backup.safetyCopy","backup.safetyTaken","backup.restoreReloaded",
  "backup.passwordRequired","backup.passwordMismatch","backup.weakPasswordLocal","backup.error",
];

it("every key used by the backup panel resolves to Persian (not the key itself)", () => {
  const missing = keys.filter((k) => i18n.t(k) === k);
  expect(missing).toEqual([]);
});
