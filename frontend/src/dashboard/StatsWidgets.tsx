//! ویجت‌های آماری داشبورد — منحنی سرمایه، تفکیک عملکرد و نقشه حرارتی.
//!
//! داده‌ها با TanStack Query کش می‌شوند (کلید "stats")؛ هیچ بازمحاسبه
//! داده خام در هر رندر انجام نمی‌شود — همه تجمیع‌ها سمت کرنل‌اند.

import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  ALL_FILTER,
  statsBreakdown,
  statsEquity,
  statsHeatmap,
  statFields,
  type Dimension,
} from "../kernel";
import { EChart } from "./EChart";
import {
  buildBreakdownOption,
  buildEquityOption,
  buildHeatmapOption,
} from "./charts";

const panelCls =
  "rounded-lg border border-border bg-surface-raised p-4 flex flex-col gap-2";

const FIXED_DIMS = ["symbol", "strategy", "timeframe", "session", "weekday"] as const;

/** منحنی سرمایه — PnL تجمعی روزانه معاملات بسته. */
export function EquityWidget() {
  const { t } = useTranslation();
  const { data: points } = useQuery({
    queryKey: ["stats", "equity"],
    queryFn: () => statsEquity(ALL_FILTER),
  });

  const option = useMemo(() => buildEquityOption(points ?? []), [points]);
  if (!points || points.length === 0) return null;
  return (
    <section data-testid="widget-equity" className={panelCls}>
      <h2 className="text-sm font-bold">{t("dashboard.equityCurve")}</h2>
      <EChart option={option} testid="chart-equity" />
    </section>
  );
}

/** تفکیک عملکرد — انتخاب بُعد ثابت یا فیلد سفارشی stat_enabled. */
export function BreakdownWidget() {
  const { t } = useTranslation();
  const [dim, setDim] = useState<Dimension>({ dim: "symbol" });

  const { data: fields } = useQuery({
    queryKey: ["stats", "statFields"],
    queryFn: () => statFields(),
  });
  const { data: groups } = useQuery({
    queryKey: ["stats", "breakdown", dim],
    queryFn: () => statsBreakdown(dim, ALL_FILTER),
  });

  const option = useMemo(() => buildBreakdownOption(groups ?? []), [groups]);
  if (!groups) return null;
  return (
    <section data-testid="widget-breakdown" className={panelCls}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-sm font-bold">{t("dashboard.performanceBreakdown")}</h2>
        <select
          data-testid="breakdown-dim"
          value={JSON.stringify(dim)}
          onChange={(e) => setDim(JSON.parse(e.target.value) as Dimension)}
          className="rounded border border-border bg-surface px-2 py-1 text-xs"
        >
          {FIXED_DIMS.map((d) => (
            <option key={d} value={JSON.stringify({ dim: d })}>
              {t(`dashboard.dim_${d}`)}
            </option>
          ))}
          {(fields ?? []).map((f) => (
            <option
              key={f.technical_key}
              value={JSON.stringify({ dim: "custom_field", field: f.technical_key })}
              data-testid={`breakdown-custom-${f.technical_key}`}
            >
              {f.display_label}
            </option>
          ))}
        </select>
      </div>
      {groups.length > 0 ? (
        <EChart option={option} testid="chart-breakdown" height={320} />
      ) : (
        <p className="text-xs text-text-muted">{t("dashboard.noData")}</p>
      )}
    </section>
  );
}

/** نقشه حرارتی زمان — روز هفته × ساعت بستن. */
export function HeatmapWidget() {
  const { t } = useTranslation();
  const { data: cells } = useQuery({
    queryKey: ["stats", "heatmap"],
    queryFn: () => statsHeatmap(ALL_FILTER),
  });

  const option = useMemo(() => buildHeatmapOption(cells ?? []), [cells]);
  if (!cells || cells.length === 0) return null;
  return (
    <section data-testid="widget-heatmap" className={panelCls}>
      <h2 className="text-sm font-bold">{t("dashboard.timeHeatmap")}</h2>
      <EChart option={option} testid="chart-heatmap" height={300} />
    </section>
  );
}
