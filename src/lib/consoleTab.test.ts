import { beforeEach, describe, expect, it, vi } from "vitest";
import { loadTab, saveTab } from "./consoleTab";

function fakeStorage(): Storage {
  const map = new Map<string, string>();
  return {
    getItem: (k: string) => map.get(k) ?? null,
    setItem: (k: string, v: string) => void map.set(k, v),
    removeItem: (k: string) => void map.delete(k),
    clear: () => map.clear(),
    key: (i: number) => [...map.keys()][i] ?? null,
    get length() {
      return map.size;
    },
  } as Storage;
}

beforeEach(() => {
  vi.stubGlobal("localStorage", fakeStorage());
});

describe("consoleTab", () => {
  it("opens on Live when nothing has been stored", () => {
    expect(loadTab()).toBe("live");
  });

  it("round-trips a saved tab", () => {
    saveTab("plan");
    expect(loadTab()).toBe("plan");
    saveTab("live");
    expect(loadTab()).toBe("live");
  });

  it("falls back to Live when the stored value is not a known tab", () => {
    localStorage.setItem("console-tab", "songs");
    expect(loadTab()).toBe("live");
  });

  it("survives storage that throws (private mode / disabled storage)", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    } as unknown as Storage);
    expect(loadTab()).toBe("live");
    expect(() => saveTab("plan")).not.toThrow();
  });

  it("moves an operator who left the console on the old Prepare tab to Plan", () => {
    // "prepare" was split in two. Landing them on Plan rather than Setup is the
    // safer guess: the weekly job, not the once-a-machine one.
    localStorage.setItem("console-tab", "prepare");
    expect(loadTab()).toBe("plan");
  });

  it("still falls back to Live for anything unrecognised", () => {
    localStorage.setItem("console-tab", "nonsense");
    expect(loadTab()).toBe("live");
  });
});
