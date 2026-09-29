import { describe, expect, it } from "vitest";
import { calendarPermissionText } from "./permissions";
describe("calendar permission recovery", () => {
  it("requests permission before directing a first-time user to the macOS list", () => {
    expect(calendarPermissionText("not_requested")).toContain(
      "Connect calendar",
    );
    expect(calendarPermissionText("not_requested")).toContain(
      "after this request",
    );
    expect(calendarPermissionText("denied")).toContain(
      "Open Calendar permissions",
    );
    expect(calendarPermissionText("write_only")).toContain("Connect calendar");
  });
  it("does not claim restricted or unpackaged builds can grant access", () => {
    expect(calendarPermissionText("restricted")).toContain("administrator");
    expect(calendarPermissionText("unavailable")).toContain(
      "packaged Patter app",
    );
  });
});
