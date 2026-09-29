//! صفحه فهرست معاملات — اتصال فیلتر، جدول و پنل جزئیات به پل کرنل.
//!
//! تغییرات پنل فیلتر محلی می‌مانند و فقط با «اعمال» پرس‌وجوی سمت کرنل
//! می‌سازند؛ صفحه‌بندی و مرتب‌سازی نیز کاملاً سمت کرنل انجام می‌شود.

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  accountsList,
  attachmentData,
  attachmentIngest,
  domainExecute,
  schemaListFields,
  symbolsList,
  queryTrades,
  tradeDetails,
  type PagedTrades,
  type SortKey,
} from "../../kernel";
import type { CustomFieldDefLike } from "../fieldTypes";
import { buildFilterNode, emptyFilterState, type FilterState } from "./filterState";
import type { TradeListBridge } from "./bridge";
import { invalidateStats } from "../../queryClient";
import { TradeFilters } from "./TradeFilters";
import { TradeTable } from "./TradeTable";
import { TradeDetailsPanel } from "./TradeDetailsPanel";

/** پیاده‌سازی پل فهرست روی دستورات IPC کرنل. */
function useKernelListBridge(): TradeListBridge {
  return useMemo(
    () => ({
      query: (args) => queryTrades(args),
      details: (tradeId) => tradeDetails(tradeId),
      executeCommand: (commandType, payload) => domainExecute(commandType, payload),
      ingestAttachment: (args) => attachmentIngest(args).then(() => undefined),
      attachmentData: (attachmentId, thumbnail) => attachmentData(attachmentId, thumbnail),
      listAccounts: () =>
        accountsList().then((xs) =>
          xs.map((a) => ({ id: a.id, name: a.name, currency: a.currency })),
        ),
      listSymbols: () =>
        symbolsList().then((xs) => xs.map((s) => ({ id: s.id, name: s.name }))),
      listFields: () =>
        schemaListFields().then((fs) =>
          fs.map((f) => ({
            id: f.id,
            technical_key: f.technical_key,
            display_label: f.display_label,
            storage_type: f.storage_type,
            semantic_type: f.semantic_type,
            unit: f.unit,
            required: f.required,
            display_order: f.display_order,
            form_group: f.form_group,
            validation_rules: f.validation_rules ?? {},
          })),
        ),
    }),
    [],
  );
}

interface TradeListPageProps {
  /** پل تزریقی (تست) — در نبود آن از پل کرنل استفاده می‌شود. */
  bridge?: TradeListBridge;
  /** تغییر این عدد فهرست را دوباره می‌خواند (مثلاً پس از ثبت معامله جدید). */
  refreshSignal?: number;
  /** پس از هر تغییر داده (حذف/ویرایش) صدا زده می‌شود — برای همگام‌سازی داشبورد. */
  onChanged?: () => void;
}

export function TradeListPage({ bridge: bridgeProp, refreshSignal = 0, onChanged }: TradeListPageProps) {
  const { t } = useTranslation();
  const kernelBridge = useKernelListBridge();
  const bridge = bridgeProp ?? kernelBridge;

  const [accounts, setAccounts] = useState<{ id: string; name: string; currency: string }[]>([]);
  const [symbols, setSymbols] = useState<{ id: string; name: string }[]>([]);
  const [fields, setFields] = useState<CustomFieldDefLike[]>([]);

  const [draft, setDraft] = useState<FilterState>(emptyFilterState);
  const [applied, setApplied] = useState<FilterState>(emptyFilterState);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(25);
  const [sortKey, setSortKey] = useState<SortKey | null>(null);
  const [sortDesc, setSortDesc] = useState(true);
  const [data, setData] = useState<PagedTrades | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reloadTick, setReloadTick] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // متادیتای انتخابگرها و فیلدهای سفارشی — یک‌بار
  useEffect(() => {
    let alive = true;
    bridge
      .listAccounts()
      .then((xs) => alive && setAccounts(xs))
      .catch(() => undefined);
    bridge
      .listSymbols()
      .then((xs) => alive && setSymbols(xs))
      .catch(() => undefined);
    bridge
      .listFields()
      .then((fs) => alive && setFields(fs))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [bridge]);

  // پرس‌وجوی فهرست — فقط وقتی فیلتر اعمال‌شده/صفحه/مرتب‌سازی عوض شود
  const appliedNode = useMemo(
    () => buildFilterNode(applied, fields),
    [applied, fields],
  );
  useEffect(() => {
    let alive = true;
    setLoading(true);
    bridge
      .query({ filter: appliedNode, page, pageSize, sortKey, sortDesc })
      .then((d) => {
        if (!alive) return;
        setData(d);
        setError(null);
      })
      .catch((e) => alive && setError(String(e?.message ?? e)))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, [bridge, appliedNode, page, pageSize, sortKey, sortDesc, reloadTick, refreshSignal]);

  const applyFilters = useCallback(() => {
    setApplied(draft);
    setPage(1);
  }, [draft]);

  const resetFilters = useCallback(() => {
    const empty = emptyFilterState();
    setDraft(empty);
    setApplied(empty);
    setPage(1);
  }, []);

  const onSortChange = useCallback((key: SortKey, desc: boolean) => {
    setSortKey(key);
    setSortDesc(desc);
    setPage(1);
  }, []);

  const onPageSizeChange = useCallback((size: number) => {
    setPageSize(size);
    setPage(1);
  }, []);

  const handleDeleted = useCallback(() => {
    setSelectedId(null);
    setReloadTick((n) => n + 1);
    invalidateStats();
    onChanged?.();
  }, [onChanged]);

  return (
    <div data-testid="trade-list-page" dir="rtl" className="flex flex-col gap-3">
      <TradeFilters
        state={draft}
        onChange={setDraft}
        accounts={accounts}
        symbols={symbols}
        fields={fields}
        onApply={applyFilters}
        onReset={resetFilters}
      />

      {error && (
        <p role="alert" className="rounded bg-red-100 px-3 py-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200">
          {t("list.errorLoad")} — {error}
        </p>
      )}

      {data && (
        <TradeTable
          data={data}
          sortKey={sortKey}
          sortDesc={sortDesc}
          onSortChange={onSortChange}
          onPageChange={setPage}
          onPageSizeChange={onPageSizeChange}
          onRowOpen={setSelectedId}
          loading={loading}
        />
      )}

      {selectedId && (
        <TradeDetailsPanel
          tradeId={selectedId}
          bridge={bridge}
          onClose={() => setSelectedId(null)}
          onDeleted={handleDeleted}
        />
      )}
    </div>
  );
}
