//! تست صفحه فهرست معاملات — پرس‌وجو، فیلتر، صفحه‌بندی، مرتب‌سازی،
//! جزئیات و اکشن‌ها (ویرایش/حذف با تأیید/بازنویسی/تخصیص) روی پل شبیه‌سازی‌شده.

import { describe, expect, it, vi, beforeEach, type Mock } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TradeListPage } from "./TradeListPage";
import type { TradeListBridge } from "./bridge";
import type { PagedTrades, TradeDetails } from "../../kernel";
import "../../i18n";

function makeRow(id: string, overrides: Partial<PagedTrades["items"][number]> = {}) {
  return {
    id,
    account_id: "acc-1",
    symbol_id: "sym-1",
    direction: "buy",
    status: "closed",
    strategy: "breakout",
    timeframe: null,
    session: null,
    entry_time: "2025-01-15T10:00:00Z",
    exit_time: null,
    realized_pnl: null,
    realized_r: null,
    ...overrides,
  };
}

function makeDetails(): TradeDetails {
  return {
    trade: {
      id: "t-1",
      account_id: "acc-1",
      symbol_id: "sym-1",
      direction: "buy",
      status: "closed",
      strategy: "breakout",
      timeframe: "H1",
      session: "london",
      market_condition: null,
      entry_type: null,
      note: "canonical-note",
      tags: null,
      emotions: null,
      mistakes: null,
      entry_time: "2025-01-15T10:00:00Z",
      exit_time: null,
      initial_stop_loss: 99,
      take_profit: 110,
      manual_risk: null,
      risk_calculation_status: "calculated",
      planned_r: 2,
      realized_pnl: 150,
      realized_r: 1.5,
      trade_r: 1.5,
      commission: 0,
      swap: 0,
    },
    // بازنویسی فعال: note از canonical تغییر کرده
    effective: { note: "effective-note" },
    entry_legs: [
      {
        id: "leg-1",
        trade_id: "t-1",
        planned_price: 100,
        executed_price: 101,
        volume: 1,
        stop_loss: 99,
        take_profit: null,
        entry_time: "2025-01-15T10:00:00Z",
        note: null,
        created_at: "2025-01-15T10:00:00Z",
        updated_at: "2025-01-15T10:00:00Z",
      },
    ],
    exit_legs: [],
    executions: [
      {
        id: "ex-1",
        trade_id: "t-1",
        leg_id: null,
        leg_kind: null,
        kind: "manual",
        direction: "buy",
        price: 101,
        volume: 1,
        executed_at: null,
        assignment_status: "needs_assignment",
        created_at: "2025-01-15T10:00:00Z",
      },
    ],
    overrides: [
      {
        id: "ov-1",
        entity_type: "journal_trade",
        entity_id: "t-1",
        field_name: "note",
        previous_value: "canonical-note",
        new_value: "effective-note",
        reason: "اشتباه تایپی",
        source: "manual",
        priority: 10,
        reversible: true,
        created_by: "user",
        created_at: "2025-01-16T09:00:00Z",
        reverted_at: null,
      },
    ],
    attachments: [
      {
        id: "att-1",
        file_name: "chart.png",
        mime_type: "image/png",
        size_bytes: 2048,
        link_kind: "chart",
      },
    ],
    custom_values: { confidence: 8 },
  };
}

type QueryArgs = Parameters<TradeListBridge["query"]>[0];

type MockBridge = TradeListBridge & {
  query: Mock<(args: QueryArgs) => Promise<PagedTrades>>;
  details: Mock<TradeListBridge["details"]>;
  executeCommand: Mock<TradeListBridge["executeCommand"]>;
};

