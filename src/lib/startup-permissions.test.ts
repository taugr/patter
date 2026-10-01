import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { accessService, missingAccess } from "./startup-permissions";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const call = vi.mocked(invoke);
beforeEach(() => call.mockReset());

describe("launch permission policy", () => {
  it("only shows missing access, with optional features omitted", () => {
    expect(
      missingAccess({ microphone: "allowed", systemAudio: "managed_by_macos" }),
    ).toEqual([]);
    expect(
      missingAccess({
        microphone: "not_requested",
        systemAudio: "managed_by_macos",
      }).map((r) => [r.id, r.action]),
    ).toEqual([["microphone", "allow"]]);
  });
  it("sends denied permissions to settings and does not request restricted access", () => {
    expect(
      missingAccess({
        microphone: "denied",
        systemAudio: "managed_by_macos",
        calendar: "restricted",
        notifications: "blocked",
      }).map((r) => [r.id, r.action]),
    ).toEqual([
      ["microphone", "settings"],
      ["calendar", "restricted"],
      ["notifications", "settings"],
    ]);
  });
  it("requests full calendar access when write-only access is insufficient", () => {
    expect(
      missingAccess({
        microphone: "allowed",
        systemAudio: "managed_by_macos",
        calendar: "write_only",
        notifications: "not requested",
      }).map((r) => r.action),
    ).toEqual(["allow", "allow"]);
  });
  it("does not report unknown status as granted", () => {
    expect(
      missingAccess({
        microphone: "unknown",
        systemAudio: "managed_by_macos",
      })[0].action,
    ).toBe("check");
  });
});

describe("native permission bridge", () => {
  it("preflights recording without prompting or querying disabled features", async () => {
    call.mockResolvedValue({
      microphone: "allowed",
      systemAudio: "managed_by_macos",
    });
    await accessService(false, false).check();
    expect(call.mock.calls).toEqual([["recording_permissions"]]);
  });
  it("does not inspect optional permissions in an unpackaged build", async () => {
    call.mockResolvedValue(null);
    expect(await accessService(true, true).check()).toBeNull();
    expect(call.mock.calls).toEqual([["recording_permissions"]]);
  });
  it("includes enabled features, but does not request consent during checks", async () => {
    call.mockImplementation(async (command) =>
      command === "recording_permissions"
        ? { microphone: "denied", systemAudio: "managed_by_macos" }
        : command === "calendar_permission"
          ? "denied"
          : "blocked",
    );
    expect(await accessService(true, true).check()).toEqual({
      microphone: "denied",
      systemAudio: "managed_by_macos",
      calendar: "denied",
      notifications: "blocked",
    });
    expect(call.mock.calls.map((args) => args[0])).toEqual([
      "recording_permissions",
      "calendar_permission",
      "notification_permission",
    ]);
  });
  it("shares concurrent checks and allows retry after a failed check", async () => {
    const service = accessService(false, false);
    call.mockRejectedValueOnce(new Error("unavailable"));
    const first = service.check();
    expect(service.check()).toBe(first);
    await expect(first).rejects.toThrow("unavailable");
    call.mockResolvedValue({
      microphone: "allowed",
      systemAudio: "managed_by_macos",
    });
    await expect(service.check()).resolves.toMatchObject({
      microphone: "allowed",
    });
    expect(call).toHaveBeenCalledTimes(2);
  });
  it("waits for preflight to finish before requesting capture consent", async () => {
    let finish!: (value: unknown) => void;
    call.mockReturnValueOnce(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    const service = accessService(false, false);
    const check = service.check();
    const allow = service.allow("microphone");
    expect(call).toHaveBeenCalledTimes(1);
    finish({ microphone: "not_requested", systemAudio: "managed_by_macos" });
    await check;
    await allow;
    expect(call).toHaveBeenLastCalledWith("request_capture_permission", {
      kind: "microphone",
    });
  });
  it("shares in-flight reads across views, then refreshes changed macOS status", async () => {
    let finish!: (value: unknown) => void;
    call.mockReturnValueOnce(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    const startup = accessService(false, false);
    const settings = accessService(false, false);
    const reads = [startup.check(), settings.check()];
    expect(call).toHaveBeenCalledTimes(1);
    finish({ microphone: "denied", systemAudio: "managed_by_macos" });
    await Promise.all(reads);
    call.mockResolvedValueOnce({
      microphone: "allowed",
      systemAudio: "managed_by_macos",
    });
    expect(await settings.check()).toMatchObject({ microphone: "allowed" });
    expect(call).toHaveBeenCalledTimes(2);
  });
  it("serializes consent and refresh across views and permits retry after denial", async () => {
    let reject!: (error: Error) => void;
    call.mockReturnValueOnce(
      new Promise((_, fail) => {
        reject = fail;
      }),
    );
    const startup = accessService(false, false);
    const settings = accessService(false, false);
    const consent = startup.allow("microphone");
    await vi.waitFor(() => expect(call).toHaveBeenCalledTimes(1));
    const read = settings.check();
    await expect(settings.allow("microphone")).rejects.toThrow("already open");
    expect(call).toHaveBeenCalledTimes(1);
    call.mockResolvedValue({
      microphone: "denied",
      systemAudio: "managed_by_macos",
    });
    reject(new Error("not granted"));
    await expect(consent).rejects.toThrow("not granted");
    expect(await read).toMatchObject({ microphone: "denied" });
    await settings.allow("microphone");
    expect(call).toHaveBeenLastCalledWith("request_capture_permission", {
      kind: "microphone",
    });
  });
  it("requests only the chosen permission and opens the matching settings pane", async () => {
    const service = accessService(true, true);
    await expect(service.allow("systemAudio")).rejects.toThrow(
      "when recording starts",
    );
    expect(call).not.toHaveBeenCalled();
    await service.openSettings("systemAudio");
    expect(call).toHaveBeenLastCalledWith("open_permission_settings", {
      kind: "systemAudio",
    });
    await service.allow("calendar");
    expect(call).toHaveBeenLastCalledWith("calendar_events");
    call.mockResolvedValue(false);
    await expect(service.allow("notifications")).rejects.toThrow(
      "Notification settings",
    );
    await service.openSettings("notifications");
    expect(call).toHaveBeenLastCalledWith("open_permission_settings", {
      kind: "notifications",
    });
  });
});
