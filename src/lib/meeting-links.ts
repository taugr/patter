import { invoke } from "@tauri-apps/api/core";
import { native } from "./storage";

export function meetingLink(
  value?: string | null,
): { url: string; provider: string } | null {
  if (!value) return null;
  value = value.trim();
  if (value.length > 8192 || /[\u0000-\u001f\u007f-\u009f\\]/u.test(value))
    return null;
  try {
    const url = new URL(value);
    if (url.protocol !== "https:" || url.username || url.password || url.port)
      return null;
    const path = url.pathname.replace(/^\/+|\/+$/g, "");
    if (
      url.hostname === "meet.google.com" &&
      (/^[a-z]{3}-[a-z]{4}-[a-z]{3}$/.test(path) ||
        /^lookup\/[^/]+$/.test(path))
    ) {
      return { url: url.href, provider: "Google Meet" };
    }
    if (
      ["zoom.us", "zoom.com"].some(
        (host) => url.hostname === host || url.hostname.endsWith(`.${host}`),
      ) &&
      (/^(j|w)\/\d+$/.test(path) ||
        /^my\/[^/]+$/.test(path) ||
        /^wc\/\d+\/join$/.test(path))
    ) {
      return { url: url.href, provider: "Zoom" };
    }
  } catch {
    /* Calendar links may be missing or malformed. */
  }
  return null;
}

export async function openMeetingLink(
  value: string,
): Promise<string | undefined> {
  const link = meetingLink(value);
  if (!link)
    throw new Error(
      "This is not a supported Google Meet or Zoom meeting link.",
    );
  if (!native)
    return `Example ${link.provider} meeting. Join opens your meeting in the Mac app.`;
  await invoke("open_meeting_link", { url: link.url });
}
