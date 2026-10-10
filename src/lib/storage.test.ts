import "fake-indexeddb/auto";
import { describe, expect, it } from "vitest";
import {
  saveMeeting,
  history,
  importAudio,
  listMeetings,
  getPreferences,
  setPreferences,
} from "./storage";
import { matchesSearch, newMeeting, type Preferences } from "./types";
import { effectiveTemplate, templateInstructions } from "./templates";
import { summaryFreshness } from "./summary-input";

describe("summary source history", () => {
  it("resolves the original source across saves, reloads, copies and unknown legacy provenance", async () => {
    const original = await saveMeeting({
      ...newMeeting("Friday review"),
      notes: "The review is Friday.",
    });
    const summary = await saveMeeting({
      ...original,
      summary: "Review Friday.",
      decisions: ["Review Friday."],
      actions: [{ id: "fixed", text: "Prepare Friday", done: true }],
      summarySource: {
        templateId: "general",
        templateName: "General",
        instructions: "Fixed",
        extraInstructions: "",
        model: "injected",
        generatedAt: "2026-10-10T00:00:00Z",
        sourceConversationId: original.id,
        sourceRevision: original.revision,
        inputProjectionVersion: 1,
      },
    });
    expect(summaryFreshness(summary)).toBe("current");
    const monday = await saveMeeting({
      ...summary,
      notes: "The review is Monday.",
      summaryInput: { projectionVersion: 1, text: "forged" },
    });
    expect(summaryFreshness(monday)).toBe("changed");
    expect(monday.actions).toEqual(summary.actions);
    expect(
      summaryFreshness(
        (await listMeetings()).find((m) => m.id === original.id)!,
      ),
    ).toBe("changed");
    const copy = await saveMeeting({
      ...monday,
      id: crypto.randomUUID(),
      revision: 0,
      title: "Draft copy",
    });
    expect(copy.summaryInput).toEqual(summary.summaryInput);
    expect(copy.summarySource?.sourceConversationId).toBe(original.id);
    const restored = await saveMeeting({
      ...summary,
      revision: monday.revision,
    });
    expect(summaryFreshness(restored)).toBe("current");
    const missing = await saveMeeting({
      ...restored,
      summarySource: { ...summary.summarySource!, sourceRevision: 999 },
    });
    expect(summaryFreshness(missing)).toBe("unknown");
    const legacy = await saveMeeting({
      ...newMeeting(),
      summary: "Old summary",
    });
    expect(summaryFreshness(legacy)).toBe("unknown");
    expect(
      (await history(original.id)).find((m) => m.revision === summary.revision)
        ?.summaryFreshness,
    ).toBe("current");
  });
});

describe("summary templates", () => {
  it("upgrades legacy preferences and persists edited templates and conversation overrides", async () => {
    await setPreferences({
      endpoint: "http://127.0.0.1:1234/v1",
      model: "test",
      whisperModel: "",
      calendarEnabled: false,
    } as Preferences);
    const old = await getPreferences();
    expect(effectiveTemplate(newMeeting(), old).id).toBe("general");
    expect(old.transcriptionModel).toBe("parakeet-v3");
    await setPreferences({
      ...old,
      whisperModel: "/legacy.bin",
      transcriptionModel: undefined,
    } as unknown as Preferences);
    expect((await getPreferences()).transcriptionModel).toBe("whisper-file");
    await setPreferences({
      ...old,
      summaryTemplate: "interview",
      templateInstructions: { brainstorm: "Keep open alternatives." },
    });
    const prefs = await getPreferences();
    expect(effectiveTemplate(newMeeting(), prefs).id).toBe("interview");
    const conversation = await saveMeeting({
      ...newMeeting(),
      summaryTemplate: "brainstorm",
      summaryInstructions: "Focus on access.",
    });
    const saved = (await listMeetings()).find((m) => m.id === conversation.id)!;
    expect(effectiveTemplate(saved, prefs).id).toBe("brainstorm");
    expect(templateInstructions("brainstorm", prefs)).toBe(
      "Keep open alternatives.",
    );
    expect(saved.summaryInstructions).toBe("Focus on access.");
    await saveMeeting({
      ...saved,
      summaryTemplate: "",
      summaryInstructions: "",
    });
    const versions = await history(saved.id);
    expect(effectiveTemplate(versions[0], prefs).id).toBe("interview");
    expect(versions[1].summaryTemplate).toBe("brainstorm");
    expect(versions[1].summaryInstructions).toBe("Focus on access.");
  });
});

describe("permanent conversation history", () => {
  it("keeps every edit and archive without removing originals", async () => {
    const original = await saveMeeting({
      ...newMeeting("Durability check"),
      notes: "Original notes",
    });
    const changed = await saveMeeting({
      ...original,
      notes: "Revised notes",
      transcript: [{ start: 0, text: "An original transcript" }],
    });
    await saveMeeting({ ...changed, archived: true });
    const versions = await history(original.id);
    expect(versions.map((v) => v.revision)).toEqual([3, 2, 1]);
    expect(versions[2].notes).toBe("Original notes");
    expect(versions[0].transcript[0].text).toBe("An original transcript");
    expect(
      (await listMeetings()).find((m) => m.id === original.id)?.archived,
    ).toBe(true);
  });
  it("rejects stale simultaneous saves instead of overwriting another editor", async () => {
    const m = newMeeting();
    const results = await Promise.allSettled([
      saveMeeting({ ...m, notes: "one" }),
      saveMeeting({ ...m, notes: "two" }),
    ]);
    expect(results.map((r) => r.status)).toEqual(["fulfilled", "rejected"]);
    expect((await history(m.id)).map((v) => v.notes)).toEqual(["one"]);
  });
  it("retains imported attachments when restoring an older version", async () => {
    const m = await saveMeeting(newMeeting());
    const imported = await importAudio(
      m,
      new File(["audio-fixture"], "example.wav", { type: "audio/wav" }),
    );
    const restored = await saveMeeting({ ...m, revision: imported!.revision });
    expect(restored.recordings).toEqual(imported?.recordings);
    const attemptedRewrite = await saveMeeting({
      ...restored,
      recordings: restored.recordings.map((r) => ({
        ...r,
        path: "/different/path",
      })),
    });
    expect(attemptedRewrite.recordings).toEqual(imported?.recordings);
  });
  it("searches notes, transcripts and actions across words", () => {
    const m = {
      ...newMeeting("Planning"),
      notes: "Armenia",
      transcript: [{ start: 0, text: "hiking routes" }],
      actions: [{ id: "a", text: "Compare trails", done: false }],
    };
    expect(matchesSearch(m, "armenia TRAILS")).toBe(true);
    expect(matchesSearch(m, "missing")).toBe(false);
  });
});

describe("calendar meeting links", () => {
  it("retains the join link independently of edited notes and in saved history", async () => {
    const joinUrl = "https://zoom.us/j/123456789?pwd=Encoded%2BPass";
    const original = await saveMeeting({
      ...newMeeting("Calendar meeting"),
      eventId: "calendar-occurrence",
      joinUrl,
      notes: joinUrl,
    });
    await saveMeeting({
      ...original,
      notes: "My notes without the invitation",
    });
    const reopened = (await listMeetings()).find((m) => m.id === original.id)!;
    expect(reopened.joinUrl).toBe(joinUrl);
    expect(reopened.eventId).toBe("calendar-occurrence");
    expect(
      (await history(original.id)).every(
        (version) => version.joinUrl === joinUrl,
      ),
    ).toBe(true);
  });
});
