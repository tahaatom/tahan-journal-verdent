import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { clearDraft, hasDraft, loadDraft, saveDraft, DRAFT_VERSION } from "./draft";
import { emptyFormState } from "./types";

describe("draft autosave", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    window.localStorage.clear();
  });

  it("returns null when no draft exists", () => {
    expect(loadDraft()).toBeNull();
    expect(hasDraft()).toBe(false);
  });

  it("saves and recovers form state", () => {
    const state = emptyFormState();
    state.accountId = "acc-1";
    state.symbolId = "sym-1";
    state.direction = "buy";
    state.strategy = "برک‌اوت";
    state.customValues = { "f-1": 7 };
    saveDraft(state);
    expect(hasDraft()).toBe(true);
    const loaded = loadDraft();
    expect(loaded).not.toBeNull();
    expect(loaded!.accountId).toBe("acc-1");
    expect(loaded!.direction).toBe("buy");
    expect(loaded!.strategy).toBe("برک‌اوت");
    expect(loaded!.customValues["f-1"]).toBe(7);
    expect(loaded!.entryLegs).toHaveLength(1);
  });

  it("strips attachment bytes but keeps metadata", () => {
    const state = emptyFormState();
    state.attachments = [
      { fileName: "chart.png", mimeType: "image/png", size: 10, linkKind: "chart", data: new Uint8Array([1, 2, 3]) },
    ];
    saveDraft(state);
    const loaded = loadDraft();
    expect(loaded!.attachments).toHaveLength(1);
    expect(loaded!.attachments[0].fileName).toBe("chart.png");
    expect(loaded!.attachments[0].linkKind).toBe("chart");
    expect(loaded!.attachments[0].data).toBeNull();
  });

  it("clears the draft", () => {
    saveDraft(emptyFormState());
    clearDraft();
    expect(hasDraft()).toBe(false);
    expect(loadDraft()).toBeNull();
  });

  it("rejects drafts with a different version", () => {
    window.localStorage.setItem(
      "tahan.tradeDraft.v1",
      JSON.stringify({ version: DRAFT_VERSION + 1, savedAt: "", state: emptyFormState() }),
    );
    expect(loadDraft()).toBeNull();
  });

  it("rejects corrupt JSON silently", () => {
    window.localStorage.setItem("tahan.tradeDraft.v1", "{not json");
    expect(loadDraft()).toBeNull();
  });
});
