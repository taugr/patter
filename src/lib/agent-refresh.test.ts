import { describe, expect, it } from "vitest";
import { mergeAgentRefresh } from "./agent-refresh";
import { newMeeting } from "./types";
describe("agent refresh", () => {
  it("refreshes saved meetings while retaining a local draft", () => {
    const draft = { ...newMeeting(), notes: "Unsaved draft", revision: 1 };
    const other = { ...newMeeting(), createdAt: draft.createdAt, revision: 1 };
    const remote = { ...draft, notes: "Agent edit", revision: 2 };
    const updated = { ...other, notes: "New notes", revision: 2 };
    expect(
      mergeAgentRefresh([draft, other], [remote, updated], draft.id),
    ).toEqual([draft, updated]);
    expect(mergeAgentRefresh([draft], [remote])).toEqual([remote]);
  });
  it("does not regress a newer local save when an older fetch returns", () => {
    const local = { ...newMeeting(), revision: 3 };
    expect(mergeAgentRefresh([local], [])).toEqual([local]);
    expect(mergeAgentRefresh([local], [{ ...local, revision: 2 }])).toEqual([
      local,
    ]);
  });
});
