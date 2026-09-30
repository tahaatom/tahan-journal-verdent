//! تست‌های پنل ایمپورت متاتریدر — انتخاب فایل، اعتبارسنجی، گزارش و خطاها.

import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MtImportPanel } from "./MtImportPanel";
import type { MtImportBridge } from "./bridge";
import type { ImportReport } from "../../kernel";
import "../../i18n";

function report(overrides: Partial<ImportReport> = {}): ImportReport {
  return {
    batch_id: "b-123",
    file_hash: "a".repeat(64),
    total_rows: 2,
    skipped: 0,
    imported: 2,
    duplicates: 0,
    errors: 0,
    needs_assignment: 0,
    trades_created: 1,
    file_duplicate: false,
    warnings: [],
    ...overrides,
  };
}

function bridge(overrides: Partial<MtImportBridge> = {}): MtImportBridge {
  return {
    import: vi.fn().mockResolvedValue(report()),
    listAccounts: vi
      .fn()
      .mockResolvedValue([{ id: "acc-1", name: "حساب متاتریدر", currency: "USD" }]),
    ...overrides,
  };
}

function csvFile(name = "deals.csv", content = "Time,Symbol\n2024.01.15,XAUUSD") {
  return new File([content], name, { type: "text/csv" });
}

/** ثبت فایل روی ورودی — بدون فیلتر `accept` کاربرتست، تا اعتبارسنجی خودمان آزموده شود. */
function pickFile(file: File) {
  fireEvent.change(screen.getByTestId("mt-import-file"), { target: { files: [file] } });
}

async function setup(b: MtImportBridge = bridge()) {
  render(<MtImportPanel bridge={b} />);
  await screen.findByTestId("mt-import-panel");
  // صبر برای بارگذاری ناهمگام حساب‌ها
  await new Promise((r) => setTimeout(r, 20));
  return b;
}

const submitBtn = () => screen.getByTestId("mt-import-submit") as HTMLButtonElement;

beforeEach(() => {
  vi.clearAllMocks();
});

