import { invoke, isTauri, convertFileSrc } from "@tauri-apps/api/core";
import {
  defaultPreferences,
  type Meeting,
  type Preferences,
  type Recording,
  type CalendarEvent,
} from "./types";
import { samples } from "./samples";
export const native = isTauri();
let database: Promise<IDBDatabase> | undefined;
function db() {
  return (database ??= new Promise((resolve, reject) => {
    const request = indexedDB.open("patter-preview-v1", 1);
    request.onupgradeneeded = () => {
      const d = request.result;
      d.createObjectStore("meetings", { keyPath: "id" });
      d.createObjectStore("versions", { keyPath: ["id", "revision"] });
      d.createObjectStore("audio");
      d.createObjectStore("settings");
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  }));
}
async function read<T>(store: string, key?: IDBValidKey): Promise<T> {
  const d = await db();
  return new Promise((resolve, reject) => {
    const tx = d.transaction(store);
    const r =
      key === undefined
        ? tx.objectStore(store).getAll()
        : tx.objectStore(store).get(key);
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error);
  });
}
async function put(store: string, data: unknown, key?: IDBValidKey) {
  const d = await db();
  return new Promise<void>((resolve, reject) => {
    const tx = d.transaction(store, "readwrite");
    tx.objectStore(store).put(data, key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error ?? new Error("Save interrupted"));
  });
}
let queue = Promise.resolve();
function serial<T>(fn: () => Promise<T>): Promise<T> {
  const task = queue.then(fn);
  queue = task.then(
    () => {},
    () => {},
  );
  return task;
}
export async function listMeetings(): Promise<Meeting[]> {
  if (native) return invoke("list_meetings");
  if (!(await read<boolean>("settings", "seeded"))) {
    for (const m of samples) {
      await put("meetings", m);
      await put("versions", m);
    }
    await put("settings", true, "seeded");
  }
  return (await read<Meeting[]>("meetings")).sort((a, b) =>
    b.createdAt.localeCompare(a.createdAt),
  );
}
export function saveMeeting(meeting: Meeting): Promise<Meeting> {
  return serial(async () => {
    if (native) return invoke("save_meeting", { meeting });
    const d = await db();
    return new Promise((resolve, reject) => {
      const tx = d.transaction(["meetings", "versions"], "readwrite");
      const current = tx.objectStore("meetings").get(meeting.id);
      let saved: Meeting;
      current.onsuccess = () => {
        const old = current.result as Meeting | undefined;
        saved = {
          ...meeting,
          recordings: [
            ...meeting.recordings.filter(
              (r) => !old?.recordings.some((previous) => previous.id === r.id),
            ),
            ...(old?.recordings ?? []),
          ],
          revision: (old?.revision ?? 0) + 1,
        };
        tx.objectStore("meetings").put(saved);
        tx.objectStore("versions").put(saved);
      };
      tx.oncomplete = () => resolve(saved);
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error ?? new Error("Save interrupted"));
    });
  });
}
export async function history(id: string): Promise<Meeting[]> {
  return native
    ? invoke("meeting_history", { id })
    : (await read<Meeting[]>("versions"))
        .filter((m) => m.id === id)
        .sort((a, b) => b.revision - a.revision);
}
export async function getPreferences(): Promise<Preferences> {
  return {
    ...defaultPreferences,
    ...(native
      ? await invoke<Preferences>("get_preferences")
      : await read<Preferences>("settings", "preferences")),
  };
}
export async function setPreferences(preferences: Preferences) {
  if (native) await invoke("set_preferences", { preferences });
  else await put("settings", preferences, "preferences");
}
export async function audioSource(recording: Recording) {
  if (recording.url) return recording.url;
  if (native && recording.path) return convertFileSrc(recording.path);
  const blob = await read<Blob>("audio", recording.id);
  if (!blob) throw new Error("Recording file is missing.");
  return URL.createObjectURL(blob);
}
export async function importAudio(
  meeting: Meeting,
  file?: File,
): Promise<Meeting | null> {
  if (native) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({
      multiple: false,
      filters: [
        {
          name: "Audio",
          extensions: ["wav", "mp3", "m4a", "aiff", "caf", "flac", "ogg"],
        },
      ],
    });
    return path ? invoke("import_audio", { id: meeting.id, path }) : null;
  }
  if (!file) return null;
  const id = crypto.randomUUID();
  await put("audio", file, id);
  return saveMeeting({
    ...meeting,
    recordings: [
      ...meeting.recordings,
      { id, name: file.name, track: "Imported audio", offset: 0 },
    ],
  });
}
export async function modelList(endpoint: string): Promise<string[]> {
  if (!native)
    throw new Error(
      "Connect local models in the desktop app. The browser preview does not contact model servers.",
    );
  return invoke("list_models", { endpoint });
}
export async function summarize(id: string): Promise<Meeting> {
  return invoke("summarize", { id });
}
export async function transcribe(id: string): Promise<Meeting> {
  return invoke("transcribe", { id });
}
export async function calendarEvents(): Promise<CalendarEvent[]> {
  return invoke("calendar_events");
}
export async function startRecording(id: string): Promise<void> {
  return invoke("start_recording", { id });
}
export async function stopRecording(): Promise<Meeting> {
  return invoke("stop_recording");
}
export async function recordingStatus(): Promise<{
  active: boolean;
  id?: string;
  error?: string;
}> {
  return native ? invoke("recording_status") : { active: false };
}
export function downloadJson(name: string, value: unknown) {
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(value, null, 2)], { type: "application/json" }),
  );
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 5000);
}
