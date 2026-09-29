//! فرم ثبت معامله — دو حالت سریع و کامل با فیلدهای سفارشی داینامیک.
//!
//! اصول:
//! - پل (`TradeFormBridge`) تزریقی است؛ فرم مستقیم به IPC وابسته نیست.
//! - پیش‌نویس با تاخیر کوتاه ذخیره و پس از بازیابی، بایت پیوست‌ها باید
//!   دوباره انتخاب شود (فراداده حفظ می‌ماند).
//! - پیش‌نمایش R برنامه‌ریزی‌شده زنده است و نبود SL هشدار می‌دهد.
//! - چیپ‌های پیشنهاد و «آخرین استفاده» برای حداقل‌سازی تایپ.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  emptyEntryLeg,
  emptyExitLeg,
  emptyFormState,
  type EntryLegDraft,
  type ExitLegDraft,
  type PendingAttachment,
  type TradeFormBridge,
  type TradeFormState,
} from "./types";
import { LINK_KINDS, loadLastUsed, saveLastUsed, SUGGESTIONS, type LastUsedKey } from "./options";
import { plannedR } from "./plannedR";
import { clearDraft, hasDraft, loadDraft, saveDraft } from "./draft";
import { executeSubmitSteps, validateForm } from "./submit";
import type { CustomFieldDefLike, FieldOptionLike } from "./fieldTypes";

const DRAFT_DEBOUNCE_MS = 400;

/** برچسب بخش‌های گروه فیلد سفارشی. */
function groupLabel(group: string | null): string {
  return group && group.trim() !== "" ? group : "عمومی";
}

interface ChipProps {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
  testId?: string;
}

/** چیپ انتخاب سریع. */
function Chip({ active, onClick, children, testId }: ChipProps) {
  return (
    <button
      type="button"
      data-testid={testId}
      data-active={active}
      onClick={onClick}
      className={`rounded-full border px-3 py-1 text-xs transition-colors ${
        active
          ? "border-accent bg-accent text-surface"
          : "border-border bg-surface text-text-muted hover:bg-accent-soft"
      }`}
    >
      {children}
    </button>
  );
}

interface FieldShellProps {
  label: string;
  error?: string;
  required?: boolean;
  children: React.ReactNode;
}

/** قاب برچسب + خطا برای هر فیلد. */
function FieldShell({ label, error, required, children }: FieldShellProps) {
  return (
    <label className="flex flex-col gap-1 text-sm">
      <span className="text-text-muted">
        {label}
        {required && <span className="text-red-500"> *</span>}
      </span>
      {children}
      {error && (
        <span role="alert" className="text-xs text-red-500">
          {error}
        </span>
      )}
    </label>
  );
}

const inputCls =
  "rounded border border-border bg-surface px-2 py-1.5 text-sm focus:border-accent focus:outline-none";

interface TradeFormProps {
  bridge: TradeFormBridge;
  onSaved?: (tradeId: string) => void;
}

/**
 * فرم ثبت معامله — کامل، فارسی و راست‌به‌چپ.
 */
