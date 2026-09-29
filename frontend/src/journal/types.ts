//! تایپ‌های ماژول ژورنال — قرارداد میان فرم، منطق خالص و پل کرنل.

/** پا ورودی — چند پا ورود و چند پا خروج. */
export interface EntryLegDraft {
  planned_price: string;
  executed_price: string;
  volume: string;
  stop_loss: string;
  take_profit: string;
  entry_time: string;
  note: string;
}

/** پا خروج. */
export interface ExitLegDraft {
  exit_reason: string;
  executed_price: string;
  volume: string;
  exit_time: string;
  note: string;
}

/** پیوست انتخاب‌شده در فرم — بایت‌ها فقط در حافظه نشسته می‌شوند. */
export interface PendingAttachment {
  fileName: string;
  mimeType: string | null;
  size: number;
  linkKind: string;
  data: Uint8Array | null;
}

/** وضعیت کامل فرم ثبت معامله. */
export interface TradeFormState {
  mode: "fast" | "full";
  accountId: string;
  symbolId: string;
  direction: "" | "buy" | "sell";
  status: "" | "open" | "closed" | "cancelled";
  strategy: string;
  timeframe: string;
  session: string;
  marketCondition: string;
  entryType: string;
  note: string;
  tags: string;
  emotions: string;
  mistakes: string;
  entryTime: string;
  commission: string;
  swap: string;
  /** پاهای ورود — حالت سریع دقیقاً یک پا دارد */
  entryLegs: EntryLegDraft[];
  exitLegs: ExitLegDraft[];
  /** مقادیر فیلدهای سفارشی بر اساس شناسه فیلد */
  customValues: Record<string, unknown>;
  /** پیوست‌های در انتظار ثبت پس از ساخت معامله */
  attachments: PendingAttachment[];
  /** شناسه معامله ساخته‌شده در تلاش ناتمام قبلی — برای ادامه بدون دوگیری */
  createdTradeId: string | null;
}

/** پاهای پیش‌فرض خالی. */
export function emptyEntryLeg(): EntryLegDraft {
  return {
    planned_price: "",
    executed_price: "",
    volume: "",
    stop_loss: "",
    take_profit: "",
    entry_time: "",
    note: "",
  };
}

/** پا خروج خالی. */
export function emptyExitLeg(): ExitLegDraft {
  return { exit_reason: "", executed_price: "", volume: "", exit_time: "", note: "" };
}

/** وضعیت خالی فرم — حالت سریع یک پای ورود دارد. */
export function emptyFormState(): TradeFormState {
  return {
    mode: "fast",
    accountId: "",
    symbolId: "",
    direction: "",
    status: "",
    strategy: "",
    timeframe: "",
    session: "",
    marketCondition: "",
    entryType: "",
    note: "",
    tags: "",
    emotions: "",
    mistakes: "",
    entryTime: "",
    commission: "",
    swap: "",
    entryLegs: [emptyEntryLeg()],
    exitLegs: [],
    customValues: {},
    attachments: [],
    createdTradeId: null,
  };
}

/**
 * پل فرم به کرنل — JournalPage پیاده‌سازی واقعی با kernel.ts را می‌دهد و
 * تست‌ها بدل (fake) تزریق می‌کنند؛ فرم مستقیم به import وابسته نیست.
 */
export interface TradeFormBridge {
  /** اجرای دستور دامنه — رویدادهای حاصل برمی‌گردد (برای استخراج trade_id). */
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
  listAccounts(): Promise<{ id: string; name: string; currency: string }[]>;
  listSymbols(): Promise<{ id: string; name: string }[]>;
  listFields(): Promise<
    {
      id: string;
      technical_key: string;
      display_label: string;
      storage_type: string;
      semantic_type: string;
      unit: string | null;
      required: boolean;
      display_order: number;
      form_group: string | null;
      validation_rules: {
        min?: number | null;
        max?: number | null;
        min_length?: number | null;
        max_length?: number | null;
      };
    }[]
  >;
  fieldOptions(fieldId: string): Promise<{ value: string; label: string }[]>;
}