describe("MtImportPanel", () => {
  it("renders account selector, Persian labels and disabled submit without a file", async () => {
    const b = await setup();
    expect(b.listAccounts).toHaveBeenCalledTimes(1);
    expect((screen.getByTestId("mt-import-account") as HTMLSelectElement).value).toBe("acc-1");
    expect(submitBtn().disabled).toBe(true);
    // کلیدها به متن فارسی ترجمه شده‌اند (نه خودِ کلید)
    expect(screen.getByText("ایمپورت فایل متاتریدر")).toBeTruthy();
    expect(screen.getByText("فایلی انتخاب نشده است")).toBeTruthy();
  });

  it("imports a chosen file and shows the Persian report", async () => {
    const user = userEvent.setup();
    const b = await setup();
    pickFile(csvFile());
    expect(submitBtn().disabled).toBe(false);
    await user.click(submitBtn());

    expect(b.import).toHaveBeenCalledTimes(1);
    const args = (b.import as ReturnType<typeof vi.fn>).mock.calls[0][0] as {
      fileName: string;
      data: Uint8Array;
      accountId: string;
    };
    expect(args.fileName).toBe("deals.csv");
    expect(args.accountId).toBe("acc-1");
    // بایت‌های واقعی فایل به کرنل می‌رود
    expect(new TextDecoder().decode(args.data)).toContain("XAUUSD");

    expect(await screen.findByTestId("mt-import-report")).toBeTruthy();
    expect(screen.getByTestId("mt-import-total").textContent).toContain("۲");
    expect(screen.getByTestId("mt-import-imported").textContent).toContain("۲");
    expect(screen.getByTestId("mt-import-trades").textContent).toContain("۱");
    expect(screen.getByText("ایمپورت کامل شد")).toBeTruthy();
    expect(screen.queryByTestId("mt-import-error")).toBeNull();
  });

  it("calls onImported with the report after success", async () => {
    const user = userEvent.setup();
    const onImported = vi.fn();
    render(<MtImportPanel bridge={bridge()} onImported={onImported} />);
    await screen.findByTestId("mt-import-panel");
    await new Promise((r) => setTimeout(r, 20));
    pickFile(csvFile());
    await user.click(submitBtn());
    await screen.findByTestId("mt-import-report");
    expect(onImported).toHaveBeenCalledWith(report());
  });

  it("rejects disallowed extensions without calling the kernel", async () => {
    const user = userEvent.setup();
    const b = await setup();
    pickFile(csvFile("report.pdf"));
    await user.click(submitBtn());
    expect(b.import).not.toHaveBeenCalled();
    const err = await screen.findByTestId("mt-import-error");
    expect(err.textContent).toContain("pdf");
  });

  it("rejects an empty file locally", async () => {
    const user = userEvent.setup();
    const b = await setup();
    pickFile(new File([], "empty.csv", { type: "text/csv" }));
    await user.click(submitBtn());
    expect(b.import).not.toHaveBeenCalled();
    expect((await screen.findByTestId("mt-import-error")).textContent).toContain("خالی");
  });

  it("shows kernel errors from the bridge", async () => {
    const user = userEvent.setup();
    await setup(
      bridge({
        import: vi.fn().mockRejectedValue({ code: 1506, message: "permission denied" }),
      }),
    );
    pickFile(csvFile());
    await user.click(submitBtn());
    const err = await screen.findByTestId("mt-import-error");
    expect(err.textContent).toContain("permission denied");
    expect(screen.queryByTestId("mt-import-report")).toBeNull();
  });

  it("re-enables submit after a failed import", async () => {
    const user = userEvent.setup();
    await setup(bridge({ import: vi.fn().mockRejectedValue(new Error("boom")) }));
    pickFile(csvFile());
    await user.click(submitBtn());
    await screen.findByTestId("mt-import-error");
    expect(submitBtn().disabled).toBe(false);
  });

  it("shows duplicate-file warning and all counts from the report", async () => {
    const user = userEvent.setup();
    await setup(
      bridge({
        import: vi.fn().mockResolvedValue(
          report({
            imported: 0,
            duplicates: 2,
            trades_created: 0,
            file_duplicate: true,
            warnings: [
              "این فایل قبلاً به‌طور کامل ایمپورت شده است؛ هیچ معامله جدیدی ساخته نشد.",
            ],
          }),
        ),
      }),
    );
    pickFile(csvFile());
    await user.click(submitBtn());
    await screen.findByTestId("mt-import-report");
    expect(screen.getByTestId("mt-import-duplicates").textContent).toContain("۲");
    expect(screen.getByTestId("mt-import-errors").textContent).toContain("۰");
    expect(screen.getByTestId("mt-import-needs").textContent).toContain("۰");
    expect(screen.getByTestId("mt-import-duplicate")).toBeTruthy();
    expect(screen.getByText(/هیچ معامله جدیدی ساخته نشد/)).toBeTruthy();
    // نشان «کامل شد» فقط برای ایمپورت بدون خطا
    expect(screen.queryByText("ایمپورت کامل شد")).toBeNull();
  });

  it("lists mapping warnings from the kernel report", async () => {
    const user = userEvent.setup();
    await setup(
      bridge({
        import: vi.fn().mockResolvedValue(
          report({
            errors: 1,
            imported: 1,
            warnings: ["ردیف 3: عدد نامعتبر: «abc»", "نماد «XAUUSD» ساخته شد."],
          }),
        ),
      }),
    );
    pickFile(csvFile());
    await user.click(submitBtn());
    await screen.findByTestId("mt-import-report");
    expect(screen.getByText("هشدارها")).toBeTruthy();
    expect(screen.getByText(/ردیف 3: عدد نامعتبر/)).toBeTruthy();
    expect(screen.getByTestId("mt-import-errors").textContent).toContain("۱");
  });

  it("shows the skipped count for non-trade rows", async () => {
    const user = userEvent.setup();
    await setup(bridge({ import: vi.fn().mockResolvedValue(report({ skipped: 2, total_rows: 4 })) }));
    pickFile(csvFile());
    await user.click(submitBtn());
    await screen.findByTestId("mt-import-report");
    expect(screen.getByTestId("mt-import-skipped").textContent).toContain("۲");
    expect(screen.getByTestId("mt-import-total").textContent).toContain("۴");
  });

  it("shows needs_assignment count for unassigned executions", async () => {
    const user = userEvent.setup();
    await setup(bridge({ import: vi.fn().mockResolvedValue(report({ needs_assignment: 1 })) }));
    pickFile(csvFile());
    await user.click(submitBtn());
    await screen.findByTestId("mt-import-report");
    expect(screen.getByTestId("mt-import-needs").textContent).toContain("۱");
  });

  it("clears a previous report when a new file is chosen", async () => {
    const user = userEvent.setup();
    await setup();
    pickFile(csvFile());
    await user.click(submitBtn());
    await screen.findByTestId("mt-import-report");
    pickFile(csvFile("other.csv"));
    expect(screen.queryByTestId("mt-import-report")).toBeNull();
  });
});
