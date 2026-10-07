//! تست‌های پنل بکاپ و بازیابی — ساخت بسته، فراداده بدون گذرواژه،
//! تأیید صحت، بازیابی و نمایش خطاهای قراردادی.

import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { BackupPanel } from "./BackupPanel";
import type { BackupBridge } from "./bridge";
import type { BackupManifestInfo, BackupReport, RestoreReport } from "../kernel";
import "../i18n";

function report(overrides: Partial<BackupReport> = {}): BackupReport {
  return {
    out_path: "C:\\data\\backups\\tahan-20260930-120000.tahanbak",
    size_bytes: 2048,
    raw_bytes: 8192,
    payload_bytes: 1900,
    entries: 3,
    attachments_count: 1,
    database_bytes: 6000,
    includes_settings: true,
    raw_blake3: "a".repeat(64),
    integrity_hash: "b".repeat(64),
    created_at: "2026-09-30T12:00:00Z",
    format_version: 1,
    duration_ms: 120,
    ...overrides,
  };
}

function manifest(overrides: Partial<BackupManifestInfo> = {}): BackupManifestInfo {
  return {
    format_version: 1,
    created_at: "2026-09-30T12:00:00Z",
    app_version: "0.1.0",
    schema_version: 2,
    cipher: "aes-256-gcm",
    kdf: {
      algorithm: "argon2id",
      salt_hex: "ab".repeat(16),
      m_cost_kib: 65536,
      t_cost: 3,
      p_cost: 1,
      key_len: 32,
    },
    zstd_level: 3,
    raw_bytes: 8192,
    raw_blake3: "a".repeat(64),
    attachments_count: 1,
    includes_settings: true,
    entries: [{ path: "database/tahan.db", size: 6000, blake3: "c".repeat(64) }],
    encryption_summary: "aes-256-gcm + argon2id (m=65536 KiB, t=3, p=1) + zstd",
    ...overrides,
  };
}

function restoreReport(overrides: Partial<RestoreReport> = {}): RestoreReport {
  return {
    source_path: "inbox\\package.tahanbak",
    entries_restored: 3,
    attachments_restored: 1,
    database_bytes: 6000,
    schema_version: 2,
    created_at: "2026-09-30T12:00:00Z",
    safety_copy: "C:\\data\\backup-safety\\2026",
    integrity_hash: "d".repeat(64),
    duration_ms: 200,
    ...overrides,
  };
}

function bridge(overrides: Partial<BackupBridge> = {}): BackupBridge {
  return {
    create: vi.fn().mockResolvedValue(report()),
    inspect: vi.fn().mockResolvedValue(manifest()),
    restore: vi.fn().mockResolvedValue(restoreReport()),
    verify: vi.fn().mockResolvedValue(manifest()),
    ...overrides,
  };
}

function bakFile(name = "tahan-20260930.tahanbak", content = "TAHANBAK-bytes") {
  return new File([content], name, { type: "application/octet-stream" });
}

function pickFile(file: File) {
  fireEvent.change(screen.getByTestId("backup-file"), { target: { files: [file] } });
}