export function TradeForm({ bridge, onSaved }: TradeFormProps) {
  const { t } = useTranslation();
  const [state, setState] = useState<TradeFormState>(() => emptyFormState());
  const [accounts, setAccounts] = useState<{ id: string; name: string; currency: string }[]>([]);
  const [symbols, setSymbols] = useState<{ id: string; name: string }[]>([]);
  const [fields, setFields] = useState<CustomFieldDefLike[]>([]);
  const [optionMap, setOptionMap] = useState<Record<string, FieldOptionLike[]>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [submitting, setSubmitting] = useState(false);
  const [savedId, setSavedId] = useState<string | null>(null);
  const [recoverable, setRecoverable] = useState(false);
  const [lastUsed, setLastUsed] = useState(() => loadLastUsed());
  const draftTimer = useRef<number | null>(null);
  const dirty = useRef(false);

  // بارگذاری داده‌های مرجع و فیلدهای سفارشی + بررسی پیش‌نویس
  useEffect(() => {
    let cancelled = false;
    bridge.listAccounts().then((a) => !cancelled && setAccounts(a)).catch(() => {});
    bridge.listSymbols().then((s) => !cancelled && setSymbols(s)).catch(() => {});
    bridge.listFields().then((f) => {
      if (cancelled) return;
      setFields(f);
      for (const field of f) {
        if (needsOptions(field)) {
          bridge
            .fieldOptions(field.id)
            .then((opts) => !cancelled && setOptionMap((m) => ({ ...m, [field.id]: opts })))
            .catch(() => {});
        }
      }
    }).catch(() => {});
    if (hasDraft()) setRecoverable(true);
    return () => {
      cancelled = true;
    };
  }, [bridge]);

  // ذخیره خودکار پیش‌نویس با تاخیر — فقط پس از نخستین تغییر کاربر؛
  // پس از ثبت موفق (savedId) دیگر پیش‌نویسی نوشته نمی‌شود
  useEffect(() => {
    if (!dirty.current || savedId !== null) return;
    if (draftTimer.current !== null) window.clearTimeout(draftTimer.current);
    draftTimer.current = window.setTimeout(() => saveDraft(state), DRAFT_DEBOUNCE_MS);
    return () => {
      if (draftTimer.current !== null) window.clearTimeout(draftTimer.current);
    };
  }, [state, savedId]);

  const patch = useCallback((p: Partial<TradeFormState>) => {
    dirty.current = true;
    setState((s) => ({ ...s, ...p }));
  }, []);

  const patchLeg = useCallback(
    (index: number, p: Partial<EntryLegDraft>) => {
      dirty.current = true;
      setState((s) => ({
        ...s,
        entryLegs: s.entryLegs.map((leg, i) => (i === index ? { ...leg, ...p } : leg)),
      }));
    },
    [],
  );

  const patchExitLeg = useCallback(
    (index: number, p: Partial<ExitLegDraft>) => {
      dirty.current = true;
      setState((s) => ({
        ...s,
        exitLegs: s.exitLegs.map((leg, i) => (i === index ? { ...leg, ...p } : leg)),
      }));
    },
    [],
  );

  const setCustom = useCallback((fieldId: string, value: unknown) => {
    dirty.current = true;
    setState((s) => ({
      ...s,
      customValues: { ...s.customValues, [fieldId]: value },
    }));
  }, []);

  /** بازیابی پیش‌نویس ذخیره‌شده. */
  const recover = useCallback(() => {
    const draft = loadDraft();
    if (draft) {
      dirty.current = true;
      setState(draft);
    }
    setRecoverable(false);
  }, []);

  const discardDraft = useCallback(() => {
    clearDraft();
    setRecoverable(false);
  }, []);

  /** ثبت مقدار متنی + ذخیره «آخرین استفاده». */
  const setLastUsedField = useCallback(
    (key: LastUsedKey, value: string) => {
      patch({ [key]: value } as Partial<TradeFormState>);
      saveLastUsed(key, value);
      setLastUsed(loadLastUsed());
    },
    [patch],
  );

  const addAttachment = useCallback(
    (file: File, linkKind: string) => {
      const reader = new FileReader();
      reader.onload = () => {
        const data = new Uint8Array(reader.result as ArrayBuffer);
        const pending: PendingAttachment = {
          fileName: file.name,
          mimeType: file.type || null,
          size: file.size,
          linkKind,
          data,
        };
        setState((s) => ({ ...s, attachments: [...s.attachments, pending] }));
        dirty.current = true;
      };
      reader.readAsArrayBuffer(file);
    },
    [],
  );

  const removeAttachment = useCallback((index: number) => {
    setState((s) => ({
      ...s,
      attachments: s.attachments.filter((_, i) => i !== index),
    }));
    dirty.current = true;
  }, []);

  const isFast = state.mode === "fast";

  // پیش‌نمایش R: حالت سریع از پای اول؛ حالت کامل برای هر پای ورود
  const fastR = useMemo(
    () =>
      plannedR(
        state.entryLegs[0]?.executed_price ?? state.entryLegs[0]?.planned_price ?? null,
        state.entryLegs[0]?.stop_loss ?? null,
        state.entryLegs[0]?.take_profit ?? null,
        state.direction,
      ),
    [state.entryLegs, state.direction],
  );

  const submit = useCallback(async () => {
    const errs = validateForm(state, fields);
    if (Object.keys(errs).length > 0) {
      setErrors(errs);
      return;
    }
    setErrors({});
    setSubmitting(true);
    try {
      const tradeId = await executeSubmitSteps(state, bridge, (id) => {
        // نگهداشت شناسه برای ادامه در صورت شکست گام‌های بعدی
        setState((s) => ({ ...s, createdTradeId: id }));
      });
      // لغو ذخیره خودکار در انتظار و سپس حذف پیش‌نویس — در غیر این صورت
      // تایمر معوق پس از حذف، پیش‌نویس را دوباره می‌نویسد
      if (draftTimer.current !== null) {
        window.clearTimeout(draftTimer.current);
        draftTimer.current = null;
      }
      clearDraft();
      dirty.current = false;
      setSavedId(tradeId);
      setRecoverable(false);
      onSaved?.(tradeId);
    } catch (e) {
      setErrors({
        submit: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setSubmitting(false);
    }
  }, [state, fields, bridge, onSaved]);

  const err = (k: string) => errors[k];

  if (savedId) {
    return (
      <div data-testid="trade-saved" className="flex flex-col items-start gap-3 text-sm">
        <p className="rounded border border-green-500 bg-green-500/10 px-4 py-2 text-green-600 dark:text-green-400">
          {t("journal.saved")} — {savedId.slice(0, 8)}
        </p>
        <button
          type="button"
          data-testid="trade-new"
          onClick={() => {
            setSavedId(null);
            setState({ ...emptyFormState(), mode: state.mode });
          }}
          className="rounded border border-border px-4 py-2 hover:bg-accent-soft"
        >
          {t("journal.newTrade")}
        </button>
      </div>
    );
  }

  return (
    <div dir="rtl" className="flex max-w-3xl flex-col gap-4 text-sm">
      {/* انتخاب حالت */}
      <div className="flex items-center gap-2" role="radiogroup" aria-label={t("journal.mode")}>
        <Chip
          testId="mode-fast"
          active={isFast}
          onClick={() => patch({ mode: "fast" })}
        >
          {t("journal.modeFast")}
        </Chip>
        <Chip
          testId="mode-full"
          active={!isFast}
          onClick={() => patch({ mode: "full" })}
        >
          {t("journal.modeFull")}
        </Chip>
      </div>

      {recoverable && (
        <div data-testid="draft-banner" className="flex items-center justify-between rounded border border-border bg-surface-alt px-3 py-2 text-xs">
          <span>{t("journal.draftFound")}</span>
          <span className="flex gap-2">
            <button type="button" data-testid="draft-recover" onClick={recover} className="rounded bg-accent-soft px-2 py-1">
              {t("journal.draftRecover")}
            </button>
            <button type="button" data-testid="draft-discard" onClick={discardDraft} className="rounded border border-border px-2 py-1">
              {t("journal.draftDiscard")}
            </button>
          </span>
        </div>
      )}

      {/* فیلدهای پایه */}
      <div className="grid grid-cols-2 gap-3">
        <FieldShell label={t("journal.account")} error={err("accountId")} required>
          <select
            data-testid="field-account"
            value={state.accountId}
            onChange={(e) => patch({ accountId: e.target.value })}
            className={inputCls}
          >
            <option value="">{t("journal.select")}</option>
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name} ({a.currency})
              </option>
            ))}
          </select>
        </FieldShell>
        <FieldShell label={t("journal.symbol")} error={err("symbolId")} required>
          <select
            data-testid="field-symbol"
            value={state.symbolId}
            onChange={(e) => patch({ symbolId: e.target.value })}
            className={inputCls}
          >
            <option value="">{t("journal.select")}</option>
            {symbols.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </FieldShell>
      </div>

      <FieldShell label={t("journal.direction")} error={err("direction")} required>
        <div className="flex gap-2">
          <Chip testId="direction-buy" active={state.direction === "buy"} onClick={() => patch({ direction: "buy" })}>
            {t("journal.buy")}
          </Chip>
          <Chip testId="direction-sell" active={state.direction === "sell"} onClick={() => patch({ direction: "sell" })}>
            {t("journal.sell")}
          </Chip>
        </div>
      </FieldShell>

      {/* پاهای ورود */}
      <section className="flex flex-col gap-2">
        <h3 className="font-bold">{t("journal.entryLegs")}</h3>
        {state.entryLegs.map((leg, i) => (
          <div key={i} data-testid={`entry-leg-${i}`} className="grid grid-cols-3 gap-2 rounded border border-border p-2">
            <FieldShell label={t("journal.volume")} error={err(`entryLegs.${i}.volume`)} required>
              <input
                data-testid={`entry-leg-${i}-volume`}
                inputMode="decimal"
                value={leg.volume}
                onChange={(e) => patchLeg(i, { volume: e.target.value })}
                className={inputCls}
              />
            </FieldShell>
            <FieldShell label={t("journal.entryPrice")} error={err(`entryLegs.${i}.executed_price`)}>
              <input
                data-testid={`entry-leg-${i}-price`}
                inputMode="decimal"
                value={leg.executed_price}
                onChange={(e) => patchLeg(i, { executed_price: e.target.value })}
                className={inputCls}
              />
            </FieldShell>
            <FieldShell label={t("journal.stopLoss")} error={err(`entryLegs.${i}.stop_loss`)}>
              <input
                data-testid={`entry-leg-${i}-sl`}
                inputMode="decimal"
                value={leg.stop_loss}
                onChange={(e) => patchLeg(i, { stop_loss: e.target.value })}
                className={inputCls}
              />
            </FieldShell>
            <FieldShell label={t("journal.takeProfit")} error={err(`entryLegs.${i}.take_profit`)}>
              <input
                data-testid={`entry-leg-${i}-tp`}
                inputMode="decimal"
                value={leg.take_profit}
                onChange={(e) => patchLeg(i, { take_profit: e.target.value })}
                className={inputCls}
              />
            </FieldShell>
            <div className="col-span-2 flex items-end">
              <PlannedRPreview r={plannedR(leg.executed_price || leg.planned_price, leg.stop_loss, leg.take_profit, state.direction)} />
            </div>
            {!isFast && state.entryLegs.length > 1 && (
              <button
                type="button"
                data-testid={`remove-entry-leg-${i}`}
                onClick={() => patch({ entryLegs: state.entryLegs.filter((_, j) => j !== i) })}
                className="self-end text-xs text-red-500 hover:underline"
              >
                {t("journal.removeLeg")}
              </button>
            )}
          </div>
        ))}
        {!isFast && (
          <button
            type="button"
            data-testid="add-entry-leg"
            onClick={() => patch({ entryLegs: [...state.entryLegs, emptyEntryLeg()] })}
            className="w-fit rounded border border-border px-3 py-1 text-xs hover:bg-accent-soft"
          >
            {t("journal.addEntryLeg")}
          </button>
        )}
      </section>

      {/* بخش‌های فقط حالت کامل */}
      {!isFast && (
        <>
          <section className="flex flex-col gap-2">
            <h3 className="font-bold">{t("journal.exitLegs")}</h3>
            {state.exitLegs.map((leg, i) => (
              <div key={i} data-testid={`exit-leg-${i}`} className="grid grid-cols-3 gap-2 rounded border border-border p-2">
                <FieldShell label={t("journal.exitReason")} error={err(`exitLegs.${i}.exit_reason`)}>
                  <input
                    data-testid={`exit-leg-${i}-reason`}
                    value={leg.exit_reason}
                    onChange={(e) => patchExitLeg(i, { exit_reason: e.target.value })}
                    className={inputCls}
                  />
                </FieldShell>
                <FieldShell label={t("journal.exitPrice")} error={err(`exitLegs.${i}.executed_price`)} required>
                  <input
                    data-testid={`exit-leg-${i}-price`}
                    inputMode="decimal"
                    value={leg.executed_price}
                    onChange={(e) => patchExitLeg(i, { executed_price: e.target.value })}
                    className={inputCls}
                  />
                </FieldShell>
                <FieldShell label={t("journal.volume")} error={err(`exitLegs.${i}.volume`)} required>
                  <input
                    data-testid={`exit-leg-${i}-volume`}
                    inputMode="decimal"
                    value={leg.volume}
                    onChange={(e) => patchExitLeg(i, { volume: e.target.value })}
                    className={inputCls}
                  />
                </FieldShell>
                <button
                  type="button"
                  data-testid={`remove-exit-leg-${i}`}
                  onClick={() => patch({ exitLegs: state.exitLegs.filter((_, j) => j !== i) })}
                  className="self-end text-xs text-red-500 hover:underline"
                >
                  {t("journal.removeLeg")}
                </button>
              </div>
            ))}
            <button
              type="button"
              data-testid="add-exit-leg"
              onClick={() => patch({ exitLegs: [...state.exitLegs, emptyExitLeg()] })}
              className="w-fit rounded border border-border px-3 py-1 text-xs hover:bg-accent-soft"
            >
              {t("journal.addExitLeg")}
            </button>
          </section>

          <div className="grid grid-cols-2 gap-3">
            <FieldShell label={t("journal.status")}>
              <select
                data-testid="field-status"
                value={state.status}
                onChange={(e) => patch({ status: e.target.value as TradeFormState["status"] })}
                className={inputCls}
              >
                <option value="">{t("journal.statusOpenDefault")}</option>
                <option value="open">{t("journal.statusOpen")}</option>
                <option value="closed">{t("journal.statusClosed")}</option>
                <option value="cancelled">{t("journal.statusCancelled")}</option>
              </select>
            </FieldShell>
            <FieldShell label={t("journal.entryTime")} error={err("entryTime")}>
              <input
                data-testid="field-entry-time"
                type="datetime-local"
                value={state.entryTime}
                onChange={(e) => patch({ entryTime: e.target.value })}
                className={inputCls}
              />
            </FieldShell>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <FieldShell label={t("journal.commission")} error={err("commission")}>
              <input
                data-testid="field-commission"
                inputMode="decimal"
                value={state.commission}
                onChange={(e) => patch({ commission: e.target.value })}
                className={inputCls}
              />
            </FieldShell>
            <FieldShell label={t("journal.swap")} error={err("swap")}>
              <input
                data-testid="field-swap"
                inputMode="decimal"
                value={state.swap}
                onChange={(e) => patch({ swap: e.target.value })}
                className={inputCls}
              />
            </FieldShell>
          </div>

          {/* فیلدهای متادیتا با چیپ پیشنهاد/آخرین استفاده */}
          {(["strategy", "timeframe", "session", "marketCondition", "entryType"] as LastUsedKey[]).map(
            (key) => (
              <FieldShell key={key} label={t(`journal.${key}`)}>
                <input
                  data-testid={`field-${key}`}
                  value={String(state[key] ?? "")}
                  onChange={(e) => setLastUsedField(key, e.target.value)}
                  className={inputCls}
                />
                <div className="flex flex-wrap gap-1">
                  {lastUsed[key] && lastUsed[key] !== state[key] && (
                    <Chip active={false} onClick={() => setLastUsedField(key, lastUsed[key]!)}>
                      {lastUsed[key]}
                    </Chip>
                  )}
                  {SUGGESTIONS[key].map((s) => (
                    <Chip key={s} active={state[key] === s} onClick={() => setLastUsedField(key, s)}>
                      {s}
                    </Chip>
                  ))}
                </div>
              </FieldShell>
            ),
          )}

          <FieldShell label={t("journal.note")}>
            <textarea
              data-testid="field-note"
              rows={3}
              value={state.note}
              onChange={(e) => patch({ note: e.target.value })}
              className={inputCls}
            />
          </FieldShell>

          {(["tags", "emotions", "mistakes"] as const).map((key) => (
            <FieldShell key={key} label={t(`journal.${key}`)}>
              <input
                data-testid={`field-${key}`}
                value={state[key]}
                onChange={(e) => patch({ [key]: e.target.value } as Partial<TradeFormState>)}
                className={inputCls}
              />
              <div className="flex flex-wrap gap-1">
                {SUGGESTIONS[key].map((s) => (
                  <Chip
                    key={s}
                    active={state[key].includes(s)}
                    onClick={() => {
                      const parts = state[key].split(",").map((x) => x.trim()).filter(Boolean);
                      const next = parts.includes(s) ? parts.filter((x) => x !== s) : [...parts, s];
                      patch({ [key]: next.join(", ") } as Partial<TradeFormState>);
                    }}
                  >
                    {s}
                  </Chip>
                ))}
              </div>
            </FieldShell>
          ))}

          {/* فیلدهای سفارشی داینامیک — گروه‌بندی و ترتیب از اسکیمای کرنل */}
          {fields.length > 0 && <CustomFieldsSection
            fields={fields}
            optionMap={optionMap}
            values={state.customValues}
            errors={errors}
            onChange={setCustom}
          />}

          {/* پیوست‌ها */}
          <section className="flex flex-col gap-2">
            <h3 className="font-bold">{t("journal.attachments")}</h3>
            {state.attachments.map((a, i) => (
              <div key={i} data-testid={`attachment-${i}`} className="flex items-center justify-between rounded border border-border px-3 py-1.5 text-xs">
                <span>
                  {a.fileName} — {LINK_KINDS.find((k) => k.value === a.linkKind)?.label ?? a.linkKind}
                </span>
                <button type="button" data-testid={`attachment-remove-${i}`} onClick={() => removeAttachment(i)} className="text-red-500">
                  ×
                </button>
              </div>
            ))}
            <AttachmentAdder onAdd={addAttachment} />
          </section>
        </>
      )}

      {/* خطای کلی ثبت */}
      {errors.submit && (
        <p role="alert" data-testid="submit-error" className="rounded border border-red-500 bg-red-500/10 px-3 py-2 text-xs text-red-500">
          {errors.submit}
        </p>
      )}

      <div className="flex items-center gap-3">
        <button
          type="button"
          data-testid="trade-submit"
          disabled={submitting}
          onClick={submit}
          className="rounded bg-accent px-6 py-2 font-bold text-surface disabled:opacity-50"
        >
          {submitting ? t("journal.submitting") : t("journal.submit")}
        </button>
        <RSummary r={fastR} show={isFast} />
      </div>
    </div>
  );
}

