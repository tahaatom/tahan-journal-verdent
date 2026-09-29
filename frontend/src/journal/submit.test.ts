import { describe, expect, it, vi } from "vitest";
import {
  baseFormSchema,
  buildSubmitSteps,
  executeSubmitSteps,
  tradeIdFromEvents,
  validateForm,
  type TradeFormBridgeLike,
} from "./submit";
import { emptyEntryLeg, emptyExitLeg, emptyFormState } from "./types";

function fullValidState() {
  const s = emptyFormState();
  s.mode = "full";
  s.accountId = "acc-1";
  s.symbolId = "sym-1";
  s.direction = "buy";
  s.status = "closed";
  s.strategy = "روند";
  s.tags = "A+, آزمایشی";
  s.entryLegs = [
    { ...emptyEntryLeg(), volume: "0.5", executed_price: "100", stop_loss: "95", take_profit: "115" },
    { ...emptyEntryLeg(), volume: "0.3", executed_price: "101" },
  ];
  s.exitLegs = [{ ...emptyExitLeg(), executed_price: "110", volume: "0.8", exit_reason: "هدف" }];
  s.customValues = { "f-1": 7, "f-empty": "" };
  return s;
}

describe("validateForm", () => {
  it("accepts a valid fast state", () => {
    const s = emptyFormState();
    s.accountId = "acc-1";
    s.symbolId = "sym-1";
    s.direction = "sell";
    s.entryLegs[0].volume = "1";
    expect(validateForm(s, [])).toEqual({});
  });

  it("rejects missing base fields with Persian messages", () => {
    const errors = validateForm(emptyFormState(), []);
    expect(errors["accountId"]).toContain("حساب");
    expect(errors["symbolId"]).toContain("نماد");
    expect(errors["direction"]).toContain("جهت");
  });

  it("rejects non-positive leg volume", () => {
    const s = emptyFormState();
    s.accountId = "a";
    s.symbolId = "s";
    s.direction = "buy";
    s.entryLegs[0].volume = "-2";
    const errors = validateForm(s, []);
    expect(errors["entryLegs.0.volume"]).toContain("حجم");
  });

  it("rejects exit leg without executed price", () => {
    const s = emptyFormState();
    s.accountId = "a";
    s.symbolId = "s";
    s.direction = "buy";
    s.entryLegs[0].volume = "1";
    s.exitLegs = [{ ...emptyExitLeg(), volume: "1" }];
    const errors = validateForm(s, []);
    expect(errors["exitLegs.0.executed_price"]).toBeDefined();
  });

  it("enforces required custom fields", () => {
    const s = emptyFormState();
    s.accountId = "a";
    s.symbolId = "s";
    s.direction = "buy";
    s.entryLegs[0].volume = "1";
    const fields = [
      {
        id: "f-1",
        technical_key: "confidence",
        display_label: "اطمینان",
        storage_type: "integer",
        semantic_type: "number",
        unit: null,
        required: true,
        display_order: 0,
        form_group: null,
        validation_rules: { min: 1, max: 10 },
      },
    ];
    const errors = validateForm(s, fields);
    expect(errors["custom.f-1"]).toContain("الزامی");
    s.customValues["f-1"] = 11;
    expect(validateForm(s, fields)["custom.f-1"]).toContain("بیشتر");
    s.customValues["f-1"] = 5;
    expect(validateForm(s, fields)).toEqual({});
  });

  it("base schema parses negative commission as invalid", () => {
    expect(baseFormSchema.safeParse({ accountId: "a", symbolId: "s", direction: "buy", commission: "-1", swap: "", entryTime: "" }).success).toBe(false);
  });
});

describe("buildSubmitSteps", () => {
  it("builds create + legs + status + custom values in order (full mode)", () => {
    const steps = buildSubmitSteps(fullValidState());
    expect(steps[0]).toMatchObject({ kind: "command", commandType: "domain.create_trade" });
    const commands = steps.filter((s) => s.kind === "command") as { commandType: string }[];
    expect(commands.map((c) => c.commandType)).toEqual([
      "domain.create_trade",
      "domain.add_entry_leg",
      "domain.add_entry_leg",
      "domain.add_exit_leg",
      "domain.update_trade",
    ]);
    // مقادیر سفارشی خالی حذف می‌شوند
    const cv = steps.find((s) => s.kind === "custom_values") as { values: Record<string, unknown> };
    expect(cv.values).toEqual({ "f-1": 7 });
  });

  it("skips create_trade when resuming with createdTradeId", () => {
    const s = fullValidState();
    s.createdTradeId = "t-42";
    const steps = buildSubmitSteps(s);
    expect(steps.some((st) => st.kind === "command" && st.commandType === "domain.create_trade")).toBe(false);
  });

  it("fast mode sends exactly create + one entry leg", () => {
    const s = emptyFormState();
    s.accountId = "a";
    s.symbolId = "s";
    s.direction = "buy";
    s.entryLegs[0] = { ...emptyEntryLeg(), volume: "1", executed_price: "100" };
    const steps = buildSubmitSteps(s);
    expect(steps).toHaveLength(2);
    const create = steps[0] as { payload: Record<string, unknown> };
    expect(create.payload["initial_stop_loss"]).toBeNull();
  });

  it("carries SL/TP of first leg into create payload", () => {
    const s = fullValidState();
    const steps = buildSubmitSteps(s);
    const create = steps[0] as { payload: Record<string, unknown> };
    expect(create.payload["initial_stop_loss"]).toBe(95);
    expect(create.payload["take_profit"]).toBe(115);
    expect(create.payload["tags"]).toBe("A+, آزمایشی");
  });

  it("queues only attachments with bytes", () => {
    const s = fullValidState();
    s.attachments = [
      { fileName: "a.png", mimeType: null, size: 1, linkKind: "chart", data: new Uint8Array([1]) },
      { fileName: "b.png", mimeType: null, size: 1, linkKind: "chart", data: null },
    ];
    const steps = buildSubmitSteps(s);
    const atts = steps.filter((st) => st.kind === "attachment") as { attachmentIndex: number }[];
    expect(atts).toEqual([{ kind: "attachment", attachmentIndex: 0 }]);
  });
});

