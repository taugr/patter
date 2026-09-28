import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { CalendarEvent } from "../lib/types";
import { native } from "../lib/storage";
import { Dialog } from "./Dialog";
export type ReminderState = {
  events: CalendarEvent[];
  alerts: CalendarEvent[];
  requested: string | null;
  error: string | null;
  notificationError: string | null;
  lastUpdated: string | null;
};
export function MeetingReminders({
  state,
  disabled,
  onRecord,
  onOpen,
  onDismiss,
  onError,
}: {
  state: ReminderState;
  disabled: boolean;
  onRecord: (event: CalendarEvent) => Promise<void>;
  onOpen: (event: CalendarEvent) => Promise<void>;
  onDismiss: (id: string) => void;
  onError: (message: string) => void;
}) {
  const [requested, setRequested] = useState<string | null>(null);
  const [working, setWorking] = useState(false);
  useEffect(() => {
    if (state.requested) setRequested(state.requested);
  }, [state.requested]);
  const prompt = state.alerts.find((event) => event.id === requested);
  async function dismiss(id: string) {
    if (native) await invoke("dismiss_reminder", { id });
    onDismiss(id);
    setRequested(null);
  }
  async function run(event: CalendarEvent, record: boolean) {
    setWorking(true);
    try {
      await (record ? onRecord(event) : onOpen(event));
      await dismiss(event.id);
    } catch (error) {
      onError(String(error));
    } finally {
      setWorking(false);
    }
  }
  return (
    <>
      {!!state.alerts.length && (
        <aside
          className="meeting-reminders"
          aria-label="Meeting reminders"
          aria-live="polite"
        >
          {state.alerts.map((event) => (
            <div className="meeting-reminder" key={event.id}>
              <div>
                <strong>{event.title}</strong>
                <small>
                  {new Date(event.start).toLocaleTimeString([], {
                    hour: "2-digit",
                    minute: "2-digit",
                  })}{" "}
                  · {event.calendar}
                </small>
              </div>
              <div className="reminder-actions">
                <button
                  className="primary"
                  disabled={disabled || working}
                  onClick={() => setRequested(event.id)}
                >
                  Record…
                </button>
                <button
                  className="secondary"
                  disabled={disabled || working}
                  onClick={() => void run(event, false)}
                >
                  Open notes
                </button>
                <button
                  className="text-button"
                  disabled={working}
                  onClick={() =>
                    void dismiss(event.id).catch((e) => onError(String(e)))
                  }
                >
                  Dismiss
                </button>
              </div>
            </div>
          ))}
        </aside>
      )}
      {prompt && !disabled && (
        <Dialog
          title={prompt.title}
          onClose={() => {
            if (!working) setRequested(null);
          }}
        >
          <p>
            Record this meeting’s microphone and computer audio. Start only when
            everyone is comfortable being recorded.
          </p>
          {!native && <p>Recording is available in the Mac app.</p>}
          <button
            className="primary"
            disabled={!native || working}
            onClick={() => void run(prompt, true)}
          >
            {working ? "Starting…" : "Start recording"}
          </button>
        </Dialog>
      )}
    </>
  );
}
