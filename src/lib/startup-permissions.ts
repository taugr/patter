import { invoke } from "@tauri-apps/api/core";
import type { CalendarPermission } from "./permissions";

export type PermissionId =
  "microphone" | "screen" | "calendar" | "notifications";
export type AccessSnapshot = {
  microphone: "allowed" | "not_requested" | "denied" | "restricted" | "unknown";
  screenAllowed: boolean;
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
  if (!s.screenAllowed)
    rows.push({
      id: "screen",
      label: "Computer audio",
      detail: "Uses Screen & System Audio Recording. No video is saved.",
      action: "allow",
    });
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
export function accessService(
  calendar: boolean,
  notifications: boolean,
): AccessService {
  let checking: Promise<AccessSnapshot | null> | undefined;
  async function read() {
    const recording = await invoke<AccessSnapshot | null>(
      "recording_permissions",
    );
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
      // Finish an in-flight preflight before starting a consent helper.
      await checking?.catch(() => {});
      if (id === "calendar") await invoke("calendar_events");
      else if (id === "notifications") {
        if (!(await invoke<boolean>("request_reminder_permission")))
          throw new Error("Allow Patter in macOS Notification settings.");
      } else await invoke("request_capture_permission", { kind: id });
    },
    openSettings: (kind) => invoke("open_permission_settings", { kind }),
  };
}
