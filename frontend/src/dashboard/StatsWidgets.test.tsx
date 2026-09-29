//! تست ویجت‌های آماری داشبورد — بارگذاری داده با Query، انتخاب بُعد و
//! فیلد سفارشی، حالت بدون داده — با پل تزریقی روی دستورهای IPC.

import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClientProvider } from "@tanstack/react-query";
import { BreakdownWidget, EquityWidget, HeatmapWidget } from "./StatsWidgets";
import { queryClient } from "../queryClient";
import * as kernel from "../kernel";
import "../i18n";

vi.mock("../kernel", async (importOriginal) => {
  const base = await importOriginal<typeof import("../kernel")>();
  return {
    ...base,
    statsEquity: vi.fn(),
    statsBreakdown: vi.fn(),
    statsHeatmap: vi.fn(),
    statFields: vi.fn(),
    isTauri: () => true,
  };
});

const mock = vi.mocked(kernel);

const equity: kernel.EquityPoint[] = [
  { date: "2026-04-19", cumulative_pnl: 150 },
  { date: "2026-04-20", cumulative_pnl: 125 },
];
const groups: kernel.GroupStat[] = [
  { key: "breakout", label: "شکست", trades: 10, wins: 6, win_rate: 60, total_pnl: 250.5, avg_r: 0.8, avg_custom_value: null },
];
const cells: kernel.HeatCell[] = [{ weekday: 0, hour: 10, trades: 3, total_pnl: 100 }];
const fields: kernel.StatFieldInfo[] = [
  { id: "f1", technical_key: "confidence", display_label: "اعتماد به نفس", storage_type: "integer", semantic_type: "score" },
];

beforeEach(() => {
  vi.clearAllMocks();
  queryClient.clear();
  mock.statsEquity.mockResolvedValue(equity);
  mock.statsBreakdown.mockResolvedValue(groups);
  mock.statsHeatmap.mockResolvedValue(cells);
  mock.statFields.mockResolvedValue(fields);
});

function renderWith(page: React.ReactElement) {
  return render(<QueryClientProvider client={queryClient}>{page}</QueryClientProvider>);
}

describe("EquityWidget", () => {
  it("fetches equity points and mounts chart container", async () => {
    renderWith(<EquityWidget />);
    expect(await screen.findByTestId("widget-equity")).toBeInTheDocument();
    expect(screen.getByTestId("chart-equity")).toBeInTheDocument();
    expect(mock.statsEquity).toHaveBeenCalledWith(kernel.ALL_FILTER);
  });

  it("renders nothing when there is no data", async () => {
    mock.statsEquity.mockResolvedValue([]);
    renderWith(<EquityWidget />);
    await waitFor(() => {
      expect(screen.queryByTestId("widget-equity")).not.toBeInTheDocument();
    });
  });
});

describe("BreakdownWidget", () => {
  it("shows groups and fixed dimension options", async () => {
    renderWith(<BreakdownWidget />);
    expect(await screen.findByTestId("widget-breakdown")).toBeInTheDocument();
    expect(screen.getByTestId("chart-breakdown")).toBeInTheDocument();
    expect(screen.getByTestId("breakdown-dim")).toBeInTheDocument();
    expect(mock.statsBreakdown).toHaveBeenCalledWith({ dim: "symbol" }, kernel.ALL_FILTER);
  });

  it("switches dimension to custom field and refetches", async () => {
    const user = userEvent.setup();
    renderWith(<BreakdownWidget />);
    await screen.findByTestId("widget-breakdown");
    await user.selectOptions(
      screen.getByTestId("breakdown-dim"),
      screen.getByTestId("breakdown-custom-confidence"),
    );
    await waitFor(() => {
      expect(mock.statsBreakdown).toHaveBeenLastCalledWith(
        { dim: "custom_field", field: "confidence" },
        kernel.ALL_FILTER,
      );
    });
  });

  it("shows empty message when no groups exist", async () => {
    mock.statsBreakdown.mockResolvedValue([]);
    renderWith(<BreakdownWidget />);
    expect(await screen.findByText("داده‌ای برای این انتخاب وجود ندارد.")).toBeInTheDocument();
    expect(screen.queryByTestId("chart-breakdown")).not.toBeInTheDocument();
  });
});

describe("HeatmapWidget", () => {
  it("fetches heat cells and mounts chart container", async () => {
    renderWith(<HeatmapWidget />);
    expect(await screen.findByTestId("widget-heatmap")).toBeInTheDocument();
    expect(screen.getByTestId("chart-heatmap")).toBeInTheDocument();
    expect(mock.statsHeatmap).toHaveBeenCalledWith(kernel.ALL_FILTER);
  });

  it("renders nothing when there are no cells", async () => {
    mock.statsHeatmap.mockResolvedValue([]);
    renderWith(<HeatmapWidget />);
    await waitFor(() => {
      expect(screen.queryByTestId("widget-heatmap")).not.toBeInTheDocument();
    });
  });
});
