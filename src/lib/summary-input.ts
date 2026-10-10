import type { Meeting } from "./types";

// Keep projection version 1 identical to native summary_input::project.
export const summaryInputVersion = 1;
export function summaryInput(meeting: Meeting): string {
  return `Meeting: ${meeting.title}\nNotes:\n${meeting.notes}\nTranscript:\n${meeting.transcript
    .map((segment) => segment.text)
    .filter((text) => typeof text === "string")
    .join("\n")}`;
}

export function summaryFreshness(meeting: Meeting) {
  if (!meeting.summary) return undefined;
  const source = meeting.summaryInput;
  if (!source || source.projectionVersion !== summaryInputVersion)
    return "unknown";
  return source.text === summaryInput(meeting) ? "current" : "changed";
}

// Preserve later draft edits while accepting server-resolved historical input.
export function mergeSavedMeeting(
  current: Meeting,
  saved: Meeting,
  expectedRevision: number,
): Meeting {
  if (current.id !== saved.id || current.revision !== expectedRevision)
    return current;
  return {
    ...current,
    revision: saved.revision,
    summaryInput: saved.summaryInput,
    summaryFreshness: saved.summaryFreshness,
  };
}
