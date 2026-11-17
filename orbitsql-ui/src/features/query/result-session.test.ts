import { describe, expect, it } from "vitest";
import { virtualResultWindow } from "./result-session";

describe("result session presentation", () => {
  it("keeps the DOM window bounded for a large loaded result", () => {
    const window = virtualResultWindow(50_000, 32 * 25_000, 32 * 30);

    expect(window.start).toBe(24_992);
    expect(window.end - window.start).toBe(46);
  });

  it("clamps the virtual window to the loaded rows", () => {
    expect(virtualResultWindow(12, 0, 32 * 30)).toEqual({ start: 0, end: 12 });
  });
});
