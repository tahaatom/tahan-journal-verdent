import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TradeForm } from "./TradeForm";
import type { TradeFormBridge } from "./types";
import "../i18n";

function makeBridge(overrides: Partial<TradeFormBridge> = {}): TradeFormBridge {
  return {
    executeCommand: vi.fn(async (commandType) => {
      if (commandType === "domain.create_trade") {
        return [{ event_type: "domain.trade_created", payload: { trade_id: "t-77" } }];
      }
      return [];
    }),
    setCustomValues: vi.fn(async () => undefined),
    ingestAttachment: vi.fn(async () => undefined),
    listAccounts: vi.fn(async () => [{ id: "acc-1", name: "حساب پیش‌فرض", currency: "USD" }]),
    listSymbols: vi.fn(async () => [{ id: "sym-1", name: "XAU/USD" }]),
    listFields: vi.fn(async () => []),
    fieldOptions: vi.fn(async () => []),
    ...overrides,
  };
}

const sampleField = {
  id: "f-1",
  technical_key: "confidence",
  display_label: "اطمینان",
  storage_type: "integer",
  semantic_type: "number",
  unit: null,
  required: false,
  display_order: 1,
  form_group: "تحلیل",
  validation_rules: { min: 1, max: 10 },
};

const sampleSelectField = {
  id: "f-2",
  technical_key: "setup_quality",
  display_label: "کیفیت ستاپ",
  storage_type: "enum",
  semantic_type: "single_select",
  unit: null,
  required: false,
  display_order: 0,
  form_group: "تحلیل",
  validation_rules: {},
};

async function fillBase(user: ReturnType<typeof userEvent.setup>) {
  // گزینه‌های حساب/نماد پس از حل وعده پل بارگذاری می‌شوند
  await waitFor(() => {
    expect((screen.getByTestId("field-account") as HTMLSelectElement).options.length).toBeGreaterThan(1);
    expect((screen.getByTestId("field-symbol") as HTMLSelectElement).options.length).toBeGreaterThan(1);
  });
  await user.selectOptions(screen.getByTestId("field-account"), "acc-1");
  await user.selectOptions(screen.getByTestId("field-symbol"), "sym-1");
  await user.click(screen.getByTestId("direction-buy"));
}

beforeEach(() => {
  window.localStorage.clear();
});

describe("TradeForm — modes", () => {
  it("starts in fast mode with minimal fields only", async () => {
    render(<TradeForm bridge={makeBridge()} />);
    expect(screen.getByTestId("mode-fast")).toHaveAttribute("data-active", "true");
    expect(screen.queryByTestId("add-entry-leg")).not.toBeInTheDocument();
    expect(screen.queryByTestId("add-exit-leg")).not.toBeInTheDocument();
    expect(screen.queryByTestId("field-status")).not.toBeInTheDocument();
  });

  it("full mode exposes multiple legs, metadata, status and attachments", async () => {
    const user = userEvent.setup();
    render(<TradeForm bridge={makeBridge()} />);
    await user.click(screen.getByTestId("mode-full"));
    expect(screen.getByTestId("add-entry-leg")).toBeInTheDocument();
    expect(screen.getByTestId("add-exit-leg")).toBeInTheDocument();
    expect(screen.getByTestId("field-status")).toBeInTheDocument();
    expect(screen.getByTestId("field-strategy")).toBeInTheDocument();
    expect(screen.getByTestId("field-tags")).toBeInTheDocument();
    expect(screen.getByTestId("attachment-file")).toBeInTheDocument();
  });

  it("fast mode submits exactly one entry leg in under-15s flow", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    render(<TradeForm bridge={bridge} />);
    await fillBase(user);
    await user.type(screen.getByTestId("entry-leg-0-volume"), "1");
    await user.type(screen.getByTestId("entry-leg-0-price"), "100");
    await user.click(screen.getByTestId("trade-submit"));
    await waitFor(() => expect(screen.getByTestId("trade-saved")).toBeInTheDocument());
    const calls = (bridge.executeCommand as ReturnType<typeof vi.fn>).mock.calls;
    expect(calls).toHaveLength(2);
    expect(calls[0][0]).toBe("domain.create_trade");
    expect(calls[1][0]).toBe("domain.add_entry_leg");
  });
});

