import { describe, expect, it } from "vitest";
import { toJalali } from "./jalali";

describe("toJalali", () => {
  it("converts ISO dates to Persian calendar", () => {
    // 2026-03-21 میلادی = ۱ فروردین ۱۴۰۵
    expect(toJalali("2026-03-21")).toMatch(/۱۴۰۵/);
    expect(toJalali("2026-03-21")).toMatch(/فروردین|۰۱|1/);
  });

  it("passes through invalid input unchanged", () => {
    expect(toJalali("")).toBe("");
    expect(toJalali("not-a-date")).toBe("not-a-date");
  });
});
