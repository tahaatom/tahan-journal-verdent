//! اعتبارسنجی فرم با Zod و ساخت گام‌های ثبت — منطق خالص و آزمون‌پذیر.
//!
//! ثبت معامله چند دستور دامنه پشت‌سرهم است (ساخت معامله، پاها، فیلدهای
//! سفارشی، پیوست‌ها). هر دستور اتمیک است اما زنجیره اتمیک نیست؛ به همین
//! دلیل شناسه معامله ساخته‌شده در وضعیت فرم نگاه داشته می‌شود تا تلاش
//! مجدد ادامه یابد نه این‌که معامله دومی بسازد.

import { z } from "zod";
import type { CustomFieldDefLike } from "./fieldTypes";
import type { TradeFormState } from "./types";

// ---------- اعتبارسنجی ----------

/** اعتبارسنجی یک مقدار عددی مثبت اختیاری به‌صورت رشته فرم. */
const positiveDecimal = z
  .string()
  .trim()
  .refine((v) => v === "" || (Number.isFinite(Number(v)) && Number(v) > 0), {
    message: "باید عدد مثبت باشد",
  });

/** اسکیمای فیلدهای پایه — مشترک میان حالت سریع و کامل. */
export const baseFormSchema = z.object({
  accountId: z.string().min(1, "حساب الزامی است"),
  symbolId: z.string().min(1, "نماد الزامی است"),
  direction: z.enum(["buy", "sell"], { message: "جهت معامله الزامی است" }),
  commission: positiveDecimal,
  swap: positiveDecimal,
  entryTime: z
    .string()
    .trim()
    .refine((v) => v === "" || !Number.isNaN(Date.parse(v)), {
      message: "زمان ورود نامعتبر است",
    }),
});

/**
 * اعتبارسنجی کامل فرم شامل پاها و فیلدهای سفارشی.
 * خطاها با کلید مسیر برگردانده می‌شوند (مثل `entryLegs.0.volume`).
 */
export function validateForm(
  state: TradeFormState,
  fields: CustomFieldDefLike[],
): Record<string, string> {
  const errors: Record<string, string> = {};

  const base = baseFormSchema.safeParse({
    accountId: state.accountId,
    symbolId: state.symbolId,
    direction: state.direction,
    commission: state.commission,
    swap: state.swap,
    entryTime: state.entryTime,
  });
  if (!base.success) {
    for (const issue of base.error.issues) {
      errors[String(issue.path[0] ?? "base")] = issue.message;
    }
  }

  // پاهای ورود — حداقل یک پا با حجم معتبر
  if (state.entryLegs.length === 0) {
    errors["entryLegs"] = "حداقل یک پای ورود لازم است";
  }
  state.entryLegs.forEach((leg, i) => {
    const volume = Number(leg.volume);
    if (leg.volume.trim() === "" || !Number.isFinite(volume) || volume <= 0) {
      errors[`entryLegs.${i}.volume`] = "حجم باید عدد مثبت باشد";
    }
    for (const key of ["planned_price", "executed_price", "stop_loss", "take_profit"] as const) {
      const v = leg[key].trim();
      if (v !== "" && (!Number.isFinite(Number(v)) || Number(v) <= 0)) {
        errors[`entryLegs.${i}.${key}`] = "باید عدد مثبت باشد";
      }
    }
  });

  state.exitLegs.forEach((leg, i) => {
    const volume = Number(leg.volume);
    if (leg.volume.trim() === "" || !Number.isFinite(volume) || volume <= 0) {
      errors[`exitLegs.${i}.volume`] = "حجم باید عدد مثبت باشد";
    }
    const price = Number(leg.executed_price);
    if (leg.executed_price.trim() === "" || !Number.isFinite(price) || price <= 0) {
      errors[`exitLegs.${i}.executed_price`] = "قیمت اجرا الزامی و باید مثبت باشد";
    }
  });

  // فیلدهای سفارشی — قواعد تعریف اعلانی (اجباری بودن، بازه، طول)
  for (const field of fields) {
    const raw = state.customValues[field.id];
    const isEmpty =
      raw === undefined ||
      raw === null ||
      raw === "" ||
      (Array.isArray(raw) && raw.length === 0);
    if (field.required && isEmpty) {
      errors[`custom.${field.id}`] = `«${field.display_label}» الزامی است`;
      continue;
    }
    if (isEmpty) continue;
    const rules = field.validation_rules ?? {};
    if (typeof raw === "number") {
      if (rules.min != null && raw < rules.min) {
        errors[`custom.${field.id}`] = `«${field.display_label}» نباید کمتر از ${rules.min} باشد`;
      }
      if (rules.max != null && raw > rules.max) {
        errors[`custom.${field.id}`] = `«${field.display_label}» نباید بیشتر از ${rules.max} باشد`;
      }
    }
    if (typeof raw === "string" && rules.min_length != null && raw.length < rules.min_length) {
      errors[`custom.${field.id}`] = `«${field.display_label}» حداقل ${rules.min_length} نویسه است`;
    }
  }

  return errors;
}

// ---------- ساخت گام‌های ثبت ----------

/** یک گام ثبت — دستور دامنه یا عملیات جانبی با توضیح. */
export type SubmitStep =
  | { kind: "command"; commandType: string; payload: Record<string, unknown> }
  | { kind: "custom_values"; values: Record<string, unknown> }
  | { kind: "attachment"; attachmentIndex: number };

function opt(v: string): string | null {
  const t = v.trim();
  return t === "" ? null : t;
}

function optNum(v: string): number | null {
  const t = v.trim();
  if (t === "") return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}