describe("tradeIdFromEvents", () => {
  it("extracts trade id from create event", () => {
    expect(
      tradeIdFromEvents([{ event_type: "domain.trade_created", payload: { trade_id: "t-9" } }]),
    ).toBe("t-9");
  });

  it("returns null otherwise", () => {
    expect(tradeIdFromEvents([{ event_type: "domain.other", payload: {} }])).toBeNull();
  });
});

describe("executeSubmitSteps", () => {
  function fakeBridge() {
    let counter = 0;
    const executed: { commandType: string; payload: Record<string, unknown> }[] = [];
    const custom: { tradeId: string; values: Record<string, unknown> }[] = [];
    const ingested: { tradeId: string; fileName: string }[] = [];
    const bridge: TradeFormBridgeLike = {
      executeCommand: async (commandType, payload) => {
        executed.push({ commandType, payload });
        if (commandType === "domain.create_trade") {
          counter += 1;
          return [{ event_type: "domain.trade_created", payload: { trade_id: `t-${counter}` } }];
        }
        return [];
      },
      setCustomValues: async (tradeId, values) => {
        custom.push({ tradeId, values });
      },
      ingestAttachment: async (args) => {
        ingested.push({ tradeId: args.tradeId, fileName: args.fileName });
      },
    };
    return { bridge, executed, custom, ingested };
  }

  it("injects trade_id into leg commands and runs all steps", async () => {
    const s = fullValidState();
    const { bridge, executed, custom } = fakeBridge();
    const tradeId = await executeSubmitSteps(s, bridge, () => {});
    expect(tradeId).toBe("t-1");
    const legCmds = executed.filter((e) => e.commandType === "domain.add_entry_leg");
    expect(legCmds).toHaveLength(2);
    expect(legCmds[0].payload["trade_id"]).toBe("t-1");
    expect(legCmds[0].payload["volume"]).toBe(0.5);
    expect(executed.find((e) => e.commandType === "domain.update_trade")?.payload["trade_id"]).toBe("t-1");
    expect(custom[0].tradeId).toBe("t-1");
  });

  it("notifies onTradeCreated immediately after creation", async () => {
    const s = fullValidState();
    const { bridge } = fakeBridge();
    const seen: string[] = [];
    await executeSubmitSteps(s, bridge, (id) => seen.push(id));
    expect(seen).toEqual(["t-1"]);
  });

  it("resumes from createdTradeId without duplicating the trade", async () => {
    const s = fullValidState();
    s.createdTradeId = "t-42";
    const { bridge, executed } = fakeBridge();
    const tradeId = await executeSubmitSteps(s, bridge, () => {});
    expect(tradeId).toBe("t-42");
    expect(executed.some((e) => e.commandType === "domain.create_trade")).toBe(false);
    expect(executed[0].commandType).toBe("domain.add_entry_leg");
    expect(executed[0].payload["trade_id"]).toBe("t-42");
  });

  it("ingests pending attachments bound to the new trade", async () => {
    const s = fullValidState();
    s.attachments = [
      { fileName: "chart.png", mimeType: "image/png", size: 3, linkKind: "chart", data: new Uint8Array([9]) },
    ];
    const { bridge, ingested } = fakeBridge();
    await executeSubmitSteps(s, bridge, () => {});
    expect(ingested).toEqual([{ tradeId: "t-1", fileName: "chart.png" }]);
  });

  it("propagates bridge failure after trade creation", async () => {
    const s = fullValidState();
    const { bridge } = fakeBridge();
    vi.spyOn(bridge, "setCustomValues").mockRejectedValue(new Error("۱۳۰۳"));
    await expect(executeSubmitSteps(s, bridge, () => {})).rejects.toThrow("۱۳۰۳");
  });
});
