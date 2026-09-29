//! تست سازنده‌های خالص نمودار — برچسب فارسی، ترتیب RTL و ساختار گزینه‌ها.

import { describe, expect, it } from "vitest";
import {
  buildBreakdownOption,
  buildEquityOption,
  buildHeatmapOption,
  WEEKDAY_FA,
  WEEKDAY_ORDER,
  fmtFa,
} from "./charts";
import type { EquityPoint, GroupStat, HeatCell } from "../kernel";

const equity: EquityPoint[] = [
  { date: "2026-04-19", cumulative_pnl: 150 },
  { date: "2026-04-20", cumulative_pnl: 125 },
  { date: "2026-04-21", cumulative_pnl: 310 },
];

const groups: GroupStat[] = [
  { key: "breakout", label: "شکست", trades: 10, wins: 6, win_rate: 60, total_pnl: 250.5, avg_r: 0.8, avg_custom_value: null },
  { key: "reversal", label: "بازگشتی", trades: 5, wins: 1, win_rate: 20, total_pnl: -40, avg_r: -0.2, avg_custom_value: null },
];

const cells: HeatCell[] = [
  { weekday: 0, hour: 10, trades: 3, total_pnl: 100 },
  { weekday: 6, hour: 22, trades: 1, total_pnl: -20 },
];

describe("fmtFa", () => {
  it("formats numbers with Persian digits", () => {
    expect(fmtFa(1234.5)).toBe("۱٬۲۳۴٫۵");
    expect(fmtFa(0)).toBe("۰");
    expect(fmtFa(-40)).toBe("-۴۰");
  });
});

describe("buildEquityOption", () => {
  it("mirrors date axis for RTL and carries cumulative series", () => {
    const opt = buildEquityOption(equity) as unknown as {
      xAxis: { inverse: boolean; data: string[] };
      series: { type: string; data: number[] }[];
    };
    expect(opt.xAxis.inverse).toBe(true);
    // کهن‌ترین تاریخ نخست (سمت راست با inverse)
    expect(opt.xAxis.data[0]).toBe("2026-04-19");
    expect(opt.series[0].data).toEqual([150, 125, 310]);
  });

  it("handles empty input", () => {
    const opt = buildEquityOption([]);
    expect((opt.xAxis as { data: unknown[] }).data).toEqual([]);
  });
});

describe("buildBreakdownOption", () => {
  it("uses Persian group labels on inverted category axis", () => {
    const opt = buildBreakdownOption(groups) as unknown as {
      yAxis: { inverse: boolean; data: string[] };
      series: { data: (number | null)[] }[];
      tooltip: { formatter: (p: unknown) => string };
    };
    expect(opt.yAxis.inverse).toBe(true);
    expect(opt.yAxis.data).toEqual(["شکست", "بازگشتی"]);
    expect(opt.series[0].data).toEqual([250.5, -40]);
    // راهنمای فارسی با نرخ برد
    const tip = opt.tooltip.formatter([{ dataIndex: 0 }]) as string;
    expect(tip).toContain("شکست");
    expect(tip).toContain("نرخ برد");
    expect(tip).toContain("٪");
  });
});

describe("buildHeatmapOption", () => {
  it("reorders weekdays Saturday-first and maps cells", () => {
    const opt = buildHeatmapOption(cells) as unknown as {
      yAxis: { data: string[] };
      xAxis: { inverse: boolean };
      series: { data: [number, number, number][]; label: { formatter: (p: unknown) => string } }[];
      visualMap: { max: number };
    };
    expect(opt.yAxis.data[0]).toBe(WEEKDAY_FA[6]); // شنبه نخست
    expect(opt.xAxis.inverse).toBe(true); // ساعت صفر سمت راست
    // یکشنبه (wd=0) به ردیف دوم و شنبه (wd=6) به ردیف نخست نگاشت می‌شود
    expect(opt.series[0].data).toContainEqual([10, 1, 3]);
    expect(opt.series[0].data).toContainEqual([22, 0, 1]);
    expect(opt.visualMap.max).toBe(3);
    // برچسب تعداد فارسی
    expect(opt.series[0].label.formatter({ value: [10, 1, 3] })).toBe("۳");
  });

  it("keeps visualMap max at 1 for empty input", () => {
    const opt = buildHeatmapOption([]);
    expect((opt.visualMap as { max: number }).max).toBe(1);
  });
});

describe("WEEKDAY_ORDER", () => {
  it("starts with Saturday and covers all seven days", () => {
    expect([...WEEKDAY_ORDER].sort()).toEqual([0, 1, 2, 3, 4, 5, 6]);
    expect(WEEKDAY_ORDER[0]).toBe(6);
  });
});
