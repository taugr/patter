import { describe, expect, it, vi } from "vitest";
import { recordingFlow } from "./recording-flow";
import { newMeeting } from "./types";
function setup() {
  const meeting = {
    ...newMeeting("Fixture meeting"),
    eventId: "calendar:occurrence",
    archived: true,
  };
  const service = {
    start: vi.fn(async (_id: string) => {}),
    stop: vi.fn(async () => meeting),
    status: vi.fn(async () => ({ active: true, id: meeting.id })),
  };
  return { meeting, service, flow: recordingFlow(service) };
}
describe("recording destination and lifecycle", () => {
  it("records and appends to the chosen linked/archived conversation, preserving its identity", async () => {
    const { meeting, service, flow } = setup();
    expect(await flow.start(async () => meeting)).toBe(meeting.id);
    expect(service.start).toHaveBeenCalledWith(meeting.id);
    expect((await flow.stop()).eventId).toBe("calendar:occurrence");
    await flow.start(async () => meeting);
    expect(service.start).toHaveBeenNthCalledWith(2, meeting.id);
  });
  it("rejects a duplicate click synchronously before resolving or creating another note", async () => {
    const { meeting, service, flow } = setup();
    let finish!: () => void;
    service.start.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    const first = flow.start(async () => meeting);
    const duplicate = vi.fn(async () => newMeeting());
    await expect(flow.start(duplicate)).rejects.toThrow("Finish");
    expect(duplicate).not.toHaveBeenCalled();
    finish();
    await first;
    await expect(flow.start(duplicate)).rejects.toThrow("Finish");
    expect(service.start).toHaveBeenCalledTimes(1);
  });
  it("a permission/start error releases the gate, never claims recording, and permits retry on the same identity", async () => {
    const { meeting, service, flow } = setup();
    service.start.mockRejectedValueOnce(
      new Error("Microphone access was not granted"),
    );
    await expect(flow.start(async () => meeting)).rejects.toThrow("Microphone");
    expect(flow.active).toBeNull();
    expect(flow.working).toBe(false);
    await flow.start(async () => meeting);
    expect(flow.active).toBe(meeting.id);
  });
  it("preserves Stop after an uncertain save/IPC failure and allows recovery of interrupted capture", async () => {
    const { meeting, service, flow } = setup();
    await flow.start(async () => meeting);
    service.stop.mockRejectedValueOnce(new Error("IPC failed"));
    await expect(flow.stop()).rejects.toThrow("IPC failed");
    expect(flow.active).toBe(meeting.id);
    expect((await flow.stop()).id).toBe(meeting.id);
    expect(flow.active).toBeNull();
  });
  it("prevents duplicate stops and clears ended sessions after native save errors", async () => {
    const { meeting, service, flow } = setup();
    await flow.start(async () => meeting);
    service.stop.mockRejectedValueOnce(
      new Error("Save failed after capture ended"),
    );
    service.status.mockResolvedValueOnce({
      active: false,
      id: undefined,
    } as unknown as { active: boolean; id: string });
    const first = flow.stop();
    await expect(flow.stop()).rejects.toThrow("No recording");
    await expect(first).rejects.toThrow("Save failed");
    expect(flow.active).toBeNull();
  });
});
