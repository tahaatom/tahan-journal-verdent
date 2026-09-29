//! پنل فیلتر فهرست معاملات — فیلدهای ساده، بازه تاریخ، جست‌وجو،
//! فیلدهای سفارشی و ترکیب AND/OR.

import { useTranslation } from "react-i18next";
import {
  validateFilterState,
  type CustomFilterRow,
  type FilterState,
} from "./filterState";
import type { CustomFieldDefLike } from "../fieldTypes";

const inputCls =
  "rounded border border-border bg-surface px-2 py-1.5 text-sm focus:border-accent focus:outline-none";

const OPS = [
  { value: "equals", labelKey: "opEquals" },
  { value: "not_equals", labelKey: "opNotEquals" },
  { value: "contains", labelKey: "opContains" },
  { value: "min", labelKey: "opMin" },
  { value: "max", labelKey: "opMax" },
  { value: "exists", labelKey: "opExists" },
] as const;

interface TradeFiltersProps {
  state: FilterState;
  onChange: (s: FilterState) => void;
  accounts: { id: string; name: string; currency: string }[];
  symbols: { id: string; name: string }[];
  fields: CustomFieldDefLike[];
  onApply: () => void;
  onReset: () => void;
}

/**
 * پنل فیلتر — تغییرات در وضعیت محلی می‌مانند و با «اعمال» به فهرست
 * می‌روند تا هر تغییر کلید، پرس‌وجوی سمت کرنل راه نیندازد.
 */
