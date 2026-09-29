import { describe, expect, it } from "vitest";
import { plannedR } from "./plannedR";

describe("plannedR", () => {
  it("computes R for a buy trade", () => {
    // ورود ۱۰۰، SL ۹۵، TP ۱۱۵ → ریسک ۵، پاداش ۱۵ → R=3
    const r = plannedR("100", "95", "115", "buy");
    expect(r.value).toBe(3);
    expect(r.invalid).toBeNull();
    expect(r.slMissing).toBe(false);
  });

  it("computes R for a sell trade", () => {
    // ورود ۱۰۰، SL ۱۰۵، TP ۹۰ → ریسک ۵، پاداش ۱۰ → R=2
    const r = plannedR("100", "105", "90", "sell");
    expect(r.value).toBe(2);
  });

  it("reports slMissing when stop loss is empty", () => {
    const r = plannedR("100", "", "115", "buy");
    expect(r.value).toBeNull();
    expect(r.slMissing).toBe(true);
  });

  it("flags inverted stop loss for buy", () => {
    const r = plannedR("100", "110", "115", "buy");
    expect(r.value).toBeNull();
    expect(r.invalid).toContain("حد ضرر");
  });

  it("flags inverted take profit for sell", () => {
    const r = plannedR("100", "105", "120", "sell");
    expect(r.value).toBeNull();
    expect(r.invalid).toContain("حد سود");
  });

  it("returns null when direction is unknown", () => {
    expect(plannedR("100", "95", "115", "").value).toBeNull();
  });

  it("rejects non-positive or non-numeric inputs", () => {
    expect(plannedR("abc", "95", "115", "buy").value).toBeNull();
    expect(plannedR("-10", "95", "115", "buy").value).toBeNull();
    expect(plannedR("0", "95", "115", "buy").value).toBeNull();
  });

  it("rounds to two decimals", () => {
    // ریسک ۳، پاداش ۱۰ → R=3.333… → 3.33
    const r = plannedR("100", "97", "110", "buy");
    expect(r.value).toBe(3.33);
  });
});
