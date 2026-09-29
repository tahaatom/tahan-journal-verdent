//! پنل جزئیات معامله — موثر/پاها/اجراها/بازنویسی‌ها/پیوست‌ها/فیلد سفارشی
//! و اکشن‌ها: ویرایش، حذف نرم با تأیید و دلیل، بازنویسی دستی، تخصیص اجرا.

import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TradeDetails } from "../../kernel";
import type { TradeListBridge } from "./bridge";
import { displayValue, effectiveDiff } from "./effective";

const inputCls =
  "rounded border border-border bg-surface px-2 py-1.5 text-sm focus:border-accent focus:outline-none";

/** پیام خطای امن — هر ورودی (Error/گزارش IPC) به رشته تبدیل می‌شود. */
function errMsg(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "object" && e !== null && "message" in e) {
    return String((e as { message: unknown }).message);
  }
  return String(e);
}

const LINK_KINDS = ["before_trade", "after_trade", "chart", "news", "other"] as const;

interface TradeDetailsPanelProps {
  tradeId: string;
  bridge: TradeListBridge;
  onClose: () => void;
  onDeleted: () => void;
}

export function TradeDetailsPanel({
  tradeId,
  bridge,
  onClose,
  onDeleted,
}: TradeDetailsPanelProps) {
  const { t } = useTranslation();
  const [details, setDetails] = useState<TradeDetails | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // ویرایش
  const [editNote, setEditNote] = useState("");
  const [editStatus, setEditStatus] = useState("");
  // حذف
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleteReason, setDeleteReason] = useState("");
  // بازنویسی دستی
  const [ovField, setOvField] = useState("note");
  const [ovValue, setOvValue] = useState("");
  const [ovReason, setOvReason] = useState("");
  // پیوست
  const [linkKind, setLinkKind] = useState<(typeof LINK_KINDS)[number]>("chart");
  // پیام موفقیت
  const [flash, setFlash] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    bridge
      .details(tradeId)
      .then((d) => {
        if (!alive) return;
        setDetails(d);
        setEditNote(d.trade.note ?? "");
        setEditStatus(d.trade.status);
      })
      .catch((e) => alive && setError(errMsg(e)));
    return () => {
      alive = false;
    };
  }, [bridge, tradeId]);

  const diff = useMemo(() => (details ? effectiveDiff(details) : null), [details]);

  const run = async (fn: () => Promise<unknown>, successMsg: string) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
      const fresh = await bridge.details(tradeId);
      setDetails(fresh);
      setEditNote(fresh.trade.note ?? "");
      setEditStatus(fresh.trade.status);
      setFlash(successMsg);
    } catch (e) {
      setError(errMsg(e));
    } finally {
      setBusy(false);
    }
  };

  const saveEdits = () =>
    run(
      () =>
        bridge.executeCommand("domain.update_trade", {
          trade_id: tradeId,
          note: editNote.trim() === "" ? null : editNote,
          status: editStatus,
        }),
      t("list.savedEdits"),
    );

  const doDelete = () =>
    run(
      () =>
        bridge.executeCommand("domain.delete_trade", {
          trade_id: tradeId,
          reason: deleteReason.trim() === "" ? null : deleteReason,
        }),
      t("list.deleted"),
    ).then(() => onDeleted());

  const addOverride = () =>
    run(
      () =>
        bridge.executeCommand("domain.add_manual_override", {
          entity_type: "journal_trade",
          entity_id: tradeId,
          field_name: ovField,
          new_value: ovValue,
          reason: ovReason,
          source: "manual",
          priority: 10,
          reversible: true,
          created_by: "user",
        }),
      t("list.overrideAdded"),
    );

  const revertOverride = (id: string) =>
    run(
      () =>
        bridge.executeCommand("domain.revert_manual_override", {
          override_id: id,
        }),
      t("list.overrideReverted"),
    );

  const assignExecution = (executionId: string, legId: string) =>
    run(
      () =>
        bridge.executeCommand("domain.assign_execution_to_leg", {
          execution_id: executionId,
          leg_id: legId,
        }),
      t("list.assigned"),
    );

  const addAttachment = async (file: File) => {
    const data = new Uint8Array(await file.arrayBuffer());
    await run(
      () =>
        bridge.ingestAttachment({
          tradeId,
          fileName: file.name,
          mimeType: file.type || null,
          data,
          linkKind,
        }),
      t("list.attachmentAdded"),
    );
  };

  if (error && !details) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" dir="rtl">
        <div className="rounded bg-surface p-6 text-sm" data-testid="details-error">
          <p role="alert" className="text-red-500">{error}</p>
          <button onClick={onClose} className="mt-3 rounded border border-border px-3 py-1">
            {t("list.close")}
          </button>
        </div>
      </div>
    );
  }

  if (!details) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" dir="rtl">
        <div className="rounded bg-surface p-6 text-sm">{t("list.loading")}</div>
      </div>
    );
  }

  const d = details;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4" dir="rtl">
      <div
        data-testid="trade-details-panel"
        className="max-h-[90vh] w-full max-w-3xl overflow-auto rounded border border-border bg-surface p-4 text-sm"
      >
        {/* سرصفحه */}
        <div className="flex items-center justify-between">
          <h2 className="text-base font-bold">
            {d.trade.symbol_id} — {t(`journal.${d.trade.direction}`)}{" "}
            <span className="text-xs text-text-muted">({tradeId.slice(0, 8)})</span>
          </h2>
          <button
            type="button"
            data-testid="close-details"
            onClick={onClose}
            className="rounded border border-border px-2 py-0.5 text-xs"
          >
            {t("list.close")}
          </button>
        </div>

        {flash && (
          <p data-testid="details-flash" className="mt-2 rounded bg-green-100 px-2 py-1 text-xs text-green-800 dark:bg-green-900 dark:text-green-200">
            {flash}
          </p>
        )}
        {error && (
          <p role="alert" data-testid="details-error" className="mt-2 rounded bg-red-100 px-2 py-1 text-xs text-red-700 dark:bg-red-900 dark:text-red-200">
            {error}
          </p>
        )}

        {/* خلاصه */}
        <section className="mt-3 grid grid-cols-4 gap-2" data-testid="details-summary">
          <Stat label={t("list.colStatus")} value={t(`journal.status${d.trade.status === "open" ? "Open" : d.trade.status === "closed" ? "Closed" : "Cancelled"}`)} />
          <Stat label={t("journal.plannedR")} value={displayValue(d.trade.planned_r)} />
          <Stat label={t("list.colR")} value={displayValue(d.trade.realized_r)} />
          <Stat label={t("list.colPnl")} value={displayValue(d.trade.realized_pnl)} />
          <Stat label={t("list.riskStatus")} value={t(`journal.risk_${d.trade.risk_calculation_status}`)} />
          <Stat label={t("journal.initialStopLoss")} value={displayValue(d.trade.initial_stop_loss)} />
          <Stat label={t("journal.takeProfit")} value={displayValue(d.trade.take_profit)} />
          <Stat label={t("list.colEntryTime")} value={displayValue(d.trade.entry_time?.slice(0, 16).replace("T", " "))} />
        </section>

        {/* تفاوت موثر با canonical */}
        {diff !== null && (
          <section className="mt-3" data-testid="effective-diff">
            <h3 className="font-bold">{t("list.effectiveDiff")}</h3>
            {diff.length === 0 ? (
              <p className="text-xs text-text-muted">{t("list.noEffectiveDiff")}</p>
            ) : (
              <ul className="mt-1 flex flex-col gap-1">
                {diff.map((item) => (
                  <li key={item.field} className="rounded bg-surface-alt px-2 py-1 text-xs">
                    <span className="font-bold">{item.field}</span>: {displayValue(item.canonical)} →{" "}
                    <span className="text-accent">{displayValue(item.effective)}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        )}

        {/* پاها */}
        <LegsSection details={d} />

        {/* اجراها */}
        <section className="mt-3">
          <h3 className="font-bold">{t("list.executions")}</h3>
          {d.executions.length === 0 ? (
            <p className="text-xs text-text-muted">{t("list.none")}</p>
          ) : (
            <ul className="mt-1 flex flex-col gap-1">
              {d.executions.map((ex) => (
                <li key={ex.id} className="flex items-center gap-2 rounded bg-surface-alt px-2 py-1 text-xs">
                  <span data-testid={`execution-${ex.id.slice(0, 8)}`}>
                    {ex.kind === "manual" ? t("list.manualExec") : t("list.fillExec")} —{" "}
                    {t(`journal.${ex.direction}`)} {ex.volume} @ {ex.price}
                  </span>
                  <span className={ex.assignment_status === "assigned" ? "text-green-600" : "text-amber-600"}>
                    {t(`list.${ex.assignment_status}`)}
                  </span>
                  {ex.assignment_status === "needs_assignment" && d.entry_legs.length > 0 && (
                    <select
                      data-testid={`assign-select-${ex.id.slice(0, 8)}`}
                      defaultValue=""
                      onChange={(e) => e.target.value && assignExecution(ex.id, e.target.value)}
                      className={inputCls}
                    >
                      <option value="">{t("list.assignTo")}</option>
                      {d.entry_legs.map((leg) => (
                        <option key={leg.id} value={leg.id}>
                          {t("list.entryLegN", { n: d.entry_legs.indexOf(leg) + 1 })}
                        </option>
                      ))}
                    </select>
                  )}
                </li>
              ))}
            </ul>
          )}
        </section>

        {/* بازنویسی‌ها */}
        <section className="mt-3">
          <h3 className="font-bold">{t("list.overrides")}</h3>
          {d.overrides.length === 0 ? (
            <p className="text-xs text-text-muted">{t("list.none")}</p>
          ) : (
            <ul className="mt-1 flex flex-col gap-1">
              {d.overrides.map((ov) => (
                <li key={ov.id} className="flex items-center gap-2 rounded bg-surface-alt px-2 py-1 text-xs">
                  <span>
                    <span className="font-bold">{ov.field_name}</span>: {displayValue(ov.previous_value)} →{" "}
                    {displayValue(ov.new_value)}
                  </span>
                  {ov.reason && <span className="text-text-muted">({ov.reason})</span>}
                  {ov.reversible && !ov.reverted_at && (
                    <button
                      type="button"
                      data-testid={`revert-override-${ov.id.slice(0, 8)}`}
                      disabled={busy}
                      onClick={() => revertOverride(ov.id)}
                      className="rounded border border-border px-2 py-0.5"
                    >
                      {t("list.revert")}
                    </button>
                  )}
                  {ov.reverted_at && <span className="text-text-muted">{t("list.reverted")}</span>}
                </li>
              ))}
            </ul>
          )}
          <div className="mt-2 flex items-center gap-2">
            <select
              data-testid="override-field"
              value={ovField}
              onChange={(e) => setOvField(e.target.value)}
              className={inputCls}
            >
              {["note", "strategy", "timeframe", "session", "emotions", "mistakes", "tags"].map((f) => (
                <option key={f} value={f}>{f}</option>
              ))}
            </select>
            <input
              data-testid="override-value"
              placeholder={t("list.newValue")}
              value={ovValue}
              onChange={(e) => setOvValue(e.target.value)}
              className={`${inputCls} grow`}
            />
            <input
              data-testid="override-reason"
              placeholder={t("list.reason")}
              value={ovReason}
              onChange={(e) => setOvReason(e.target.value)}
              className={`${inputCls} grow`}
            />
            <button
              type="button"
              data-testid="add-override"
              disabled={busy || ovValue === ""}
              onClick={addOverride}
              className="rounded bg-accent px-3 py-1 text-xs font-bold text-surface disabled:opacity-40"
            >
              {t("list.addOverride")}
            </button>
          </div>
        </section>

        {/* پیوست‌ها */}
        <section className="mt-3">
          <h3 className="font-bold">{t("list.attachments")}</h3>
          {d.attachments.length === 0 ? (
            <p className="text-xs text-text-muted">{t("list.none")}</p>
          ) : (
            <ul className="mt-1 flex flex-col gap-1">
              {d.attachments.map((a) => (
                <li key={a.id} className="flex items-center gap-2 rounded bg-surface-alt px-2 py-1 text-xs">
                  <span data-testid={`attachment-${a.file_name}`}>
                    {a.file_name} ({Math.max(1, Math.ceil(a.size_bytes / 1024))} KB)
                  </span>
                  <span className="text-text-muted">{t(`journal.link_${a.link_kind}`)}</span>
                </li>
              ))}
            </ul>
          )}
          <div className="mt-2 flex items-center gap-2">
            <select
              data-testid="attachment-kind"
              value={linkKind}
              onChange={(e) => setLinkKind(e.target.value as (typeof LINK_KINDS)[number])}
              className={inputCls}
            >
              {LINK_KINDS.map((k) => (
                <option key={k} value={k}>{t(`journal.link_${k}`)}</option>
              ))}
            </select>
            <input
              type="file"
              data-testid="attachment-input"
              onChange={(e) => {
                const f = e.target.files?.[0];
                if (f) addAttachment(f);
                e.target.value = "";
              }}
              className="text-xs"
            />
          </div>
        </section>

        {/* فیلدهای سفارشی */}
        {Object.keys(d.custom_values).length > 0 && (
          <section className="mt-3" data-testid="details-custom-values">
            <h3 className="font-bold">{t("list.customValues")}</h3>
            <ul className="mt-1 flex flex-wrap gap-2">
              {Object.entries(d.custom_values).map(([k, v]) => (
                <li key={k} className="rounded bg-surface-alt px-2 py-1 text-xs">
                  <span className="font-bold">{k}</span>: {displayValue(v)}
                </li>
              ))}
            </ul>
          </section>
        )}

        {/* ویرایش سریع */}
        <section className="mt-3 flex flex-wrap items-end gap-2 border-t border-border pt-3">
          <label className="flex flex-col gap-1 text-xs">
            {t("journal.note")}
            <textarea
              data-testid="edit-note"
              value={editNote}
              onChange={(e) => setEditNote(e.target.value)}
              rows={2}
              className={`${inputCls} w-64`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {t("list.colStatus")}
            <select
              data-testid="edit-status"
              value={editStatus}
              onChange={(e) => setEditStatus(e.target.value)}
              className={inputCls}
            >
              <option value="open">{t("journal.statusOpen")}</option>
              <option value="closed">{t("journal.statusClosed")}</option>
              <option value="cancelled">{t("journal.statusCancelled")}</option>
            </select>
          </label>
          <button
            type="button"
            data-testid="save-edits"
            disabled={busy}
            onClick={saveEdits}
            className="rounded bg-accent px-4 py-1.5 text-xs font-bold text-surface disabled:opacity-40"
          >
            {t("list.save")}
          </button>
          <span className="grow" />
          {confirmDelete ? (
            <span className="flex items-end gap-2" data-testid="delete-confirm">
              <input
                data-testid="delete-reason"
                placeholder={t("list.deleteReason")}
                value={deleteReason}
                onChange={(e) => setDeleteReason(e.target.value)}
                className={inputCls}
              />
              <button
                type="button"
                data-testid="confirm-delete"
                disabled={busy}
                onClick={doDelete}
                className="rounded bg-red-600 px-3 py-1.5 text-xs font-bold text-white"
              >
                {t("list.confirmDelete")}
              </button>
              <button
                type="button"
                data-testid="cancel-delete"
                onClick={() => setConfirmDelete(false)}
                className="rounded border border-border px-3 py-1.5 text-xs"
              >
                {t("list.cancel")}
              </button>
            </span>
          ) : (
            <button
              type="button"
              data-testid="delete-trade"
              onClick={() => setConfirmDelete(true)}
              className="rounded border border-red-500 px-3 py-1.5 text-xs text-red-500"
            >
              {t("list.delete")}
            </button>
          )}
        </section>
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded bg-surface-alt px-2 py-1 text-xs">
      <div className="text-text-muted">{label}</div>
      <div data-testid={`stat-${label}`} className="font-bold">{value}</div>
    </div>
  );
}

function LegsSection({ details }: { details: TradeDetails }) {
  const { t } = useTranslation();
  return (
    <>
      <section className="mt-3">
        <h3 className="font-bold">{t("list.entryLegs")}</h3>
        {details.entry_legs.length === 0 ? (
          <p className="text-xs text-text-muted">{t("list.none")}</p>
        ) : (
          <table className="mt-1 w-full text-xs">
            <thead>
              <tr className="text-text-muted">
                <th className="text-right">{t("list.planned")}</th>
                <th className="text-right">{t("list.executed")}</th>
                <th className="text-right">{t("journal.volume")}</th>
                <th className="text-right">{t("journal.stopLoss")}</th>
                <th className="text-right">{t("journal.takeProfit")}</th>
              </tr>
            </thead>
            <tbody>
              {details.entry_legs.map((leg) => (
                <tr key={leg.id} data-testid={`entry-leg-${leg.id.slice(0, 8)}`} className="odd:bg-surface-alt/40">
                  <td className="px-1 py-0.5">{displayValue(leg.planned_price)}</td>
                  <td className="px-1 py-0.5">{displayValue(leg.executed_price)}</td>
                  <td className="px-1 py-0.5">{leg.volume}</td>
                  <td className="px-1 py-0.5">{displayValue(leg.stop_loss)}</td>
                  <td className="px-1 py-0.5">{displayValue(leg.take_profit)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
      <section className="mt-3">
        <h3 className="font-bold">{t("list.exitLegs")}</h3>
        {details.exit_legs.length === 0 ? (
          <p className="text-xs text-text-muted">{t("list.none")}</p>
        ) : (
          <table className="mt-1 w-full text-xs">
            <thead>
              <tr className="text-text-muted">
                <th className="text-right">{t("list.executed")}</th>
                <th className="text-right">{t("journal.volume")}</th>
                <th className="text-right">{t("list.exitReason")}</th>
                <th className="text-right">{t("list.colExitTime")}</th>
              </tr>
            </thead>
            <tbody>
              {details.exit_legs.map((leg) => (
                <tr key={leg.id} data-testid={`exit-leg-${leg.id.slice(0, 8)}`} className="odd:bg-surface-alt/40">
                  <td className="px-1 py-0.5">{displayValue(leg.executed_price)}</td>
                  <td className="px-1 py-0.5">{leg.volume}</td>
                  <td className="px-1 py-0.5">{displayValue(leg.exit_reason)}</td>
                  <td className="px-1 py-0.5">{displayValue(leg.exit_time?.slice(0, 16).replace("T", " "))}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </>
  );
}
