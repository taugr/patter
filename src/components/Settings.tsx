import { type ReactNode, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, DownloadSimple, ArrowSquareOut } from "@phosphor-icons/react";
import { zoomLevels } from "../lib/zoom";
import { Dialog } from "./Dialog";
import {
  modelList,
  native,
  setPreferences,
  downloadJson,
  listMeetings,
} from "../lib/storage";
import type { CalendarEvent, Preferences } from "../lib/types";
import { TranscriptionSettings } from "./TranscriptionSettings";
import { AgentSettings } from "./AgentSettings";
import { BackupSettings } from "./BackupSettings";
import { AnarlogImport } from "./AnarlogImport";
import {
  instructionLimit,
  summaryTemplates,
  templateInstructions,
} from "../lib/templates";
export function Settings({
  calendarEvents,
  reminderError,
  onPreviewReminder,
  zoom,
  onZoom,
  preferences,
  onSave,
  onClose,
  onConnectCalendar,
  updates,
  installing,
  beforeImport,
  onImported,
}: {
  calendarEvents: CalendarEvent[];
  reminderError: string | null;
  onPreviewReminder: () => void;
  zoom: number;
  onZoom: (value: number) => void;
  beforeImport: () => Promise<void>;
  onImported: () => Promise<void>;
  updates: (settingsPending: boolean) => ReactNode;
  installing: boolean;
  preferences: Preferences;
  onSave: (p: Preferences) => void;
  onClose: () => void;
  onConnectCalendar: () => Promise<void>;
}) {
  const [draft, setDraft] = useState(preferences);
  const [models, setModels] = useState<string[]>([]);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [backingUp, setBackingUp] = useState(false);
  async function act(fn: () => Promise<void>) {
    setBusy(true);
    setMessage("");
    try {
      await fn();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function save() {
    await setPreferences(draft);
    onSave(draft);
    setMessage("Settings saved.");
  }
  return (
    <Dialog
      title="Settings"
      onClose={() => {
        if (!importing && !backingUp) onClose();
      }}
    >
      {!native && (
        <p className="form-message">
          Browser preview. Disabled features are available in the Mac app.
        </p>
      )}
      {updates(
        busy ||
          importing ||
          backingUp ||
          downloading ||
          JSON.stringify(draft) !== JSON.stringify(preferences),
      )}
      <fieldset className="settings-fields" disabled={installing || importing}>
        <section className="settings-section">
          <h3>Appearance</h3>
          <label>
            Zoom
            <select
              value={zoom}
              onChange={(e) => onZoom(Number(e.target.value))}
            >
              {zoomLevels.map((level) => (
                <option key={level} value={level}>
                  {level}%
                </option>
              ))}
            </select>
          </label>
          <small>
            ⌘+ to zoom in · ⌘− to zoom out · ⌘0 to reset. Saved on this Mac.
          </small>
        </section>
        <section className="settings-section">
          <h3>Calendar</h3>
          <p>Use your Mac’s calendars, including Google Calendar.</p>
          <button
            className="secondary"
            disabled={busy || !native}
            onClick={() =>
              act(async () => {
                await onConnectCalendar();
                setDraft({ ...draft, calendarEnabled: true });
                setMessage("Calendar connected.");
              })
            }
          >
            Connect calendar
            <ArrowSquareOut size={16} />
          </button>
          <label className="agent-option">
            <input
              type="checkbox"
              checked={draft.reminderEnabled}
              disabled={!native || !draft.calendarEnabled || busy}
              onChange={(e) => {
                const enabled = e.target.checked;
                if (!enabled) {
                  setDraft({ ...draft, reminderEnabled: false });
                  return;
                }
                void act(async () => {
                  const allowed = await invoke<boolean>(
                    "request_reminder_permission",
                  );
                  if (!allowed)
                    throw new Error(
                      "Allow Patter in System Settings → Notifications, then try again.",
                    );
                  setDraft((value) => ({ ...value, reminderEnabled: true }));
                });
              }}
            />
            Meeting reminders
          </label>
          <small>
            While Patter is open, including in the background. Save to apply
            changes.
          </small>
          {draft.reminderEnabled && (
            <>
              <label>
                Remind me
                <select
                  value={draft.reminderMinutes}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      reminderMinutes: Number(e.target.value),
                    })
                  }
                >
                  {[0, 1, 2, 5, 10, 15, 30].map((minutes) => (
                    <option key={minutes} value={minutes}>
                      {minutes ? `${minutes} min before` : "At start time"}
                    </option>
                  ))}
                </select>
              </label>
              <label className="agent-option">
                <input
                  type="checkbox"
                  checked={draft.reminderSound}
                  onChange={(e) =>
                    setDraft({ ...draft, reminderSound: e.target.checked })
                  }
                />
                Play sound
              </label>
              <label className="agent-option">
                <input
                  type="checkbox"
                  checked={draft.reminderShowTitle}
                  onChange={(e) =>
                    setDraft({ ...draft, reminderShowTitle: e.target.checked })
                  }
                />
                Show meeting titles in notifications
              </label>
              <details>
                <summary>Calendars</summary>
                <small>
                  All calendars unless you select specific ones. All-day and
                  declined events are skipped.
                </small>
                {[
                  ...new Map(
                    calendarEvents
                      .filter((event) => event.calendarId)
                      .map((event) => [event.calendarId!, event.calendar]),
                  ).entries(),
                ].map(([id, name]) => (
                  <label className="agent-option" key={id}>
                    <input
                      type="checkbox"
                      checked={draft.reminderCalendars.includes(id)}
                      onChange={(e) =>
                        setDraft({
                          ...draft,
                          reminderCalendars: e.target.checked
                            ? [...draft.reminderCalendars, id]
                            : draft.reminderCalendars.filter(
                                (item) => item !== id,
                              ),
                        })
                      }
                    />
                    {name}
                  </label>
                ))}
                {!!draft.reminderCalendars.length && (
                  <button
                    className="text-button"
                    onClick={() =>
                      setDraft({ ...draft, reminderCalendars: [] })
                    }
                  >
                    Use all calendars
                  </button>
                )}
              </details>
            </>
          )}
          <button
            className="text-button"
            disabled={busy}
            onClick={() => {
              if (!native) {
                onPreviewReminder();
                return;
              }
              void act(async () => {
                const allowed = await invoke<boolean>(
                  "request_reminder_permission",
                );
                if (!allowed)
                  throw new Error(
                    "Allow Patter in System Settings → Notifications.",
                  );
                await invoke("test_reminder");
                setMessage(
                  "Test sent. Check macOS notification settings or Focus if it does not appear.",
                );
              });
            }}
          >
            {native ? "Test notification" : "Preview reminder"}
          </button>
          {reminderError && (
            <p className="form-message" role="status">
              {reminderError}
            </p>
          )}
        </section>
        <section className="settings-section">
          <h3>Summaries</h3>
          <p>
            Connect LM Studio, Ollama, or another local OpenAI-compatible
            server.
          </p>
          <label>
            Local server
            <input
              value={draft.endpoint}
              onChange={(e) => setDraft({ ...draft, endpoint: e.target.value })}
              placeholder="http://127.0.0.1:1234/v1"
            />
          </label>
          <div className="form-row">
            <label>
              Model
              <input
                list="models"
                value={draft.model}
                onChange={(e) => setDraft({ ...draft, model: e.target.value })}
                placeholder="Model name"
              />
              <datalist id="models">
                {models.map((m) => (
                  <option key={m} value={m} />
                ))}
              </datalist>
            </label>
            <button
              className="secondary"
              disabled={busy || !native}
              onClick={() =>
                act(async () => {
                  const found = await modelList(draft.endpoint);
                  setModels(found);
                  setMessage(
                    found.length
                      ? `${found.length} models found.`
                      : "The server has no loaded models.",
                  );
                })
              }
            >
              Find models
            </button>
          </div>
          <small>LM Studio: port 1234. Ollama: port 11434, with /v1.</small>
        </section>
        <section className="settings-section">
          <h3>Summary templates</h3>
          <p>Choose a default; change it for individual conversations.</p>
          <label>
            Default
            <select
              value={draft.summaryTemplate}
              onChange={(e) =>
                setDraft({ ...draft, summaryTemplate: e.target.value })
              }
            >
              {summaryTemplates.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            Instructions
            <textarea
              rows={5}
              maxLength={instructionLimit}
              value={templateInstructions(draft.summaryTemplate, draft)}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  templateInstructions: {
                    ...draft.templateInstructions,
                    [draft.summaryTemplate]: e.target.value,
                  },
                })
              }
            />
          </label>
          <button
            className="text-button"
            type="button"
            disabled={
              draft.templateInstructions[draft.summaryTemplate] === undefined
            }
            onClick={() => {
              const instructions = { ...draft.templateInstructions };
              delete instructions[draft.summaryTemplate];
              setDraft({ ...draft, templateInstructions: instructions });
            }}
          >
            Reset instructions
          </button>
          <small>Instructions are saved per template.</small>
        </section>
        <TranscriptionSettings
          draft={draft}
          onChange={setDraft}
          onBusy={setDownloading}
        />
        <section className="settings-section">
          <h3>Library</h3>
          <p>Archived conversations keep their audio and version history.</p>
          <button
            className="secondary"
            disabled={busy || importing || backingUp}
            onClick={() =>
              act(async () => {
                if (native) {
                  const { open } = await import("@tauri-apps/plugin-dialog");
                  const path = await open({ directory: true });
                  if (typeof path === "string") {
                    const saved = await invoke<string>("backup_library", {
                      destination: path,
                    });
                    setMessage(`Library backup saved to ${saved}`);
                  }
                } else {
                  downloadJson(
                    "patter-preview-notes.json",
                    await listMeetings(),
                  );
                  setMessage("Notes exported without audio.");
                }
              })
            }
          >
            <DownloadSimple size={18} />
            {native ? "Back up library" : "Export preview notes"}
          </button>
        </section>
        <AgentSettings
          disabled={busy || installing || importing || backingUp}
        />
        <BackupSettings
          disabled={busy || installing || importing}
          beforeBackup={async () => {
            await beforeImport();
            await save();
          }}
          onBusy={setBackingUp}
        />
        <AnarlogImport
          disabled={busy || installing || backingUp}
          onBusy={setImporting}
          beforeImport={beforeImport}
          onImported={onImported}
        />
        {message && (
          <p className="form-message" role="status">
            {message}
          </p>
        )}
        <div className="dialog-actions">
          <button
            className="text-button"
            disabled={importing || backingUp}
            onClick={onClose}
          >
            Close
          </button>
          <button
            className="primary"
            disabled={busy || importing || backingUp}
            onClick={() => act(save)}
          >
            <Check size={18} />
            Save
          </button>
        </div>
      </fieldset>
    </Dialog>
  );
}