describe("TradeForm — validation", () => {
  it("shows Persian errors and does not call the bridge when empty", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    render(<TradeForm bridge={bridge} />);
    await user.click(screen.getByTestId("trade-submit"));
    expect(await screen.findByText(/حساب الزامی است/)).toBeInTheDocument();
    expect(bridge.executeCommand).not.toHaveBeenCalled();
  });

  it("rejects zero volume without submitting", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    render(<TradeForm bridge={bridge} />);
    await fillBase(user);
    await user.type(screen.getByTestId("entry-leg-0-volume"), "0");
    await user.click(screen.getByTestId("trade-submit"));
    expect(await screen.findByText(/حجم باید عدد مثبت باشد/)).toBeInTheDocument();
    expect(bridge.executeCommand).not.toHaveBeenCalled();
  });
});

describe("TradeForm — PlannedR preview", () => {
  it("shows live R when entry, SL and TP exist", async () => {
    const user = userEvent.setup();
    render(<TradeForm bridge={makeBridge()} />);
    await user.click(screen.getByTestId("direction-buy"));
    await user.type(screen.getByTestId("entry-leg-0-price"), "100");
    await user.type(screen.getByTestId("entry-leg-0-sl"), "95");
    await user.type(screen.getByTestId("entry-leg-0-tp"), "115");
    expect(screen.getAllByTestId("planned-r")[0]).toHaveTextContent("3.00R");
  });

  it("warns when SL is missing", async () => {
    const user = userEvent.setup();
    render(<TradeForm bridge={makeBridge()} />);
    await user.click(screen.getByTestId("direction-buy"));
    await user.type(screen.getByTestId("entry-leg-0-price"), "100");
    expect(screen.getAllByTestId("sl-warning").length).toBeGreaterThan(0);
  });
});

describe("TradeForm — multi legs and manual executions", () => {
  it("submits multiple entry and exit legs with executed prices", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    render(<TradeForm bridge={bridge} />);
    await user.click(screen.getByTestId("mode-full"));
    await fillBase(user);
    await user.type(screen.getByTestId("entry-leg-0-volume"), "1");
    await user.type(screen.getByTestId("entry-leg-0-price"), "100");
    await user.click(screen.getByTestId("add-entry-leg"));
    await user.type(screen.getByTestId("entry-leg-1-volume"), "0.5");
    await user.type(screen.getByTestId("entry-leg-1-price"), "101");
    await user.click(screen.getByTestId("add-exit-leg"));
    await user.type(screen.getByTestId("exit-leg-0-volume"), "1.5");
    await user.type(screen.getByTestId("exit-leg-0-price"), "110");
    await user.click(screen.getByTestId("trade-submit"));
    await waitFor(() => expect(screen.getByTestId("trade-saved")).toBeInTheDocument());
    const calls = (bridge.executeCommand as ReturnType<typeof vi.fn>).mock.calls;
    const types = calls.map((c) => c[0]);
    expect(types).toEqual([
      "domain.create_trade",
      "domain.add_entry_leg",
      "domain.add_entry_leg",
      "domain.add_exit_leg",
    ]);
    // پاهای اجراشده قیمت اجرا دارند — کرنل اجرای دستی خودکار می‌سازد
    expect(calls[1][1]["executed_price"]).toBe(100);
    expect(calls[3][1]["executed_price"]).toBe(110);
  });
});

describe("TradeForm — custom fields", () => {
  it("renders custom fields grouped by form_group respecting order, without code changes", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge({
      listFields: vi.fn(async () => [sampleField, sampleSelectField]),
      fieldOptions: vi.fn(async (fieldId: string) =>
        fieldId === "f-2"
          ? [
              { value: "good", label: "خوب" },
              { value: "bad", label: "بد" },
            ]
          : [],
      ),
    });
    render(<TradeForm bridge={bridge} />);
    await user.click(screen.getByTestId("mode-full"));
    const group = await screen.findByTestId("custom-group-تحلیل");
    expect(group).toBeInTheDocument();
    // ترتیب: f-2 با display_order=0 قبل از f-1
    const keys = group.querySelectorAll("[data-testid]");
    const testIds = Array.from(keys).map((k) => k.getAttribute("data-testid"));
    expect(testIds.indexOf("custom-setup_quality")).toBeLessThan(testIds.indexOf("custom-confidence"));
    await user.selectOptions(screen.getByTestId("custom-setup_quality"), "good");
    await user.type(screen.getByTestId("custom-confidence"), "7");
  });

  it("sends custom values after trade creation", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge({
      listFields: vi.fn(async () => [sampleField]),
    });
    render(<TradeForm bridge={bridge} />);
    await user.click(screen.getByTestId("mode-full"));
    await fillBase(user);
    await user.type(screen.getByTestId("entry-leg-0-volume"), "1");
    await user.type(screen.getByTestId("custom-confidence"), "8");
    await user.click(screen.getByTestId("trade-submit"));
    await waitFor(() => expect(screen.getByTestId("trade-saved")).toBeInTheDocument());
    expect(bridge.setCustomValues).toHaveBeenCalledWith("t-77", { "f-1": 8 });
  });
});

