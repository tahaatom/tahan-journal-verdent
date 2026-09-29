//! سازنده‌های خالص گزینه‌های ECharts برای ویجت‌های داشبورد (فاز ۱.۱۴).
//!
//! خالص بودن این توابع آزمون‌پذیری کامل خروجی نمودارها (برچسب فارسی،
//! ترتیب RTL، محورها) را بدون DOM ممکن می‌کند. خوانش راست‌به‌چپ:
//! محورهای دسته‌ای با inverse=true ساخته می‌شوند تا نخستین مقدار
//! (کهن‌ترین تاریخ/ساعت صفر/نخستین گروه) سمت راست قرار گیرد.

import type { EChartsCoreOption } from "echarts/core";
import type { EquityPoint, GroupStat, HeatCell } from "../kernel";

/** ترتیب هفته فارسی — شنبه نخست (۰=یکشنبه … ۶=شنبه در strftime). */
export const WEEKDAY_ORDER = [6, 0, 1, 2, 3, 4, 5] as const;

/** برچسب فارسی روزهای هفته بر مبنای خروجی strftime '%w'. */
export const WEEKDAY_FA: Record<number, string> = {
  0: "یکشنبه",
  1: "دوشنبه",
  2: "سه‌شنبه",
  3: "چهارشنبه",
  4: "پنجشنبه",
  5: "جمعه",
  6: "شنبه",
};

/** قالب عدد فارسی برای برچسب‌ها و راهنمای نمودار.
 *  قالب‌دهنده محلی ممکن است منفی یونیکد (U+2212) و نشانه‌های جهت برگرداند؛
 *  خروجی برای نمایش پایدار نرمال‌سازی می‌شود. */
export function fmtFa(n: number, maxDigits = 2): string {
  return n
    .toLocaleString("fa-IR", { maximumFractionDigits: maxDigits })
    .replace(/\u2212/g, "-")
    .replace(/[\u200E\u200F\u202A-\u202E]/g, "");
}

const FONT = "Vazirmatn, sans-serif";

/** گزینه‌های مشترک متن فارسی. */
function textStyle(): Record<string, unknown> {
  return { fontFamily: FONT };
}

/** منحنی سرمایه — خط PnL تجمعی روزانه با پیش‌فرض RTL. */
export function buildEquityOption(points: EquityPoint[]): EChartsCoreOption {
  return {
    textStyle: textStyle(),
    tooltip: {
      trigger: "axis",
      confine: true,
      valueFormatter: (v: unknown) => fmtFa(Number(v)),
    },
    grid: { top: 16, right: 16, bottom: 16, left: 16, containLabel: true },
    xAxis: {
      type: "category",
      data: points.map((p) => p.date),
      inverse: true,
      axisLabel: { fontFamily: FONT },
      axisTick: { alignWithLabel: true },
    },
    yAxis: {
      type: "value",
      axisLabel: { fontFamily: FONT, formatter: (v: number) => fmtFa(v) },
      splitLine: { lineStyle: { opacity: 0.3 } },
    },
    series: [
      {
        type: "line",
        name: "cumulative_pnl",
        data: points.map((p) => p.cumulative_pnl),
        showSymbol: false,
        smooth: true,
        areaStyle: { opacity: 0.15 },
      },
    ],
  };
}

/**
 * تفکیک عملکرد — میله افقی به ازای هر گروه؛ برچسب فارسی گروه روی محور
 * دسته‌ای و راهنمای شامل تعداد/نرخ برد/میانگین R.
 */
export function buildBreakdownOption(groups: GroupStat[]): EChartsCoreOption {
  return {
    textStyle: textStyle(),
    tooltip: {
      trigger: "axis",
      confine: true,
      axisPointer: { type: "shadow" },
      formatter: (params: unknown) => {
        const list = Array.isArray(params) ? params : [params];
        const first = list[0] as { dataIndex?: number } | undefined;
        const g = first?.dataIndex !== undefined ? groups[first.dataIndex] : undefined;
        if (!g) return "";
        const parts = [
          `${g.label}`,
          `${fmtFa(g.trades)} معامله`,
          g.win_rate === null ? null : `نرخ برد: ${fmtFa(g.win_rate, 1)}٪`,
          g.total_pnl === null ? null : `PnL: ${fmtFa(g.total_pnl)}`,
          g.avg_r === null ? null : `میانگین R: ${fmtFa(g.avg_r)}`,
        ].filter(Boolean);
        return parts.join("<br/>");
      },
    },
    grid: { top: 8, right: 24, bottom: 8, left: 8, containLabel: true },
    xAxis: {
      type: "value",
      axisLabel: { fontFamily: FONT, formatter: (v: number) => fmtFa(v) },
      splitLine: { lineStyle: { opacity: 0.3 } },
    },
    yAxis: {
      type: "category",
      data: groups.map((g) => g.label),
      inverse: true,
      axisLabel: { fontFamily: FONT },
    },
    series: [
      {
        type: "bar",
        data: groups.map((g) => g.total_pnl ?? 0),
        barMaxWidth: 26,
      },
    ],
  };
}

/**
 * نقشه حرارتی زمان — ماتریس روز هفته (شنبه نخست) × ساعت (صفر در راست)
 * با برچسب فارسی و رنگ‌بندی بر اساس تعداد معاملات.
 */
export function buildHeatmapOption(cells: HeatCell[]): EChartsCoreOption {
  const hours = Array.from({ length: 24 }, (_, h) => h);
  const yCats = WEEKDAY_ORDER.map((w) => WEEKDAY_FA[w]);
  const rowIndex = new Map<number, number>(WEEKDAY_ORDER.map((w, i) => [w as number, i]));
  const data = cells.map((c) => [c.hour, rowIndex.get(c.weekday) ?? 0, c.trades]);
  const maxTrades = Math.max(1, ...cells.map((c) => c.trades));
  return {
    textStyle: textStyle(),
    tooltip: {
      position: "top",
      confine: true,
      formatter: (p: unknown) => {
        const cell = p as { value?: [number, number, number] };
        const [h, r, n] = cell.value ?? [0, 0, 0];
        const day = yCats[r] ?? "";
        return `${day} — ${fmtFa(h)}:۰۰ — ${fmtFa(n)} معامله`;
      },
    },
    grid: { top: 8, right: 16, bottom: 48, left: 16, containLabel: true },
    xAxis: {
      type: "category",
      data: hours.map((h) => fmtFa(h, 0)),
      inverse: true,
      axisLabel: { fontFamily: FONT },
    },
    yAxis: {
      type: "category",
      data: yCats,
      axisLabel: { fontFamily: FONT },
    },
    visualMap: {
      min: 0,
      max: maxTrades,
      calculable: false,
      orient: "horizontal",
      left: "center",
      bottom: 0,
      inRange: {
        color: ["#eef2ff", "#818cf8", "#4338ca"],
      },
      textStyle: { fontFamily: FONT },
    },
    series: [
      {
        type: "heatmap",
        data,
        label: {
          show: true,
          fontFamily: FONT,
          formatter: (p: unknown) => {
            const v = (p as { value?: [number, number, number] }).value;
            return v ? fmtFa(v[2], 0) : "";
          },
        },
      },
    ],
  };
}