/** پیش‌نمایش R برای یک پا. */
function PlannedRPreview({ r }: { r: ReturnType<typeof plannedR> }) {
  const { t } = useTranslation();
  if (r.invalid) return <span className="text-xs text-red-500">{r.invalid}</span>;
  if (r.value !== null) {
    return (
      <span data-testid="planned-r" className="text-xs text-accent">
        {t("journal.plannedR")}: {r.value.toFixed(2)}R
      </span>
    );
  }
  if (r.slMissing) {
    return (
      <span data-testid="sl-warning" className="text-xs text-amber-500">
        {t("journal.slMissingWarning")}
      </span>
    );
  }
  return null;
}

/** جمع‌بندی R کنار دکمه ثبت در حالت سریع. */
function RSummary({ r, show }: { r: ReturnType<typeof plannedR>; show: boolean }) {
  const { t } = useTranslation();
  if (!show || r.value === null) return null;
  return (
    <span data-testid="r-summary" className="text-xs text-text-muted">
      {t("journal.plannedR")}: {r.value.toFixed(2)}R
    </span>
  );
}

/** آیا فیلد به گزینه نیاز دارد؟ */
function needsOptions(field: CustomFieldDefLike): boolean {
  return field.storage_type === "enum" || field.storage_type === "multi_enum" || field.storage_type === "tag_set";
}