/**
 * ساخت گام‌های ثبت از وضعیت فرم.
 * اگر `state.createdTradeId` مقدار داشته باشد (ادامه تلاش ناتمام)،
 * گام ساخت معامله حذف می‌شود.
 */
export function buildSubmitSteps(state: TradeFormState): SubmitStep[] {
  const steps: SubmitStep[] = [];

  if (!state.createdTradeId) {
    const first = state.entryLegs[0];
    steps.push({
      kind: "command",
      commandType: "domain.create_trade",
      payload: {
        account_id: state.accountId,
        symbol_id: state.symbolId,
        direction: state.direction,
        strategy: opt(state.strategy),
        timeframe: opt(state.timeframe),
        session: opt(state.session),
        market_condition: opt(state.marketCondition),
        entry_type: opt(state.entryType),
        note: opt(state.note),
        tags: opt(state.tags),
        emotions: opt(state.emotions),
        mistakes: opt(state.mistakes),
        entry_time: opt(state.entryTime),
        initial_stop_loss: optNum(first?.stop_loss ?? ""),
        take_profit: optNum(first?.take_profit ?? ""),
        commission: optNum(state.commission) ?? 0,
        swap: optNum(state.swap) ?? 0,
      },
    });
  }

  for (const leg of state.entryLegs) {
    steps.push({
      kind: "command",
      commandType: "domain.add_entry_leg",
      payload: {
        planned_price: optNum(leg.planned_price),
        executed_price: optNum(leg.executed_price),
        volume: Number(leg.volume),
        stop_loss: optNum(leg.stop_loss),
        take_profit: optNum(leg.take_profit),
        entry_time: opt(leg.entry_time),
        note: opt(leg.note),
      },
    });
  }

  for (const leg of state.exitLegs) {
    steps.push({
      kind: "command",
      commandType: "domain.add_exit_leg",
      payload: {
        exit_reason: opt(leg.exit_reason),
        executed_price: Number(leg.executed_price),
        volume: Number(leg.volume),
        exit_time: opt(leg.exit_time),
        note: opt(leg.note),
      },
    });
  }

  if (state.status !== "" && state.status !== "open") {
    steps.push({
      kind: "command",
      commandType: "domain.update_trade",
      payload: { status: state.status },
    });
  }

  const customValues: Record<string, unknown> = {};
  for (const [fieldId, value] of Object.entries(state.customValues)) {
    if (value !== undefined && value !== null && value !== "") {
      customValues[fieldId] = value;
    }
  }
  if (Object.keys(customValues).length > 0) {
    steps.push({ kind: "custom_values", values: customValues });
  }

  state.attachments.forEach((a, i) => {
    if (a.data !== null) steps.push({ kind: "attachment", attachmentIndex: i });
  });

  return steps;
}

/** استخراج شناسه معامله از رویداد ساخت معامله. */
export function tradeIdFromEvents(
  events: { event_type: string; payload: Record<string, unknown> }[],
): string | null {
  const created = events.find((e) => e.event_type === "domain.trade_created");
  if (!created) return null;
  const id = created.payload["trade_id"];
  return typeof id === "string" ? id : null;
}

// ---------- اجرای گام‌ها ----------

/** دستورهایی که شناسه معامله لازم دارند و پس از ساخت درج می‌شود. */
function needsTradeId(commandType: string): boolean {
  return (
    commandType === "domain.add_entry_leg" ||
    commandType === "domain.add_exit_leg" ||
    commandType === "domain.update_trade"
  );
}

/**
 * اجرای ترتیبی گام‌های ثبت روی پل.
 * به‌محض ساخت موفق معامله، `onTradeCreated` فراخوانی می‌شود تا فرم بتواند
 * شناسه را نگه دارد و شکست گام‌های بعدی به دوگیری منجر نشود.
 * شناسه معامله نهایی را برمی‌گرداند.
 */
export async function executeSubmitSteps(
  state: TradeFormState,
  bridge: TradeFormBridgeLike,
  onTradeCreated: (tradeId: string) => void,
): Promise<string> {
  let tradeId = state.createdTradeId ?? "";
  for (const step of buildSubmitSteps(state)) {
    switch (step.kind) {
      case "command": {
        const payload = needsTradeId(step.commandType)
          ? { ...step.payload, trade_id: tradeId }
          : step.payload;
        const events = await bridge.executeCommand(step.commandType, payload);
        if (step.commandType === "domain.create_trade") {
          tradeId = tradeIdFromEvents(events) ?? "";
          if (tradeId === "") {
            throw new Error("رویداد ساخت معامله شناسه معامله نداشت");
          }
          onTradeCreated(tradeId);
        }
        break;
      }
      case "custom_values":
        await bridge.setCustomValues(tradeId, step.values);
        break;
      case "attachment": {
        const a = state.attachments[step.attachmentIndex];
        if (a?.data) {
          await bridge.ingestAttachment({
            tradeId,
            fileName: a.fileName,
            mimeType: a.mimeType,
            data: a.data,
            linkKind: a.linkKind,
          });
        }
        break;
      }
    }
  }
  return tradeId;
}

/** حداقل سطح پل لازم برای اجرای گام‌ها — زیرمجموعه TradeFormBridge. */
export interface TradeFormBridgeLike {
  executeCommand(
    commandType: string,
    payload: Record<string, unknown>,
  ): Promise<{ event_type: string; payload: Record<string, unknown> }[]>;
  setCustomValues(tradeId: string, values: Record<string, unknown>): Promise<void>;
  ingestAttachment(args: {
    tradeId: string;
    fileName: string;
    mimeType: string | null;
    data: Uint8Array;
    linkKind: string;
  }): Promise<void>;
}
