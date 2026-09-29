//! پل فهرست/جزئیات — جداسازی فرم از IPC برای آزمون‌پذیری.

import type { AttachmentData, DomainEvent, PagedTrades, TradeDetails } from "../../kernel";
import type { SortKey } from "../../kernel";

/** پل موردنیاز فهرست معاملات و پنل جزئیات. */
export interface TradeListBridge {
  query(args: {
    filter: unknown;
    page: number;
    pageSize: number;
    sortKey?: SortKey | null;
    sortDesc?: boolean;
  }): Promise<PagedTrades>;
  details(tradeId: string): Promise<TradeDetails>;
  executeCommand(
    commandType: string,
    payload: Record<string, unknown>,
  ): Promise<DomainEvent[]>;
  ingestAttachment(args: {
    tradeId: string;
    fileName: string;
    mimeType: string | null;
    data: Uint8Array;
    linkKind: string;
  }): Promise<void>;
  /** بایت‌های پیوست برای پیش‌نمایش/بزرگ‌نمایی (فاز ۱.۱۳). */
  attachmentData(attachmentId: string, thumbnail: boolean): Promise<AttachmentData>;
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
}