/** بخش فیلدهای سفارشی — گروه‌بندی بر اساس form_group و ترتیب display_order. */
function CustomFieldsSection({
  fields,
  optionMap,
  values,
  errors,
  onChange,
}: {
  fields: CustomFieldDefLike[];
  optionMap: Record<string, FieldOptionLike[]>;
  values: Record<string, unknown>;
  errors: Record<string, string>;
  onChange: (fieldId: string, value: unknown) => void;
}) {
  const { t } = useTranslation();
  const groups = useMemo(() => {
    const map = new Map<string, CustomFieldDefLike[]>();
    for (const f of [...fields].sort((a, b) => a.display_order - b.display_order)) {
      const g = groupLabel(f.form_group);
      if (!map.has(g)) map.set(g, []);
      map.get(g)!.push(f);
    }
    return [...map.entries()];
  }, [fields]);

  return (
    <section className="flex flex-col gap-3">
      <h3 className="font-bold">{t("journal.customFields")}</h3>
      {groups.map(([group, groupFields]) => (
        <fieldset key={group} data-testid={`custom-group-${group}`} className="rounded border border-border p-3">
          <legend className="px-1 text-xs text-text-muted">{group}</legend>
          <div className="grid grid-cols-2 gap-3">
            {groupFields.map((field) => (
              <CustomFieldInput
                key={field.id}
                field={field}
                options={optionMap[field.id] ?? []}
                value={values[field.id]}
                error={errors[`custom.${field.id}`]}
                onChange={(v) => onChange(field.id, v)}
              />
            ))}
          </div>
        </fieldset>
      ))}
    </section>
  );
}

