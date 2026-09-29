export type CalendarPermission =
  | "checking"
  | "not_requested"
  | "allowed"
  | "denied"
  | "restricted"
  | "write_only"
  | "unavailable"
  | "unknown";
export function calendarPermissionText(status: CalendarPermission): string {
  switch (status) {
    case "checking":
      return "Checking calendar permission…";
    case "not_requested":
      return "Connect calendar to request access. Patter appears in macOS permissions after this request.";
    case "allowed":
      return "Calendar access allowed.";
    case "denied":
      return "Calendar access is off. Open Calendar permissions, allow Patter, then reconnect.";
    case "write_only":
      return "Full access is needed to read events. Click Connect calendar to request it.";
    case "restricted":
      return "Calendar access is restricted by macOS or your administrator.";
    case "unavailable":
      return "Use the packaged Patter app to set permissions.";
    default:
      return "Calendar permission could not be checked. Try checking again.";
  }
}
