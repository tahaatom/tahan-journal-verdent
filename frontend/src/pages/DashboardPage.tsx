//! داشبورد — کارت‌های خلاصه + ویجت‌های منحنی سرمایه، تفکیک عملکرد
//! و نقشه حرارتی با TanStack Query (کش سمت کرنل) — فاز ۱.۱۴.

import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { coreStats, isTauri, uiExtensions, type CoreStats } from "../kernel";
import { DeclarativeWidget } from "../components/DeclarativeWidget";
import { EmptyState, ErrorState, LoadingSkeleton } from "../components/States";
import type { DeclarativeExtension, ExtensionPoint } from "../extensions";
import { BreakdownWidget, EquityWidget, HeatmapWidget } from "../dashboard/StatsWidgets";

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-border bg-surface-raised p-4" data-testid="stat-card">
      <p className="text-xs text-text-muted">{label}</p>
      <p className="mt-1 text-xl font-bold tabular-nums" dir="ltr">
        {value}
      </p>
    </div>
  );
}

/** داشبورد — آمار واقعی کرنل + ویجت‌های اعلانی افزونه‌ها. */
export function DashboardPage() {
  const { t } = useTranslation();
  const live = isTauri();

  const stats = useQuery({
    queryKey: ["stats", "core"],
    queryFn: coreStats,
    enabled: live,
  });
  const widgets = useQuery({
    queryKey: ["ui", "dashboard_widget"],
    queryFn: () => uiExtensions("dashboard_widget" satisfies ExtensionPoint),
    enabled: live,
  });

  if (stats.isError) return <ErrorState onRetry={() => stats.refetch()} />;

  const s: CoreStats | undefined = stats.data;
  const hasData = s !== undefined && s.total_trades > 0;

  return (
    <div className="flex flex-col gap-4" data-testid="page-dashboard">
      {stats.isPending && live && <LoadingSkeleton />}
      {!live && <EmptyState message={t("dashboard.empty")} />}
      {s !== undefined && !hasData && <EmptyState message={t("dashboard.empty")} />}
      {hasData && s && (
        <>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
            <StatCard label={t("dashboard.totalTrades")} value={String(s.total_trades)} />
            <StatCard label={t("dashboard.closedTrades")} value={String(s.closed_trades)} />
            <StatCard
              label={t("dashboard.winRate")}
              value={s.win_rate === null ? "—" : `${s.win_rate.toFixed(1)}%`}
            />
            <StatCard
              label={t("dashboard.avgR")}
              value={s.avg_r === null ? "—" : s.avg_r.toFixed(2)}
            />
            <StatCard
              label={t("dashboard.totalPnl")}
              value={s.total_pnl === null ? "—" : s.total_pnl.toFixed(2)}
            />
            <StatCard
              label={t("dashboard.maxDrawdown")}
              value={s.max_drawdown === null ? "—" : s.max_drawdown.toFixed(2)}
            />
          </div>

          {/* ویجت‌های آماری — هر یک داده خود را با کلید "stats" کش می‌کند */}
          <EquityWidget />
          <BreakdownWidget />
          <HeatmapWidget />

          {(widgets.data ?? []).length > 0 && (
            <section aria-label={t("extensions.dashboardWidgets")} className="flex flex-col gap-3">
              <h2 className="text-sm font-bold">{t("extensions.dashboardWidgets")}</h2>
              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                {(widgets.data as DeclarativeExtension[]).map((w) => (
                  <DeclarativeWidget key={w.id} point="dashboard_widget" ext={w} />
                ))}
              </div>
            </section>
          )}
        </>
      )}
    </div>
  );
}
