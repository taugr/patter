import { describe, expect, it, vi } from "vitest";
import cases from "../../test-fixtures/meeting-links.json";
const mocks = vi.hoisted(() => ({ invoke: vi.fn(), native: true }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("./storage", () => ({
  get native() {
    return mocks.native;
  },
}));
import { meetingLink, openMeetingLink } from "./meeting-links";

describe("meeting links", () => {
  it.each(cases)("validates $url", ({ url, supported }) => {
    expect(!!meetingLink(url)).toBe(supported);
  });
  it("preserves Zoom passcodes when opening via the native command", async () => {
    mocks.native = true;
    const url =
      "https://us02web.zoom.us/j/123456789?pwd=Encoded%2BPass&from=calendar";
    await openMeetingLink(url);
    expect(mocks.invoke).toHaveBeenLastCalledWith("open_meeting_link", { url });
  });
  it("does not dispatch an unsupported URL", async () => {
    mocks.invoke.mockClear();
    await expect(openMeetingLink("file:///tmp/meeting")).rejects.toThrow(
      "not a supported",
    );
    expect(mocks.invoke).not.toHaveBeenCalled();
  });
  it("keeps preview clicks local", async () => {
    mocks.native = false;
    mocks.invoke.mockClear();
    expect(
      await openMeetingLink("https://meet.google.com/abc-defg-hij"),
    ).toContain("Example Google Meet");
    expect(mocks.invoke).not.toHaveBeenCalled();
    mocks.native = true;
  });
});
