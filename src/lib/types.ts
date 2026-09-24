export interface ActionItem {
  id: string;
  text: string;
  done: boolean;
}
export interface Segment {
  start: number;
  text: string;
  speaker?: string;
}
export interface Recording {
  id: string;
  name: string;
  path?: string;
  url?: string;
  track: string;
  offset: number;
}
export interface Meeting {
  id: string;
  title: string;
  createdAt: string;
  duration: number;
  notes: string;
  summary: string;
  summaryTemplate?: string;
  summaryInstructions?: string;
  summarySource?: {
    templateId: string;
    templateName: string;
    instructions: string;
    extraInstructions: string;
    model: string;
    generatedAt: string;
  };
  decisions: string[];
  actions: ActionItem[];
  transcript: Segment[];
  recordings: Recording[];
  archived: boolean;
  revision: number;
  sample?: boolean;
  eventId?: string;
}
export interface CalendarEvent {
  id: string;
  title: string;
  start: string;
  end: string;
  url?: string;
  calendar: string;
}
export interface Preferences {
  endpoint: string;
  model: string;
  whisperModel: string;
  calendarEnabled: boolean;
  summaryTemplate: string;
  templateInstructions: Record<string, string>;
}
export const defaultPreferences: Preferences = {
  endpoint: "http://127.0.0.1:1234/v1",
  model: "",
  whisperModel: "",
  calendarEnabled: false,
  summaryTemplate: "general",
  templateInstructions: {},
};
export function newMeeting(title = "Untitled conversation"): Meeting {
  return {
    id: crypto.randomUUID(),
    title,
    createdAt: new Date().toISOString(),
    duration: 0,
    notes: "",
    summary: "",
    decisions: [],
    actions: [],
    transcript: [],
    recordings: [],
    archived: false,
    revision: 0,
  };
}
export function matchesSearch(meeting: Meeting, query: string) {
  const words = query.toLocaleLowerCase().trim().split(/\s+/);
  const text = [
    meeting.title,
    meeting.notes,
    meeting.summary,
    ...meeting.transcript.map((s) => s.text),
    ...meeting.actions.map((a) => a.text),
  ]
    .join(" ")
    .toLocaleLowerCase();
  return words.every((word) => text.includes(word));
}
export function formatTime(seconds: number) {
  const safe = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0));
  return `${Math.floor(safe / 60)
    .toString()
    .padStart(2, "0")}:${(safe % 60).toString().padStart(2, "0")}`;
}
