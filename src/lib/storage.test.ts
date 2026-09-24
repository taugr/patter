import "fake-indexeddb/auto";
import { describe, expect, it } from "vitest";
import { saveMeeting, history, importAudio, listMeetings } from "./storage";
import { matchesSearch, newMeeting } from "./types";

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
  it("serializes simultaneous saves into distinct revisions", async () => {
    const m = newMeeting();
    await Promise.all([
      saveMeeting({ ...m, notes: "one" }),
      saveMeeting({ ...m, notes: "two" }),
      saveMeeting({ ...m, notes: "three" }),
    ]);
    expect((await history(m.id)).map((v) => v.notes)).toEqual([
      "three",
      "two",
      "one",
    ]);
  });
  it("retains imported attachments when restoring an older version", async () => {
    const m = await saveMeeting(newMeeting());
    const imported = await importAudio(
      m,
      new File(["audio-fixture"], "example.wav", { type: "audio/wav" }),
    );
    const restored = await saveMeeting(m);
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