export function TradeFilters({
  state,
  onChange,
  accounts,
  symbols,
  fields,
  onApply,
  onReset,
}: TradeFiltersProps) {
  const { t } = useTranslation();
  const errors = validateFilterState(state, fields);

  const patch = (p: Partial<FilterState>) => onChange({ ...state, ...p });

  const patchRow = (i: number, p: Partial<CustomFilterRow>) => {
    patch({
      customRows: state.customRows.map((r, j) => (j === i ? { ...r, ...p } : r)),
    });
  };

  const addRow = () => {
    const first = fields[0];
    if (!first) return;
    patch({
      customRows: [
        ...state.customRows,
        { fieldKey: first.technical_key, op: "equals", value: "" },
      ],
    });
  };

  return (
    <section
      data-testid="trade-filters"
      dir="rtl"
      className="flex flex-col gap-3 rounded border border-border p-3"
    >
      <div className="grid grid-cols-4 gap-2">
        <select
          data-testid="filter-account"
          value={state.accountId}
          onChange={(e) => patch({ accountId: e.target.value })}
          className={inputCls}
        >
          <option value="">{t("list.allAccounts")}</option>
          {accounts.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
            </option>
          ))}
        </select>
        <select
          data-testid="filter-symbol"
          value={state.symbolId}
          onChange={(e) => patch({ symbolId: e.target.value })}
          className={inputCls}
        >
          <option value="">{t("list.allSymbols")}</option>
          {symbols.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>
        <select
          data-testid="filter-direction"
          value={state.direction}
          onChange={(e) => patch({ direction: e.target.value as FilterState["direction"] })}
          className={inputCls}
        >
          <option value="">{t("list.allDirections")}</option>
          <option value="buy">{t("journal.buy")}</option>
          <option value="sell">{t("journal.sell")}</option>
        </select>
        <select
          data-testid="filter-status"
          value={state.status}
          onChange={(e) => patch({ status: e.target.value as FilterState["status"] })}
          className={inputCls}
        >
          <option value="">{t("list.allStatuses")}</option>
          <option value="open">{t("journal.statusOpen")}</option>
          <option value="closed">{t("journal.statusClosed")}</option>
          <option value="cancelled">{t("journal.statusCancelled")}</option>
        </select>
      </div>

      <div className="grid grid-cols-4 gap-2">
        <input
          data-testid="filter-strategy"
          placeholder={t("journal.strategy")}
          value={state.strategy}
          onChange={(e) => patch({ strategy: e.target.value })}
          className={inputCls}
        />
        <input
          data-testid="filter-timeframe"
          placeholder={t("journal.timeframe")}
          value={state.timeframe}
          onChange={(e) => patch({ timeframe: e.target.value })}
          className={inputCls}
        />
        <input
          data-testid="filter-session"
          placeholder={t("journal.session")}
          value={state.session}
          onChange={(e) => patch({ session: e.target.value })}
          className={inputCls}
        />
        <input
          data-testid="filter-search"
          placeholder={t("list.searchPlaceholder")}
          value={state.search}
          onChange={(e) => patch({ search: e.target.value })}
          className={inputCls}
        />
      </div>

      <div className="grid grid-cols-4 gap-2">
        <input
          data-testid="filter-tags"
          placeholder={t("journal.tags")}
          value={state.tags}
          onChange={(e) => patch({ tags: e.target.value })}
          className={inputCls}
        />
        <input
          data-testid="filter-emotions"
          placeholder={t("journal.emotions")}
          value={state.emotions}
          onChange={(e) => patch({ emotions: e.target.value })}
          className={inputCls}
        />
        <input
          data-testid="filter-mistakes"
          placeholder={t("journal.mistakes")}
          value={state.mistakes}
          onChange={(e) => patch({ mistakes: e.target.value })}
          className={inputCls}
        />
        <span className="grid grid-cols-2 gap-2">
          <input
            data-testid="filter-entry-from"
            type="date"
            aria-label={t("list.fromDate")}
            value={state.entryFrom}
            onChange={(e) => patch({ entryFrom: e.target.value })}
            className={inputCls}
          />
          <input
            data-testid="filter-entry-to"
            type="date"
            aria-label={t("list.toDate")}
            value={state.entryTo}
            onChange={(e) => patch({ entryTo: e.target.value })}
            className={inputCls}
          />
        </span>
      </div>

      {/* فیلدهای سفارشی */}
      {fields.length > 0 && (
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <span className="text-xs text-text-muted">{t("list.customFilters")}</span>
            <button
              type="button"
              data-testid="add-custom-filter"
              onClick={addRow}
              className="rounded border border-border px-2 py-0.5 text-xs hover:bg-accent-soft"
            >
              +
            </button>
            <span className="grow" />
            <span className="text-xs text-text-muted">{t("list.combine")}</span>
            <button
              type="button"
              data-testid="combine-and"
              data-active={state.combine === "and"}
              onClick={() => patch({ combine: "and" })}
              className={`rounded px-2 py-0.5 text-xs ${
                state.combine === "and" ? "bg-accent text-surface" : "border border-border"
              }`}
            >
              {t("list.combineAnd")}
            </button>
            <button
              type="button"
              data-testid="combine-or"
              data-active={state.combine === "or"}
              onClick={() => patch({ combine: "or" })}
              className={`rounded px-2 py-0.5 text-xs ${
                state.combine === "or" ? "bg-accent text-surface" : "border border-border"
              }`}
            >
              {t("list.combineOr")}
            </button>
          </div>
          {state.customRows.map((row, i) => (
            <div key={i} className="flex items-center gap-2">
              <select
                data-testid={`custom-filter-field-${i}`}
                value={row.fieldKey}
                onChange={(e) => patchRow(i, { fieldKey: e.target.value })}
                className={inputCls}
              >
                {fields.map((f) => (
                  <option key={f.technical_key} value={f.technical_key}>
                    {f.display_label}
                  </option>
                ))}
              </select>
              <select
                data-testid={`custom-filter-op-${i}`}
                value={row.op}
                onChange={(e) => patchRow(i, { op: e.target.value as CustomFilterRow["op"] })}
                className={inputCls}
              >
                {OPS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {t(`list.${o.labelKey}`)}
                  </option>
                ))}
              </select>
              {row.op !== "exists" && (
                <input
                  data-testid={`custom-filter-value-${i}`}
                  value={row.value}
                  onChange={(e) => patchRow(i, { value: e.target.value })}
                  className={`${inputCls} grow`}
                />
              )}
              {errors[String(i)] && (
                <span role="alert" className="text-xs text-red-500">
                  {errors[String(i)]}
                </span>
              )}
              <button
                type="button"
                data-testid={`remove-custom-filter-${i}`}
                onClick={() =>
                  patch({ customRows: state.customRows.filter((_, j) => j !== i) })
                }
                className="text-xs text-red-500"
              >
                ×
              </button>
            </div>
          ))}
        </div>
      )}

      <div className="flex gap-2">
        <button
          type="button"
          data-testid="apply-filters"
          onClick={onApply}
          className="rounded bg-accent px-4 py-1.5 text-sm font-bold text-surface"
        >
          {t("list.apply")}
        </button>
        <button
          type="button"
          data-testid="reset-filters"
          onClick={onReset}
          className="rounded border border-border px-4 py-1.5 text-sm hover:bg-accent-soft"
        >
          {t("list.reset")}
        </button>
      </div>
    </section>
  );
}
