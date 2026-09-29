import { describe, expect, it } from "vitest";
import {
  buildFilterNode,
  emptyFilterState,
  isEmptyFilter,
  typedCustomValue,
  validateFilterState,
  type FilterState,
} from "./filterState";

const intField = {
  id: "f-1",
  technical_key: "confidence",
  display_label: "اطمینان",
  storage_type: "integer",
  semantic_type: "number",
  unit: null,
  required: false,
  display_order: 0,
  form_group: null,
  validation_rules: {},
};

const textField = {
  id: "f-2",
  technical_key: "setup",
  display_label: "ستاپ",
  storage_type: "text",
  semantic_type: "text",
  unit: null,
  required: false,
  display_order: 1,
  form_group: null,
  validation_rules: {},
};

describe("typedCustomValue", () => {
  it("builds integer values strictly", () => {
    expect(typedCustomValue(intField, "7")).toEqual({ kind: "integer", value: 7 });
    // اعشار برای integer نامعتبر است
    expect(typedCustomValue(intField, "7.5")).toBeNull();
    expect(typedCustomValue(intField, "abc")).toBeNull();
  });

  it("builds boolean values from Persian and English", () => {
    const b = { storage_type: "boolean" };
    expect(typedCustomValue(b, "بله")).toEqual({ kind: "boolean", value: true });
    expect(typedCustomValue(b, "false")).toEqual({ kind: "boolean", value: false });
    expect(typedCustomValue(b, "شاید")).toBeNull();
  });

  it("treats text as-is and trims", () => {
    expect(typedCustomValue(textField, "  برک‌اوت ")).toEqual({ kind: "text", value: "برک‌اوت" });
  });
});

describe("buildFilterNode", () => {
  it("returns empty all-group for empty state", () => {
    expect(buildFilterNode(emptyFilterState(), [])).toEqual({ type: "all", children: [] });
  });

  it("returns simple node for only simple conditions", () => {
    const s = emptyFilterState();
    s.direction = "buy";
    s.status = "closed";
    s.entryFrom = "2026-01-01";
    const node = buildFilterNode(s, []) as Record<string, unknown>;
    expect(node["type"]).toBe("simple");
    expect(node["direction"]).toBe("buy");
    expect(node["status"]).toBe("closed");
    expect(node["entry_from"]).toBe("2026-01-01");
  });

  it("combines simple and custom under all-group in AND mode", () => {
    const s = emptyFilterState();
    s.direction = "buy";
    s.customRows = [{ fieldKey: "confidence", op: "min", value: "70" }];
    const node = buildFilterNode(s, [intField]);
    expect(node["type"]).toBe("all");
    const children = node["children"] as Record<string, unknown>[];
    expect(children).toHaveLength(2);
    expect(children[0]["type"]).toBe("simple");
    expect(children[1]).toEqual({
      type: "custom",
      field_key: "confidence",
      op: { op: "min", value: { kind: "integer", value: 70 } },
    });
  });

  it("combines under any-group in OR mode", () => {
    const s = emptyFilterState();
    s.direction = "sell";
    s.customRows = [{ fieldKey: "confidence", op: "min", value: "70" }];
    s.combine = "or";
    expect(buildFilterNode(s, [intField])["type"]).toBe("any");
  });

  it("drops invalid custom rows but keeps valid ones", () => {
    const s = emptyFilterState();
    s.customRows = [
      { fieldKey: "confidence", op: "min", value: "abc" }, // نامعتبر → حذف
      { fieldKey: "setup", op: "equals", value: "برک‌اوت" }, // معتبر
      { fieldKey: "confidence", op: "exists", value: "" }, // exists بدون مقدار
    ];
    const node = buildFilterNode(s, [intField, textField]);
    const children = node["children"] as Record<string, unknown>[];
    expect(children).toHaveLength(3); // ساده + ۲ شرط معتبر
    expect(children[1]).toEqual({
      type: "custom",
      field_key: "setup",
      op: { op: "equals", value: { kind: "text", value: "برک‌اوت" } },
    });
    expect(children[2]).toEqual({
      type: "custom",
      field_key: "confidence",
      op: { op: "exists" },
    });
  });
});

describe("validateFilterState", () => {
  it("flags invalid values by row index", () => {
    const s: FilterState = emptyFilterState();
    s.customRows = [
      { fieldKey: "confidence", op: "min", value: "x" },
      { fieldKey: "setup", op: "contains", value: "ok" },
      { fieldKey: "confidence", op: "exists", value: "" },
    ];
    const errors = validateFilterState(s, [intField, textField]);
    expect(Object.keys(errors)).toEqual(["0"]);
    expect(errors["0"]).toContain("اطمینان");
  });
});

describe("isEmptyFilter", () => {
  it("detects any active condition", () => {
    expect(isEmptyFilter(emptyFilterState())).toBe(true);
    const s = emptyFilterState();
    s.search = "x";
    expect(isEmptyFilter(s)).toBe(false);
  });
});