/** ورودی فیلد سفارشی بر اساس نوع ذخیره‌سازی. */
function CustomFieldInput({
  field,
  options,
  value,
  error,
  onChange,
}: {
  field: CustomFieldDefLike;
  options: FieldOptionLike[];
  value: unknown;
  error?: string;
  onChange: (v: unknown) => void;
}) {
  const rules = field.validation_rules ?? {};
  const numeric = field.storage_type === "integer" || field.storage_type === "decimal" || field.storage_type === "rating";
  const label = field.display_label + (field.unit ? ` (${field.unit})` : "");

  if (field.storage_type === "boolean") {
    return (
      <FieldShell label={label} error={error} required={field.required}>
        <input
          type="checkbox"
          data-testid={`custom-${field.technical_key}`}
          checked={value === true}
          onChange={(e) => onChange(e.target.checked)}
          className="size-4"
        />
      </FieldShell>
    );
  }

  if (needsOptions(field) && options.length > 0) {
    if (field.storage_type === "enum") {
      return (
        <FieldShell label={label} error={error} required={field.required}>
          <select
            data-testid={`custom-${field.technical_key}`}
            value={String(value ?? "")}
            onChange={(e) => onChange(e.target.value === "" ? null : e.target.value)}
            className={inputCls}
          >
            <option value="">—</option>
            {options.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </FieldShell>
      );
    }
    // چندانتخابی — چیپ‌های قابل تاگل
    const selected = Array.isArray(value) ? (value as string[]) : [];
    return (
      <FieldShell label={label} error={error} required={field.required}>
        <div className="flex flex-wrap gap-1">
          {options.map((o) => (
            <Chip
              key={o.value}
              active={selected.includes(o.value)}
              onClick={() =>
                onChange(
                  selected.includes(o.value)
                    ? selected.filter((x) => x !== o.value)
                    : [...selected, o.value],
                )
              }
            >
              {o.label}
            </Chip>
          ))}
        </div>
      </FieldShell>
    );
  }

  if (field.storage_type === "long_text") {
    return (
      <FieldShell label={label} error={error} required={field.required}>
        <textarea
          data-testid={`custom-${field.technical_key}`}
          rows={2}
          maxLength={rules.max_length ?? undefined}
          value={String(value ?? "")}
          onChange={(e) => onChange(e.target.value === "" ? null : e.target.value)}
          className={inputCls}
        />
      </FieldShell>
    );
  }

  if (field.storage_type === "datetime") {
    return (
      <FieldShell label={label} error={error} required={field.required}>
        <input
          type="datetime-local"
          data-testid={`custom-${field.technical_key}`}
          value={String(value ?? "")}
          onChange={(e) => onChange(e.target.value === "" ? null : e.target.value)}
          className={inputCls}
        />
      </FieldShell>
    );
  }

  return (
    <FieldShell label={label} error={error} required={field.required}>
      <input
        data-testid={`custom-${field.technical_key}`}
        inputMode={numeric ? "decimal" : "text"}
        type={numeric ? "number" : "text"}
        min={rules.min ?? undefined}
        max={rules.max ?? undefined}
        maxLength={rules.max_length ?? undefined}
        value={value === null || value === undefined ? "" : String(value)}
        onChange={(e) => {
          const raw = e.target.value;
          if (numeric) onChange(raw === "" ? null : Number(raw));
          else onChange(raw === "" ? null : raw);
        }}
        className={inputCls}
      />
    </FieldShell>
  );
}

/** افزودن پیوست با انتخاب نوع پیوند. */
function AttachmentAdder({ onAdd }: { onAdd: (file: File, linkKind: string) => void }) {
  const { t } = useTranslation();
  const [linkKind, setLinkKind] = useState<string>("chart");
  const inputRef = useRef<HTMLInputElement>(null);
  return (
    <div className="flex items-center gap-2 text-xs">
      <select
        data-testid="attachment-link-kind"
        value={linkKind}
        onChange={(e) => setLinkKind(e.target.value)}
        className={inputCls}
      >
        {LINK_KINDS.map((k) => (
          <option key={k.value} value={k.value}>
            {k.label}
          </option>
        ))}
      </select>
      <input
        ref={inputRef}
        type="file"
        data-testid="attachment-file"
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) onAdd(file, linkKind);
          e.target.value = "";
        }}
        className="text-xs"
      />
      <span className="text-text-muted">{t("journal.attachmentHint")}</span>
    </div>
  );
}