async function setup(b: BackupBridge = bridge()) {
  render(<BackupPanel bridge={b} />);
  await screen.findByTestId("backup-panel");
  return b;
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("BackupPanel", () => {
  it("renders Persian labels and disables both submits initially", async () => {
    await setup();
    expect(screen.getByText("بکاپ و بازیابی رمزنگاری‌شده")).toBeTruthy();
    expect((screen.getByTestId("backup-create-submit") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId("backup-restore-submit") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId("backup-include-settings") as HTMLInputElement).checked).toBe(true);
  });

  it("creates a backup and shows the Persian report", async () => {
    const user = userEvent.setup();
    const b = await setup();
    await user.type(screen.getByTestId("backup-password"), "Passw0rd!-Tahan");
    await user.type(screen.getByTestId("backup-password2"), "Passw0rd!-Tahan");
    await user.click(screen.getByTestId("backup-create-submit"));

    expect(b.create).toHaveBeenCalledTimes(1);
    const args = (b.create as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(args.password).toBe("Passw0rd!-Tahan");
    expect(args.includeSettings).toBe(true);

    await screen.findByTestId("backup-report");
    expect(screen.getByTestId("backup-report-entries").textContent).toContain("۳");
    expect(screen.getByTestId("backup-report-path").textContent).toContain("tahan-20260930-120000.tahanbak");
    expect(screen.getByText("بکاپ با موفقیت ساخته شد")).toBeTruthy();
    expect(screen.queryByTestId("backup-error")).toBeNull();
  });

  it("rejects mismatched passwords locally without calling the kernel", async () => {
    const user = userEvent.setup();
    const b = await setup();
    await user.type(screen.getByTestId("backup-password"), "Passw0rd!-Tahan");
    await user.type(screen.getByTestId("backup-password2"), "Other-Pass9");
    await user.click(screen.getByTestId("backup-create-submit"));
    expect((await screen.findByTestId("backup-error")).textContent).toContain("یکسان نیستند");
    expect(b.create).not.toHaveBeenCalled();
  });

  it("rejects weak passwords locally before hitting the kernel", async () => {
    const user = userEvent.setup();
    const b = await setup();
    await user.type(screen.getByTestId("backup-password"), "short");
    await user.type(screen.getByTestId("backup-password2"), "short");
    await user.click(screen.getByTestId("backup-create-submit"));
    expect((await screen.findByTestId("backup-error")).textContent).toContain("حداقل ۸ کاراکتر");
    expect(b.create).not.toHaveBeenCalled();
  });

  it("shows inspectable metadata without a password after choosing a file", async () => {
    const b = await setup();
    pickFile(bakFile());
    const meta = await screen.findByTestId("backup-meta");
    expect(b.inspect).toHaveBeenCalledTimes(1);
    expect(meta.textContent).toContain("argon2id");
    expect(meta.textContent).toContain("aes-256-gcm");
    expect(meta.textContent).toContain("0.1.0");
    expect((screen.getByTestId("backup-restore-submit") as HTMLButtonElement).disabled).toBe(true);
  });

  it("verifies the package with the password and shows success", async () => {
    const user = userEvent.setup();
    const b = await setup();
    pickFile(bakFile());
    await screen.findByTestId("backup-meta");
    await user.type(screen.getByTestId("backup-restore-password"), "Passw0rd!-Tahan");
    await user.click(screen.getByTestId("backup-verify-submit"));
    expect(await screen.findByTestId("backup-verified")).toBeTruthy();
    expect(b.verify).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId("backup-error")).toBeNull();
  });

  it("restores the package and shows the safety-copy report", async () => {
    const user = userEvent.setup();
    const b = await setup();
    pickFile(bakFile());
    await screen.findByTestId("backup-meta");
    await user.type(screen.getByTestId("backup-restore-password"), "Passw0rd!-Tahan");
    await user.click(screen.getByTestId("backup-restore-submit"));

    expect(b.restore).toHaveBeenCalledTimes(1);
    const args = (b.restore as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(args.fileName).toBe("tahan-20260930.tahanbak");
    expect(new TextDecoder().decode(args.data)).toContain("TAHANBAK");

    await screen.findByTestId("restore-report");
    expect(screen.getByTestId("restore-report-entries").textContent).toContain("۳");
    expect(screen.getByTestId("restore-report-safety").textContent).toContain("گرفته شد");
    expect(screen.getByText("بازیابی کامل شد")).toBeTruthy();
    expect((screen.getByTestId("backup-restore-password") as HTMLInputElement).value).toBe("");
  });

  it("shows the kernel error code message from a wrong password", async () => {
    const user = userEvent.setup();
    await setup(
      bridge({
        restore: vi.fn().mockRejectedValue({
          code: 1601,
          message: "رمزگشایی بسته ناموفق بود: گذرواژه نادرست است یا بسته دستکاری شده",
        }),
      }),
    );
    pickFile(bakFile());
    await screen.findByTestId("backup-meta");
    await user.type(screen.getByTestId("backup-restore-password"), "Wrong!Pass9");
    await user.click(screen.getByTestId("backup-restore-submit"));
    const err = await screen.findByTestId("backup-error");
    expect(err.textContent).toContain("گذرواژه نادرست");
    expect(screen.queryByTestId("restore-report")).toBeNull();
  });

  it("keeps restore disabled until a password is entered", async () => {
    const user = userEvent.setup();
    const b = await setup();
    pickFile(bakFile());
    await screen.findByTestId("backup-meta");
    const btn = screen.getByTestId("backup-restore-submit") as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    await user.type(screen.getByTestId("backup-restore-password"), "Passw0rd!-Tahan");
    expect(btn.disabled).toBe(false);
    expect(b.restore).not.toHaveBeenCalled();
  });

  it("shows inspect errors when the package cannot be read", async () => {
    await setup(bridge({ inspect: vi.fn().mockRejectedValue({ code: 1600, message: "فایل بسته بکاپ یافت نشد" }) }));
    pickFile(bakFile());
    const err = await screen.findByTestId("backup-error");
    expect(err.textContent).toContain("یافت نشد");
    expect(screen.queryByTestId("backup-meta")).toBeNull();
  });

  it("clears metadata and reports when a new file is chosen", async () => {
    await setup();
    pickFile(bakFile());
    await screen.findByTestId("backup-meta");
    pickFile(bakFile("other.tahanbak"));
    expect(screen.queryByTestId("backup-meta")).toBeNull();
    expect(screen.queryByTestId("restore-report")).toBeNull();
  });
});
