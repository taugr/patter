import { describe, expect, it } from "vitest";
import { newMeeting, type Meeting } from "./types";
import {
  mergeSavedMeeting,
  summaryFreshness,
  summaryInput,
} from "./summary-input";

function summarized(): Meeting {
  const meeting = {
    ...newMeeting("Review"),
    revision: 2,
    notes: "Friday",
    summary: "Review Friday.",
    transcript: [
      { start: 0, speaker: "A", text: "First" },
      { start: 5, text: "Second" },
    ],
  };
  return {
    ...meeting,
    summaryInput: { projectionVersion: 1, text: summaryInput(meeting) },
  };
}
describe("exact summary input", () => {
  it("uses the native projection and distinguishes text edits from other revisions", () => {
    const meeting = summarized();
    expect(summaryInput(meeting)).toBe(
      "Meeting: Review\nNotes:\nFriday\nTranscript:\nFirst\nSecond",
    );
    expect(summaryFreshness(meeting)).toBe("current");
    expect(summaryFreshness({ ...meeting, notes: "Monday" })).toBe("changed");
    expect(summaryFreshness({ ...meeting, title: "New title" })).toBe(
      "changed",
    );
    expect(
      summaryFreshness({
        ...meeting,
        transcript: [{ start: 0, text: "Monday" }],
      }),
    ).toBe("changed");
    expect(
      summaryFreshness({
        ...meeting,
        revision: 9,
        archived: true,
        summaryTemplate: "interview",
        actions: [{ id: "a", text: "Task", done: true }],
        transcript: meeting.transcript.map((s) => ({
          ...s,
          start: 99,
          speaker: "B",
        })),
      }),
    ).toBe("current");
    expect(
      summaryFreshness({
        ...meeting,
        notes: "Monday",
        summaryInput: meeting.summaryInput,
      }),
    ).toBe("changed");
    expect(summaryFreshness({ ...meeting, notes: "Friday" })).toBe("current");
  });
  it("reports legacy, missing and incompatible input as unknown", () => {
    const meeting = summarized();
    expect(
      summaryFreshness({
        ...meeting,
        summaryInput: undefined,
        summaryFreshness: "current",
      }),
    ).toBe("unknown");
    expect(
      summaryFreshness({
        ...meeting,
        summaryInput: { projectionVersion: 2, text: summaryInput(meeting) },
      }),
    ).toBe("unknown");
    expect(summaryFreshness(newMeeting())).toBeUndefined();
  });
  it("merges native save provenance immediately without overwriting a later draft", () => {
    const saved = summarized();
    const draft = { ...saved, notes: "Monday", summaryInput: undefined };
    const result = mergeSavedMeeting(draft, { ...saved, revision: 3 }, 2);
    expect(result.notes).toBe("Monday");
    expect(result.revision).toBe(3);
    expect(summaryFreshness(result)).toBe("changed");
    expect(mergeSavedMeeting({ ...draft, revision: 4 }, saved, 2)).toEqual({
      ...draft,
      revision: 4,
    });
    expect(
      mergeSavedMeeting(
        saved,
        {
          ...saved,
          summaryInput: undefined,
          summaryFreshness: "unknown",
          revision: 3,
        },
        2,
      ).summaryInput,
    ).toBeUndefined();
  });
});
