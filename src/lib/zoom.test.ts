import { describe, expect, it } from "vitest";
import { nextZoom, normalizeZoom } from "./zoom";
describe("zoom", () => {
  it("uses a safe default for corrupt or old saved values", () => {
    for (const value of [0, 101, NaN, Infinity, -100])
      expect(normalizeZoom(value)).toBe(100);
  });
  it("steps through supported levels and stops at the limits", () => {
    expect(nextZoom(100, 1)).toBe(110);
    expect(nextZoom(100, -1)).toBe(90);
    expect(nextZoom(200, 1)).toBe(200);
    expect(nextZoom(75, -1)).toBe(75);
  });
});
