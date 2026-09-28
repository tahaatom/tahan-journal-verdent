import { useTranslation } from "react-i18next";

/** حالت خطا با پیام فارسی و تلاش دوباره. */
export function ErrorState({ onRetry }: { onRetry?: () => void }) {
  const { t } = useTranslation();
  return (
    <div
      data-testid="error-state"
      role="alert"
      className="flex flex-col items-center gap-3 rounded-lg border border-danger bg-surface-raised p-8 text-center"
    >
      <span aria-hidden className="text-2xl">⚠️</span>
      <p className="text-sm font-bold">{t("state.errorTitle")}</p>
      {onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="rounded bg-accent-soft px-4 py-2 text-sm text-accent hover:opacity-80"
        >
          {t("state.retry")}
        </button>
      )}
    </div>
  );
}

/** حالت خالی با توضیح فارسی. */
export function EmptyState({ message }: { message: string }) {
  return (
    <div
      data-testid="empty-state"
      className="flex flex-col items-center gap-2 rounded-lg border border-border bg-surface-raised p-10 text-center"
    >
      <span aria-hidden className="text-2xl">📭</span>
      <p className="text-sm text-text-muted">{message}</p>
    </div>
  );
}

/** اسکلت بارگذاری. */
export function LoadingSkeleton({ rows = 3 }: { rows?: number }) {
  const { t } = useTranslation();
  return (
    <div data-testid="loading-skeleton" aria-busy="true" aria-label={t("state.loading")} className="flex flex-col gap-3">
      {Array.from({ length: rows }, (_, i) => (
        <div
          key={i}
          className="h-10 animate-pulse rounded bg-surface-alt"
          style={{ animationDelay: `${i * 120}ms` }}
        />
      ))}
    </div>
  );
}
