import { describe, expect, it } from "vitest";
import { dropIndex, moveTo } from "./reorder";

const order = ["a", "b", "c", "d"];

describe("moving an item to a position", () => {
  it("moves upwards", () => {
    expect(moveTo(order, 2, 0)).toEqual(["c", "a", "b", "d"]);
  });

  it("moves downwards and lands where it was dropped", () => {
    // The one that catches naive implementations: removing first shifts every later
    // index up by one, so remove-then-insert-at-`to` lands a row short.
    expect(moveTo(order, 0, 2)).toEqual(["b", "c", "a", "d"]);
    expect(moveTo(order, 0, 3)).toEqual(["b", "c", "d", "a"]);
  });

  it("moves to the very end", () => {
    expect(moveTo(order, 1, 3)).toEqual(["a", "c", "d", "b"]);
  });

  it("does nothing when the item lands where it started", () => {
    expect(moveTo(order, 2, 2)).toEqual(order);
  });

  it("ignores indices outside the list", () => {
    // A drop can be released over nothing at all; that must not empty the order.
    expect(moveTo(order, -1, 2)).toEqual(order);
    expect(moveTo(order, 1, 9)).toEqual(order);
    expect(moveTo(order, 9, 1)).toEqual(order);
  });

  it("never mutates the array it was given", () => {
    const input = [...order];
    moveTo(input, 0, 3);
    expect(input).toEqual(order);
  });

  it("keeps every item", () => {
    // Losing a cue mid-service would be far worse than misplacing one.
    const out = moveTo(order, 3, 0);
    expect([...out].sort()).toEqual([...order].sort());
  });
});

describe("where a drop lands", () => {
  it("is the row it was released over", () => {
    expect(dropIndex(0, 2)).toBe(2);
  });

  it("is a no-op when released on itself", () => {
    expect(dropIndex(2, 2)).toBe(2);
  });
});
