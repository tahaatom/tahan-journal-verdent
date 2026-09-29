//! جدول فهرست معاملات — صفحه‌بندی و مرتب‌سازی سمت کرنل با مجازی‌سازی ردیف‌ها.
//!
//! مرتب‌سازی فقط روی کلیدهای مجاز کرنل (`SORT_KEYS`) فعال می‌شود؛
//! ستون‌های دیگر قابلیت مرتب‌سازی ندارند تا هرگز پرس‌وجوی نامعتبر
//! ساخته نشود. ردیف‌ها با virtualizer مجازی‌سازی می‌شوند تا فهرست‌های
//! طولانی صفحه سبک بماند.

import { useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { PagedTrades, SortKey, TradeListRow } from "../../kernel";

const ROW_HEIGHT = 40;

const columnHelper = createColumnHelper<TradeListRow>();

function fmtTime(v: string | null): string {
  return v ? v.slice(0, 16).replace("T", " ") : "—";
}

function fmtNum(v: number | null, digits = 2): string {
  return v === null ? "—" : v.toFixed(digits);
}

interface TradeTableProps {
  data: PagedTrades;
  sortKey: SortKey | null;
  sortDesc: boolean;
  onSortChange: (key: SortKey, desc: boolean) => void;
  onPageChange: (page: number) => void;
  onPageSizeChange: (size: number) => void;
  onRowOpen: (tradeId: string) => void;
  loading?: boolean;
}

export function TradeTable({
  data,
  sortKey,
  sortDesc,
  onSortChange,
  onPageChange,
  onPageSizeChange,
  onRowOpen,
  loading,
}: TradeTableProps) {
  const { t } = useTranslation();
  const parentRef = useRef<HTMLDivElement>(null);

  const columns = [
    columnHelper.accessor("entry_time", {
      id: "entry_time",
      header: t("list.colEntryTime"),
      cell: (info) => fmtTime(info.getValue()),
      enableSorting: true,
    }),
    columnHelper.accessor("symbol_id", {
      id: "symbol_id",
      header: t("list.colSymbol"),
      enableSorting: false,
    }),
    columnHelper.accessor("direction", {
      id: "direction",
      header: t("list.colDirection"),
      enableSorting: false,
      cell: (info) => t(`journal.${info.getValue()}`),
    }),
    columnHelper.accessor("status", {
      id: "status",
      header: t("list.colStatus"),
      enableSorting: false,
      cell: (info) => t(`journal.status${info.getValue() === "open" ? "Open" : info.getValue() === "closed" ? "Closed" : "Cancelled"}`),
    }),
    columnHelper.accessor("strategy", {
      id: "strategy",
      header: t("list.colStrategy"),
      enableSorting: false,
      cell: (info) => info.getValue() ?? "—",
    }),
    columnHelper.accessor("realized_pnl", {
      id: "realized_pnl",
      header: t("list.colPnl"),
      enableSorting: true,
      cell: (info) => (
        <span
          className={
            info.getValue() === null
              ? ""
              : info.getValue()! >= 0
                ? "text-green-600 dark:text-green-400"
                : "text-red-500"
          }
        >
          {fmtNum(info.getValue())}
        </span>
      ),
    }),
    columnHelper.accessor("realized_r", {
      id: "realized_r",
      header: t("list.colR"),
      enableSorting: true,
      cell: (info) => (info.getValue() === null ? "—" : `${fmtNum(info.getValue())}R`),
    }),
    columnHelper.display({
      id: "actions",
      header: "",
      cell: (info) => (
        <button
          type="button"
          data-testid={`open-trade-${info.row.original.id}`}
          onClick={() => onRowOpen(info.row.original.id)}
          className="rounded border border-border px-2 py-0.5 text-xs hover:bg-accent-soft"
        >
          {t("list.details")}
        </button>
      ),
    }),
  ];

  const table = useReactTable({
    data: data.items,
    columns,
    getCoreRowModel: getCoreRowModel(),
    manualPagination: true,
    manualSorting: true,
  });

  const rowModel = table.getRowModel().rows;
  const virtualizer = useVirtualizer({
    count: rowModel.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
  });

  const totalPages = Math.max(1, Math.ceil(data.total / data.page_size));

  // وقتی ظرف اندازه‌گیری ندارد (مثلاً در محیط تست) مجازی‌سازی غیرفعال می‌شود
  const visibleRows: { row: (typeof rowModel)[number]; offset: number | null }[] =
    virtualizer.getVirtualItems().length > 0
      ? virtualizer
          .getVirtualItems()
          .map((vi) => ({ row: rowModel[vi.index], offset: vi.start }))
      : rowModel.map((row) => ({ row, offset: null }));

  const handleSort = (key: string, sortable: boolean) => {
    if (!sortable) return;
    if (sortKey === key) {
      onSortChange(key as SortKey, !sortDesc);
    } else {
      onSortChange(key as SortKey, true);
    }
  };

  return (
    <div dir="rtl" className="flex flex-col gap-2 text-sm">
      <div
        ref={parentRef}
        data-testid="trade-table-scroll"
        className="max-h-96 overflow-auto rounded border border-border"
      >
        <table className="w-full min-w-max border-collapse">
          <thead className="sticky top-0 z-10 bg-surface-alt">
            {table.getHeaderGroups().map((hg) => (
              <tr key={hg.id}>
                {hg.headers.map((header) => {
                  const sortable = header.column.getCanSort();
                  const isSorted = sortKey === header.column.id;
                  return (
                    <th
                      key={header.id}
                      data-testid={`th-${header.id}`}
                      onClick={() => handleSort(header.column.id, sortable)}
                      className={`border-b border-border px-3 py-2 text-right text-xs font-bold ${
                        sortable ? "cursor-pointer select-none hover:bg-accent-soft" : ""
                      }`}
                    >
                      {flexRender(header.column.columnDef.header, header.getContext())}
                      {isSorted && <span className="mr-1">{sortDesc ? "▼" : "▲"}</span>}
                    </th>
                  );
                })}
              </tr>
            ))}
          </thead>
          <tbody>
            {visibleRows.length > 0 ? (
              visibleRows.map(({ row, offset }) => (
                <tr
                  key={row.id}
                  data-testid={`trade-row-${row.original.id}`}
                  style={{ height: ROW_HEIGHT, ...(offset !== null ? { transform: `translateY(${offset}px)` } : {}) }}
                  className="odd:bg-surface-alt/40"
                >
                  {row.getVisibleCells().map((cell) => (
                    <td key={cell.id} className="border-b border-border/50 px-3 py-1 text-xs">
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </td>
                  ))}
                </tr>
              ))
            ) : (
              <tr>
                <td colSpan={columns.length} className="px-3 py-8 text-center text-xs text-text-muted">
                  {loading ? t("list.loading") : t("list.empty")}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* صفحه‌بندی */}
      <div className="flex items-center justify-between text-xs" data-testid="trade-pagination">
        <span className="text-text-muted">
          {t("list.totalCount", { count: data.total })}
        </span>
        <span className="flex items-center gap-2">
          <select
            data-testid="page-size"
            value={data.page_size}
            onChange={(e) => onPageSizeChange(Number(e.target.value))}
            className="rounded border border-border bg-surface px-1 py-0.5"
          >
            {[25, 50, 100, 200].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
          <button
            type="button"
            data-testid="page-prev"
            disabled={data.page <= 1}
            onClick={() => onPageChange(data.page - 1)}
            className="rounded border border-border px-2 py-0.5 disabled:opacity-40"
          >
            {t("list.prev")}
          </button>
          <span data-testid="page-info">
            {t("list.pageOf", { page: data.page, total: totalPages })}
          </span>
          <button
            type="button"
            data-testid="page-next"
            disabled={data.page >= totalPages}
            onClick={() => onPageChange(data.page + 1)}
            className="rounded border border-border px-2 py-0.5 disabled:opacity-40"
          >
            {t("list.next")}
          </button>
        </span>
      </div>
    </div>
  );
}
