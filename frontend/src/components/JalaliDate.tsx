import { useMemo } from "react";
import { toJalali } from "../utils/jalali";

/** برچسب تاریخ جلالی با اعداد فارسی — برای ستون‌های تاریخ در سراسر برنامه. */
export function JalaliDate({ iso }: { iso: string }) {
  const label = useMemo(() => toJalali(iso), [iso]);
  return (
    <span dir="rtl" className="inline-block tabular-nums">
      {label}
    </span>
  );
}
