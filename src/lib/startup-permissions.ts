import { invoke } from "@tauri-apps/api/core";
import type { CalendarPermission } from "./permissions";

export type PermissionId =
  "microphone" | "systemAudio" | "calendar" | "notifications";
export type AccessSnapshot = {
  microphone: "allowed" | "not_requested" | "denied" | "restricted" | "unknown";
  systemAudio: "managed_by_macos";
  calendar?: CalendarPermission;
  notifications?: string;
};
export type AccessRow = {
  id: PermissionId;
  label: string;
  detail: string;
  action: "allow" | "settings" | "restricted" | "check";
};
export function missingAccess(s: AccessSnapshot): AccessRow[] {
  const rows: AccessRow[] = [];
  if (s.microphone !== "allowed")
    rows.push({
      id: "microphone",
      label: "Microphone",
      detail:
        s.microphone === "restricted"
          ? "Restricted by macOS or your administrator."
          : "Record your voice.",
      action:
        s.microphone === "restricted"
          ? "restricted"
          : s.microphone === "denied"
            ? "settings"
            : s.microphone === "not_requested"
              ? "allow"
              : "check",
    });
  // macOS checks Core Audio consent at recording start. A missing screen
  // grant says nothing about system audio and must not create an Allow loop.
  if (s.calendar && s.calendar !== "allowed")
    rows.push({
      id: "calendar",
      label: "Calendar",
      detail: "Read your upcoming meetings.",
      action:
        s.calendar === "restricted"
          ? "restricted"
          : s.calendar === "denied"
            ? "settings"
            : ["not_requested", "write_only"].includes(s.calendar)
              ? "allow"
              : "check",
    });
  if (s.notifications && s.notifications !== "allowed")
    rows.push({
      id: "notifications",
      label: "Notifications",
      detail: "Show your meeting reminders.",
      action:
        s.notifications === "blocked"
          ? "settings"
          : s.notifications === "not requested"
            ? "allow"
            : "check",
    });
  return rows;
}
export interface AccessService {
  check(): Promise<AccessSnapshot | null>;
  allow(id: PermissionId): Promise<void>;
  openSettings(id: PermissionId): Promise<void>;
}
// Coalesce only in-flight work across Settings/startup/StrictMode. Never cache
// a settled status: every focus/Check again reads the current macOS grant.
let pendingRecordingCheck: Promise<AccessSnapshot | null> | undefined;
let pendingPermissionRequest: Promise<void> | undefined;
function recordingPreflight() {
  return (pendingRecordingCheck ??= (async () => {
    if (pendingPermissionRequest)
      await pendingPermissionRequest.catch(() => {});
    return invoke<AccessSnapshot | null>("recording_permissions");
  })().finally(() => {
    pendingRecordingCheck = undefined;
  }));
}
export function accessService(
  calendar: boolean,
  notifications: boolean,
): AccessService {
  let checking: Promise<AccessSnapshot | null> | undefined;
  async function read() {
    const recording = await recordingPreflight();
    if (!recording) return null; // Unpackaged development builds cannot own consent.
    const [calendarStatus, notificationStatus] = await Promise.all([
      calendar ? invoke<CalendarPermission>("calendar_permission") : undefined,
      notifications ? invoke<string>("notification_permission") : undefined,
    ]);
    return {
      ...recording,
      calendar: calendarStatus,
      notifications: notificationStatus,
    };
  }
  return {
    // StrictMode/focus checks share one helper process; checking never prompts.
    check: () =>
      (checking ??= read().finally(() => {
        checking = undefined;
      })),
    async allow(id) {
      if (id === "systemAudio")
        throw new Error(
          "macOS checks system audio when recording starts. Screen recording is not needed.",
        );
      if (pendingPermissionRequest)
        throw new Error(
          "A permission request is already open. Finish it first.",
        );
      // Capture existing reads before publishing the request. New reads wait
      // for consent to finish; old reads finish first without a circular wait.
      const existingCheck = checking;
      const existingRecordingCheck = pendingRecordingCheck;
      const request = (async () => {
        await existingCheck?.catch(() => {});
        await existingRecordingCheck?.catch(() => {});
        if (id === "calendar") await invoke("calendar_events");
        else if (id === "notifications") {
          if (!(await invoke<boolean>("request_reminder_permission")))
            throw new Error("Allow Patter in macOS Notification settings.");
        } else await invoke("request_capture_permission", { kind: id });
      })();
      pendingPermissionRequest = request;
      try {
        await request;
      } finally {
        pendingPermissionRequest = undefined;
      }
    },
    openSettings: (kind) => invoke("open_permission_settings", { kind }),
  };
}
