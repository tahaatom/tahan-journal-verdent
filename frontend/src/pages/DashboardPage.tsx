import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { coreStats, isTauri, uiExtensions, type CoreStats } from "../kernel";
import { DeclarativeWidget } from "../components/DeclarativeWidget";
import { EmptyState, ErrorState, LoadingSkeleton } from "../components/States";
import type { DeclarativeExtension, ExtensionPoint } from "../extensions";

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-border bg-surface-raised p-4">
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
  const [stats, setStats] = useState<CoreStats | null>(null);
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(true);
  const [widgets, setWidgets] = useState<DeclarativeExtension[]>([]);

  const load = useCallback(() => {
    setError(false);
    if (!isTauri()) {
      // مرورگر/تست: هسته در دسترس نیست — پوسته پایدار می‌ماند
      setLoading(false);
      setStats(null);
      return;
    }
    setLoading(true);
    coreStats()
      .then((s) => setStats(s))
      .catch(() => setError(true))
      .finally(() => setLoading(false));
    uiExtensions("dashboard_widget" satisfies ExtensionPoint)
      .then((exts) => setWidgets(exts as DeclarativeExtension[]))
      .catch(() => setWidgets([]));
  }, []);

  useEffect(load, [load]);

  if (error) return <ErrorState onRetry={load} />;

  const hasData = stats !== null && stats.total_trades > 0;

  return (
    <div className="flex flex-col gap-4" data-testid="page-dashboard">
      {loading && <LoadingSkeleton />}
      {!loading && !hasData && <EmptyState message={t("dashboard.empty")} />}
      {!loading && hasData && stats && (
        <>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
            <StatCard label={t("dashboard.totalTrades")} value={String(stats.total_trades)} />
            <StatCard label={t("dashboard.closedTrades")} value={String(stats.closed_trades)} />
            <StatCard
              label={t("dashboard.winRate")}
              value={stats.win_rate === null ? "—" : `${stats.win_rate.toFixed(1)}%`}
            />
            <StatCard
              label={t("dashboard.avgR")}
              value={stats.avg_r === null ? "—" : stats.avg_r.toFixed(2)}
            />
            <StatCard
              label={t("dashboard.totalPnl")}
              value={stats.total_pnl === null ? "—" : stats.total_pnl.toFixed(2)}
            />
            <StatCard
              label={t("dashboard.maxDrawdown")}
              value={stats.max_drawdown === null ? "—" : stats.max_drawdown.toFixed(2)}
            />
          </div>
          {widgets.length > 0 && (
            <section aria-label={t("extensions.dashboardWidgets")} className="flex flex-col gap-3">
              <h2 className="text-sm font-bold">{t("extensions.dashboardWidgets")}</h2>
              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                {widgets.map((w) => (
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