describe("TradeForm — draft autosave", () => {
  it("autosaves draft after typing and recovers it on remount", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    const { unmount } = render(<TradeForm bridge={bridge} />);
    await fillBase(user);
    await user.type(screen.getByTestId("entry-leg-0-volume"), "2");
    // انتظار تاخیر ذخیره خودکار
    await new Promise((r) => setTimeout(r, 600));
    expect(window.localStorage.getItem("tahan.tradeDraft.v1")).not.toBeNull();
    unmount();

    const bridge2 = makeBridge();
    render(<TradeForm bridge={bridge2} />);
    expect(await screen.findByTestId("draft-banner")).toBeInTheDocument();
    await user.click(screen.getByTestId("draft-recover"));
    expect((screen.getByTestId("field-account") as HTMLSelectElement).value).toBe("acc-1");
    expect((screen.getByTestId("entry-leg-0-volume") as HTMLInputElement).value).toBe("2");
  });

  it("clears the draft after successful submit", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    render(<TradeForm bridge={bridge} />);
    await fillBase(user);
    await user.type(screen.getByTestId("entry-leg-0-volume"), "1");
    await user.click(screen.getByTestId("trade-submit"));
    await waitFor(() => expect(screen.getByTestId("trade-saved")).toBeInTheDocument());
    await new Promise((r) => setTimeout(r, 600));
    expect(window.localStorage.getItem("tahan.tradeDraft.v1")).toBeNull();
  });

  it("resume flow does not duplicate the trade after partial failure", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge({
      listFields: vi.fn(async () => [sampleField]),
      setCustomValues: vi.fn(async () => {
        throw new Error("خطای کرنل");
      }),
    });
    const first = render(<TradeForm bridge={bridge} />);
    await user.click(screen.getByTestId("mode-full"));
    await fillBase(user);
    await user.type(screen.getByTestId("entry-leg-0-volume"), "1");
    await user.type(await screen.findByTestId("custom-confidence"), "8");
    await user.click(screen.getByTestId("trade-submit"));
    // شکست در گام فیلدهای سفارشی — پس از ساخت معامله و پای ورود
    await waitFor(() => expect(screen.getByTestId("submit-error")).toBeInTheDocument());
    expect(bridge.executeCommand).toHaveBeenCalledTimes(2);
    expect((bridge.setCustomValues as ReturnType<typeof vi.fn>).mock.calls[0][0]).toBe("t-77");
    await new Promise((r) => setTimeout(r, 600));
    expect(window.localStorage.getItem("tahan.tradeDraft.v1")).not.toBeNull();

    // تلاش مجدد: بدون ساخت دوباره ادامه می‌دهد
    first.unmount();
    const bridge2 = makeBridge();
    render(<TradeForm bridge={bridge2} />);
    await user.click(screen.getByTestId("draft-recover"));
    await user.click(screen.getByTestId("trade-submit"));
    await waitFor(() => expect(screen.getAllByTestId("trade-saved").length).toBeGreaterThan(0));
    const calls = (bridge2.executeCommand as ReturnType<typeof vi.fn>).mock.calls;
    expect(calls.every((c) => c[0] !== "domain.create_trade")).toBe(true);
    expect(calls.some((c) => c[0] === "domain.add_entry_leg")).toBe(true);
    expect(bridge2.setCustomValues).toHaveBeenCalledWith("t-77", { "f-1": 8 });
  }, 10000);
});
