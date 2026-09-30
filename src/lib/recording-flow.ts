import type { Meeting } from "./types";

// The gate closes synchronously, before any save, permission check or IPC await.
export function recordingFlow(service: {
  start: (id: string) => Promise<void>;
  stop: () => Promise<Meeting>;
  status: () => Promise<{ active: boolean; id?: string; error?: string }>;
}) {
  let working = false;
  let active: string | null = null;
  return {
    get active() {
      return active;
    },
    get working() {
      return working;
    },
    async start(resolve: () => Promise<Meeting>) {
      if (working || active)
        throw new Error("Finish the current recording or task first.");
      working = true;
      try {
        const meeting = await resolve();
        await service.start(meeting.id);
        active = meeting.id;
        return meeting.id;
      } finally {
        working = false;
      }
    },
    async stop(before: () => Promise<void> = async () => {}) {
      if (working || !active) throw new Error("No recording is ready to stop.");
      working = true;
      try {
        await before();
        const meeting = await service.stop();
        active = null;
        return meeting;
      } catch (error) {
        // IPC/save failure may leave capture alive. Keep Stop available unless
        // the native side confirms the session has already been released.
        const status = await service.status().catch(() => null);
        if (status && !status.active && !status.id) active = null;
        throw error;
      } finally {
        working = false;
      }
    },
  };
}
