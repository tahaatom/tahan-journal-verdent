import { useTranslation } from "react-i18next";
import {
  renderOutcome,
  type DeclarativeExtension,
  type ExtensionPoint,
} from "../extensions";

/**
 * میزبان رندر افزونه‌ها — فقط خروجی `renderOutcome` را رندر می‌کند.
 *
 * کامپوننت‌های این فایل ثابت‌اند و از اسکیمای افزونه ساخته نمی‌شوند؛
 * بنابراین «تزریق React» ساختاراً ناممکن است (قاعده نسخه ۱ قرارداد plugin-api).
 */
export function DeclarativeWidget({
  point,
  ext,
}: {
  point: ExtensionPoint;
  ext: DeclarativeExtension;
}) {
  const { t } = useTranslation();
  const outcome = renderOutcome(point, ext);

  if (outcome.kind === "denied") {
    return (
      <div
        data-testid={`ext-denied-${ext.id}`}
        role="alert"
        className="rounded border border-danger p-3 text-xs text-danger"
      >
        {t(`error.ui.${outcome.violation ?? "injection_denied"}`)}
      </div>
    );
  }

  const title = ext.title || ext.plugin_id;

  if (outcome.kind === "divider") {
    return <hr data-testid={`ext-${ext.id}`} className="border-border" />;
  }

  return (
    <div
      data-testid={`ext-${ext.id}`}
      className="rounded-lg border border-border bg-surface-raised p-4"
    >
      <p className="mb-2 text-xs text-text-muted">{title}</p>
      {outcome.kind === "stat" && (
        <p data-testid={`ext-value-${ext.id}`} className="text-lg font-bold">
          {String(ext.schema["value"] ?? "—")}
        </p>
      )}
      {outcome.kind === "text" && (
        <p className="text-sm">{String(ext.schema["body"] ?? "")}</p>
      )}
      {outcome.kind === "list" && (
        <ul className="list-inside list-disc text-sm">
          {(Array.isArray(ext.schema["items"]) ? ext.schema["items"] : [])
            .map((item, i) => (
              <li key={i}>{String(item)}</li>
            ))}
        </ul>
      )}
      {(outcome.kind === "table" || outcome.kind === "command") && (
        <p className="text-sm text-text-muted">
          {String(ext.schema["body"] ?? title)}
        </p>
      )}
    </div>
  );
}