function makeBridge(
  overrides: Partial<MockBridge> = {},
): MockBridge {
  const query = vi.fn<(args: QueryArgs) => Promise<PagedTrades>>(
    async (args) => ({
      items: [makeRow("t-1", { realized_pnl: 150, realized_r: 1.5 }), makeRow("t-2")],
      total: 60,
      page: args.page,
      page_size: args.pageSize,
    }),
  );
  const details = vi.fn<TradeListBridge["details"]>(async () => makeDetails());
  const executeCommand = vi.fn<TradeListBridge["executeCommand"]>(async () => []);
  return {
    query,
    details,
    executeCommand,
    ingestAttachment: vi.fn(async () => undefined),
    listAccounts: vi.fn(async () => [{ id: "acc-1", name: "حساب پیش‌فرض", currency: "USD" }]),
    listSymbols: vi.fn(async () => [{ id: "sym-1", name: "XAU/USD" }]),
    listFields: vi.fn(async () => []),
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

/**
 * رندر صفحه و صبر تا پایداری — وعده‌های متادیتا (حساب/نماد/فیلدها) حل
 * می‌شوند و رندر دوباره پیش از تعامل تمام می‌شود تا کلیک اول بین
 * pointerdown و pointerup گم نشود.
 */
async function setup(bridge: MockBridge, props: { refreshSignal?: number; onChanged?: () => void } = {}) {
  render(<TradeListPage bridge={bridge} {...props} />);
  await screen.findByTestId("trade-row-t-1");
  await new Promise((r) => setTimeout(r, 20));
}

describe("TradeListPage — query & pagination", () => {
  it("queries page 1 with unconstrained filter on mount", async () => {
    const bridge = makeBridge();
    await setup(bridge);
    expect(screen.getByTestId("trade-row-t-1")).toBeInTheDocument();
    expect(screen.getByTestId("trade-row-t-2")).toBeInTheDocument();
    const first = bridge.query.mock.calls[0][0];
    expect(first.page).toBe(1);
    expect(first.pageSize).toBe(25);
    expect(first.filter).toEqual({ type: "all", children: [] });
    expect(first.sortKey ?? null).toBeNull();
    expect(screen.getByTestId("page-info")).toHaveTextContent("صفحه 1 از 3");
  });

  it("requests next page on page-next click", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    await setup(bridge);
    await user.click(screen.getByTestId("page-next"));
    await waitFor(() => {
      expect(bridge.query).toHaveBeenLastCalledWith(
        expect.objectContaining({ page: 2 }),
      );
    });
  });

  it("sends filter with search only after apply", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    await setup(bridge);
    await user.type(screen.getByTestId("filter-search"), "طلایی");
    await user.click(screen.getByTestId("apply-filters"));
    await waitFor(() => {
      expect(bridge.query).toHaveBeenLastCalledWith(
        expect.objectContaining({
          filter: { type: "simple", search: "طلایی" },
          page: 1,
        }),
      );
    });
  });

  it("cycles sort on sortable header clicks and resets page", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    await setup(bridge);
    await user.click(screen.getByTestId("th-realized_pnl"));
    await waitFor(() => {
      expect(bridge.query).toHaveBeenLastCalledWith(
        expect.objectContaining({ sortKey: "realized_pnl", sortDesc: true }),
      );
    });
    await user.click(screen.getByTestId("th-realized_pnl"));
    await waitFor(() => {
      expect(bridge.query).toHaveBeenLastCalledWith(
        expect.objectContaining({ sortKey: "realized_pnl", sortDesc: false }),
      );
    });
    // ستون غیرقابل مرتب‌سازی پرس‌وجو نمی‌سازد
    await user.click(screen.getByTestId("th-symbol_id"));
    expect(bridge.query).toHaveBeenLastCalledWith(
      expect.not.objectContaining({ sortKey: "symbol_id" }),
    );
  });

  it("shows empty message when query returns no rows", async () => {
    const bridge = makeBridge({
      query: vi.fn(async (args) => ({
        items: [],
        total: 0,
        page: args.page,
        page_size: args.pageSize,
      })),
    });
    render(<TradeListPage bridge={bridge} />);
    expect(await screen.findByText("معامله‌ای یافت نشد.")).toBeInTheDocument();
  });

  it("surfaces query errors as an alert", async () => {
    const bridge = makeBridge({
      query: vi.fn(async () => {
        throw new Error("db locked");
      }),
    });
    render(<TradeListPage bridge={bridge} />);
    expect(await screen.findByRole("alert")).toHaveTextContent("db locked");
  });
});

