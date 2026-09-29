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
      return "Click Connect calendar to request access. Patter appears in macOS Calendar permissions after this request.";
    case "allowed":
      return "Full Calendar access allowed. Patter only reads your events.";
    case "denied":
      return "Calendar access is off. Open Calendar permissions, allow Patter, then reconnect.";
    case "write_only":
      return "Full access is needed to read events. Click Connect calendar to request it.";
    case "restricted":
      return "Calendar access is restricted by macOS or your administrator.";
    case "unavailable":
      return "Open the packaged Patter app from Applications to configure permissions.";
    default:
      return "Calendar permission could not be checked. Try checking again.";
  }
}
