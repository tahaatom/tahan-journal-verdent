/** نمایش تاریخ جلالی از رشته ISO — تقویم فارسی استاندارد ICU. */
export function toJalali(iso: string): string {
  if (!/^\d{4}-\d{2}-\d{2}/.test(iso)) return iso;
  try {
    return new Intl.DateTimeFormat("fa-IR-u-ca-persian", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      timeZone: "UTC",
    }).format(new Date(iso.slice(0, 10) + "T00:00:00Z"));
  } catch {
    return iso;
  }
}