describe("TradeListPage — details panel", () => {
  it("renders effective diff, legs, executions, overrides, attachments and custom values", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    await setup(bridge);
    await user.click(screen.getByTestId("open-trade-t-1"));

    const panel = await screen.findByTestId("trade-details-panel");
    expect(panel).toBeInTheDocument();
    // تفاوت موثر: canonical → effective
    const diff = screen.getByTestId("effective-diff");
    expect(diff).toHaveTextContent("note");
    expect(diff).toHaveTextContent("canonical-note");
    expect(diff).toHaveTextContent("effective-note");
    // پاها
    expect(screen.getByTestId("entry-leg-leg-1")).toBeInTheDocument();
    // اجرا با وضعیت نیاز به تخصیص + انتخابگر تخصیص
    expect(screen.getByTestId("execution-ex-1")).toHaveTextContent("اجرای دستی");
    expect(screen.getByTestId("trade-details-panel")).toHaveTextContent("در انتظار تخصیص");
    expect(screen.getByTestId("assign-select-ex-1")).toBeInTheDocument();
    // بازنویسی + دکمه برگرداندن
    expect(screen.getByTestId("revert-override-ov-1")).toBeInTheDocument();
    // پیوست
    expect(screen.getByTestId("attachment-chart.png")).toBeInTheDocument();
    // فیلد سفارشی
    expect(screen.getByTestId("details-custom-values")).toHaveTextContent("confidence");
  });

  it("saves edits through domain.update_trade", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    await setup(bridge);
    await user.click(screen.getByTestId("open-trade-t-1"));
    await screen.findByTestId("trade-details-panel");
    await user.clear(screen.getByTestId("edit-note"));
    await user.type(screen.getByTestId("edit-note"), "یادداشت اصلاح‌شده");
    await user.click(screen.getByTestId("save-edits"));
    await waitFor(() => {
      expect(bridge.executeCommand).toHaveBeenCalledWith("domain.update_trade", {
        trade_id: "t-1",
        note: "یادداشت اصلاح‌شده",
        status: "closed",
      });
    });
    expect(await screen.findByTestId("details-flash")).toHaveTextContent(
      "تغییرات ذخیره شد",
    );
  });

  it("deletes with confirmation and reason, closes panel and refetches", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    const onChanged = vi.fn();
    await setup(bridge, { onChanged });
    await user.click(screen.getByTestId("open-trade-t-1"));
    await screen.findByTestId("trade-details-panel");

    // حذف بدون تأیید انجام نمی‌شود
    await user.click(screen.getByTestId("delete-trade"));
    expect(screen.getByTestId("delete-confirm")).toBeInTheDocument();
    expect(bridge.executeCommand).not.toHaveBeenCalledWith(
      "domain.delete_trade",
      expect.anything(),
    );

    await user.type(screen.getByTestId("delete-reason"), "ثبت تکراری");
    await user.click(screen.getByTestId("confirm-delete"));
    await waitFor(() => {
      expect(bridge.executeCommand).toHaveBeenCalledWith("domain.delete_trade", {
        trade_id: "t-1",
        reason: "ثبت تکراری",
      });
    });
    // پنل بسته می‌شود، فهرست دوباره خوانده می‌شود و همگام‌سازی انجام می‌گیرد
    await waitFor(() => {
      expect(screen.queryByTestId("trade-details-panel")).not.toBeInTheDocument();
      expect(bridge.query.mock.calls.length).toBeGreaterThanOrEqual(2);
      expect(onChanged).toHaveBeenCalled();
    });
  });

  it("adds a manual override on journal_trade with audit reason", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    await setup(bridge);
    await user.click(screen.getByTestId("open-trade-t-1"));
    await screen.findByTestId("trade-details-panel");
    await user.selectOptions(screen.getByTestId("override-field"), "mistakes");
    await user.type(screen.getByTestId("override-value"), "ورود زودهنگام");
    await user.type(screen.getByTestId("override-reason"), "بازخورد منتور");
    await user.click(screen.getByTestId("add-override"));
    await waitFor(() => {
      expect(bridge.executeCommand).toHaveBeenCalledWith("domain.add_manual_override", {
        entity_type: "journal_trade",
        entity_id: "t-1",
        field_name: "mistakes",
        new_value: "ورود زودهنگام",
        reason: "بازخورد منتور",
        source: "manual",
        priority: 10,
        reversible: true,
        created_by: "user",
      });
    });
  });

  it("assigns a needs-assignment execution to an entry leg", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    await setup(bridge);
    await user.click(screen.getByTestId("open-trade-t-1"));
    await screen.findByTestId("trade-details-panel");
    await user.selectOptions(screen.getByTestId("assign-select-ex-1"), "leg-1");
    await waitFor(() => {
      expect(bridge.executeCommand).toHaveBeenCalledWith(
        "domain.assign_execution_to_leg",
        { execution_id: "ex-1", leg_id: "leg-1" },
      );
    });
  });

  it("reverts an active override", async () => {
    const user = userEvent.setup();
    const bridge = makeBridge();
    await setup(bridge);
    await user.click(screen.getByTestId("open-trade-t-1"));
    await screen.findByTestId("trade-details-panel");
    await user.click(screen.getByTestId("revert-override-ov-1"));
    await waitFor(() => {
      expect(bridge.executeCommand).toHaveBeenCalledWith(
        "domain.revert_manual_override",
        { override_id: "ov-1" },
      );
    });
  });
});
