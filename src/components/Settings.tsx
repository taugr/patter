import {
  type ReactNode,
  useState,
  useEffect,
  useCallback,
  useRef,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Check,
  DownloadSimple,
  ArrowSquareOut,
  GearSix,
  CalendarBlank,
  Cpu,
  FolderSimple,
  Plugs,
} from "@phosphor-icons/react";
import {
  calendarPermissionText,
  type CalendarPermission,
} from "../lib/permissions";
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
const settingsTabs = [
  { id: "general", label: "General", icon: GearSix },
  { id: "calendar", label: "Calendar", icon: CalendarBlank },
  { id: "models", label: "Models", icon: Cpu },
  { id: "library", label: "Library", icon: FolderSimple },
  { id: "agents", label: "Agents", icon: Plugs },
] as const;
type SettingsTab = (typeof settingsTabs)[number]["id"];

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
  const [tab, setTab] = useState<SettingsTab>("general");
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const contentRef = useRef<HTMLDivElement>(null);
  const [modelMessage, setModelMessage] = useState("");
  const [libraryMessage, setLibraryMessage] = useState("");
  const [draft, setDraft] = useState(preferences);
  const [models, setModels] = useState<string[]>([]);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [backingUp, setBackingUp] = useState(false);
  const [calendarMessage, setCalendarMessage] = useState("");
  const [notificationMessage, setNotificationMessage] = useState("");
  const [permissionMessage, setPermissionMessage] = useState("");
  const [requestingRecordingAccess, setRequestingRecordingAccess] =
    useState(false);
  const [calendarPermission, setCalendarPermission] =
    useState<CalendarPermission>(native ? "checking" : "unavailable");
  const [notificationPermission, setNotificationPermission] = useState(
    native ? "checking" : "unavailable",
  );
  const dirty = JSON.stringify(draft) !== JSON.stringify(preferences);
  useEffect(() => {
    setMessage("");
  }, [draft]);
  useEffect(() => {
    if (contentRef.current) contentRef.current.scrollTop = 0;
  }, [tab]);
  const refreshPermissions = useCallback(async () => {
    if (!native) return;
    const [calendar, notifications] = await Promise.allSettled([
      invoke<CalendarPermission>("calendar_permission"),
      invoke<string>("notification_permission"),
    ]);
    setCalendarPermission(
      calendar.status === "fulfilled" ? calendar.value : "unknown",
    );
    setNotificationPermission(
      notifications.status === "fulfilled"
        ? notifications.value
        : "unavailable",
    );
  }, []);
  useEffect(() => {
    void refreshPermissions();
    window.addEventListener("focus", refreshPermissions);
    const timer = native
      ? setInterval(() => void refreshPermissions(), 5000)
      : undefined;
    return () => {
      window.removeEventListener("focus", refreshPermissions);
      clearInterval(timer);
    };
  }, [refreshPermissions]);
  async function permissionAction(
    fn: () => Promise<void>,
    report: (message: string) => void,
  ) {
    setBusy(true);
    report("");
    try {
      await fn();
    } catch (e) {
      report(String(e));
    } finally {
      await refreshPermissions();
      setBusy(false);
    }
  }
  function openPermissions(kind: string, report: (message: string) => void) {
    void invoke("open_permission_settings", { kind }).catch((e) =>
      report(String(e)),
    );
  }
  async function act(fn: () => Promise<void>, report = setMessage) {
    setBusy(true);
    report("");
    try {
      await fn();
    } catch (e) {
      report(String(e));
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
      className="settings-dialog"
      onClose={() => {
        if (!importing && !backingUp) onClose();
      }}
    >
      {!native && (
        <p className="settings-preview">
          Preview · Some controls need the Mac app.
        </p>
      )}
      <fieldset className="settings-fields" disabled={installing || importing}>
        <div className="settings-layout">
          <div
            className="settings-tabs"
            role="tablist"
            aria-label="Settings categories"
            aria-orientation="horizontal"
          >
            {settingsTabs.map(({ id, label, icon: Icon }, index) => (
              <button
                key={id}
                id={`settings-tab-${id}`}
                role="tab"
                type="button"
                aria-selected={tab === id}
                aria-controls={`settings-panel-${id}`}
                tabIndex={tab === id ? 0 : -1}
                ref={(element) => {
                  tabRefs.current[index] = element;
                }}
                onClick={() => setTab(id)}
                onKeyDown={(event) => {
                  let next = index;
                  if (event.key === "ArrowDown" || event.key === "ArrowRight")
                    next = (index + 1) % settingsTabs.length;
                  else if (event.key === "ArrowUp" || event.key === "ArrowLeft")
                    next =
                      (index + settingsTabs.length - 1) % settingsTabs.length;
                  else if (event.key === "Home") next = 0;
                  else if (event.key === "End") next = settingsTabs.length - 1;
                  else return;
                  event.preventDefault();
                  setTab(settingsTabs[next].id);
                  tabRefs.current[next]?.focus();
                }}
              >
                <Icon size={20} aria-hidden="true" />
                {label}
              </button>
            ))}
          </div>
          {/* Keep panels mounted so tab changes preserve drafts and active jobs. */}
          <div className="settings-content" ref={contentRef}>
            <div
              id="settings-panel-general"
              role="tabpanel"
              aria-labelledby="settings-tab-general"
              hidden={tab !== "general"}
              tabIndex={0}
            >
              <section className="settings-section settings-zoom">
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
                <small>⌘+ / ⌘− to zoom · ⌘0 to reset</small>
              </section>
              <section className="settings-section">
                <h3>Recording</h3>
                <button
                  className="secondary"
                  disabled={!native || busy || installing}
                  onClick={() =>
                    void permissionAction(async () => {
                      setRequestingRecordingAccess(true);
                      try {
                        await invoke("request_recording_access");
                        setPermissionMessage(
                          "Recording access allowed. If macOS asks you to restart Patter, quit and reopen it.",
                        );
                      } finally {
                        setRequestingRecordingAccess(false);
                      }
                    }, setPermissionMessage)
                  }
                >
                  {requestingRecordingAccess
                    ? "Waiting for macOS…"
                    : "Enable recording access"}
                </button>
                <details className="settings-help">
                  <summary>Permissions help</summary>
                  <div className="permission-actions">
                    <button
                      className="text-button"
                      disabled={!native || busy}
                      onClick={() =>
                        openPermissions("microphone", setPermissionMessage)
                      }
                    >
                      Microphone <ArrowSquareOut size={14} />
                    </button>
                    <button
                      className="text-button"
                      disabled={!native || busy}
                      onClick={() =>
                        openPermissions("screen", setPermissionMessage)
                      }
                    >
                      Screen &amp; System Audio <ArrowSquareOut size={14} />
                    </button>
                  </div>
                  <p>
                    Grant access without recording. If access was denied, use
                    the shortcuts above, then reopen Patter. Computer audio uses
                    Screen &amp; System Audio Recording; no video is saved.
                  </p>
                </details>
                {permissionMessage && (
                  <p className="form-message" role="status">
                    {permissionMessage}
                  </p>
                )}
              </section>
              {updates(busy || importing || backingUp || downloading || dirty)}
            </div>
            <div
              id="settings-panel-calendar"
              role="tabpanel"
              aria-labelledby="settings-tab-calendar"
              hidden={tab !== "calendar"}
              tabIndex={0}
            >
              <section className="settings-section">
                <p>Use calendars connected to this Mac.</p>
                <button
                  className="secondary"
                  disabled={busy || !native}
                  onClick={() =>
                    permissionAction(async () => {
                      await onConnectCalendar();
                      setDraft((value) => ({
                        ...value,
                        calendarEnabled: true,
                      }));
                    }, setCalendarMessage)
                  }
                >
                  {busy ? "Please wait…" : "Connect calendar"}
                  <ArrowSquareOut size={16} />
                </button>
                <p
                  className={
                    calendarMessage || reminderError
                      ? "form-message"
                      : "permission-hint"
                  }
                  role="status"
                >
                  {calendarMessage ||
                    reminderError ||
                    calendarPermissionText(calendarPermission)}
                </p>
                <details className="settings-help">
                  <summary>Connection help</summary>
                  <div className="permission-actions">
                    <button
                      className="text-button"
                      disabled={!native || busy}
                      onClick={() =>
                        openPermissions("calendar", setCalendarMessage)
                      }
                    >
                      Calendar permissions <ArrowSquareOut size={14} />
                    </button>
                    <button
                      className="text-button"
                      disabled={!native || busy}
                      onClick={() => void refreshPermissions()}
                    >
                      Check again
                    </button>
                  </div>

                  <p>
                    Add your Google account in macOS Internet Accounts and
                    enable Calendars. Check that your meetings appear in Apple
                    Calendar, then connect here.
                  </p>
                  <button
                    className="text-button"
                    disabled={!native || busy}
                    onClick={() =>
                      openPermissions("accounts", setCalendarMessage)
                    }
                  >
                    Internet Accounts <ArrowSquareOut size={14} />
                  </button>
                </details>
              </section>
              <section className="settings-section">
                <h3>Reminders</h3>
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
                      void permissionAction(async () => {
                        const allowed = await invoke<boolean>(
                          "request_reminder_permission",
                        );
                        if (!allowed)
                          throw new Error(
                            "Allow Patter in System Settings → Notifications, then try again.",
                          );
                        setDraft((value) => ({
                          ...value,
                          reminderEnabled: true,
                        }));
                      }, setNotificationMessage);
                    }}
                  />
                  Notify me before meetings
                </label>
                <small>Patter must be open to send reminders.</small>
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
                            {minutes
                              ? `${minutes} min before`
                              : "At start time"}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="agent-option">
                      <input
                        type="checkbox"
                        checked={draft.reminderSound}
                        onChange={(e) =>
                          setDraft({
                            ...draft,
                            reminderSound: e.target.checked,
                          })
                        }
                      />
                      Play sound
                    </label>
                    <label className="agent-option">
                      <input
                        type="checkbox"
                        checked={draft.reminderShowTitle}
                        onChange={(e) =>
                          setDraft({
                            ...draft,
                            reminderShowTitle: e.target.checked,
                          })
                        }
                      />
                      Show meeting titles
                    </label>
                    <details>
                      <summary>Calendars</summary>
                      <small>
                        All calendars unless you select specific ones. All-day
                        and declined events are skipped.
                      </small>
                      {[
                        ...new Map(
                          calendarEvents
                            .filter((event) => event.calendarId)
                            .map((event) => [
                              event.calendarId!,
                              event.calendar,
                            ]),
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
                    void permissionAction(async () => {
                      const allowed = await invoke<boolean>(
                        "request_reminder_permission",
                      );
                      if (!allowed)
                        throw new Error(
                          "Allow Patter in System Settings → Notifications.",
                        );
                      await invoke("test_reminder");
                      setNotificationMessage(
                        "Test sent. Check macOS notification settings or Focus if it does not appear.",
                      );
                    }, setNotificationMessage);
                  }}
                >
                  {native ? "Test notification" : "Preview reminder"}
                </button>
                <details className="settings-help">
                  <summary>Notification settings</summary>
                  <p>
                    {notificationPermission === "blocked"
                      ? "Notifications are off. Allow Patter in macOS settings."
                      : "If reminders do not appear, check macOS notifications and Focus."}
                  </p>
                  <button
                    className="text-button"
                    disabled={!native || busy}
                    onClick={() =>
                      openPermissions("notifications", setNotificationMessage)
                    }
                  >
                    Open macOS settings <ArrowSquareOut size={14} />
                  </button>
                </details>
                {notificationMessage && (
                  <p className="form-message" role="status">
                    {notificationMessage}
                  </p>
                )}
              </section>
            </div>
            <div
              id="settings-panel-models"
              role="tabpanel"
              aria-labelledby="settings-tab-models"
              hidden={tab !== "models"}
              tabIndex={0}
            >
              <TranscriptionSettings
                draft={draft}
                onChange={setDraft}
                onBusy={setDownloading}
              />
              <section className="settings-section">
                <h3>Summaries</h3>

                <label>
                  Local server
                  <input
                    value={draft.endpoint}
                    onChange={(e) =>
                      setDraft({ ...draft, endpoint: e.target.value })
                    }
                    placeholder="http://127.0.0.1:1234/v1"
                  />
                </label>
                <div className="form-row">
                  <label>
                    Model
                    <input
                      list="models"
                      value={draft.model}
                      onChange={(e) =>
                        setDraft({ ...draft, model: e.target.value })
                      }
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
                        setModelMessage(
                          found.length
                            ? `${found.length} models found.`
                            : "The server has no loaded models.",
                        );
                      }, setModelMessage)
                    }
                  >
                    Find models
                  </button>
                </div>
                {modelMessage && (
                  <p className="form-message" role="status">
                    {modelMessage}
                  </p>
                )}
                <details className="settings-help">
                  <summary>Connect a local model</summary>
                  <p>
                    Start a server in LM Studio or Ollama, then find a model. LM
                    Studio uses port 1234; Ollama uses 11434. Include /v1 in the
                    address.
                  </p>
                </details>
              </section>
              <section className="settings-section">
                <h3>Templates</h3>

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
                <details className="settings-help">
                  <summary>Edit instructions</summary>
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
                      draft.templateInstructions[draft.summaryTemplate] ===
                      undefined
                    }
                    onClick={() => {
                      const instructions = { ...draft.templateInstructions };
                      delete instructions[draft.summaryTemplate];
                      setDraft({
                        ...draft,
                        templateInstructions: instructions,
                      });
                    }}
                  >
                    Reset instructions
                  </button>
                </details>
              </section>
            </div>
            <div
              id="settings-panel-library"
              role="tabpanel"
              aria-labelledby="settings-tab-library"
              hidden={tab !== "library"}
              tabIndex={0}
            >
              <BackupSettings
                disabled={busy || installing || importing}
                beforeBackup={async () => {
                  await beforeImport();
                  await save();
                }}
                onBusy={setBackingUp}
              />
              <section className="settings-section">
                <h3>Local backup</h3>
                {!native && <p>Preview export excludes audio.</p>}
                <button
                  className="secondary"
                  disabled={busy || importing || backingUp}
                  onClick={() =>
                    act(async () => {
                      if (native) {
                        const { open } =
                          await import("@tauri-apps/plugin-dialog");
                        const path = await open({ directory: true });
                        if (typeof path === "string") {
                          const saved = await invoke<string>("backup_library", {
                            destination: path,
                          });
                          setLibraryMessage(`Library backup saved to ${saved}`);
                        }
                      } else {
                        downloadJson(
                          "patter-preview-notes.json",
                          await listMeetings(),
                        );
                        setLibraryMessage("Notes exported without audio.");
                      }
                    }, setLibraryMessage)
                  }
                >
                  <DownloadSimple size={18} />
                  {native ? "Back up library" : "Export preview notes"}
                </button>
                {libraryMessage && (
                  <p className="form-message" role="status">
                    {libraryMessage}
                  </p>
                )}
              </section>
              <details className="settings-help settings-import">
                <summary>Import from Anarlog</summary>
                <AnarlogImport
                  disabled={busy || installing || backingUp}
                  onBusy={setImporting}
                  beforeImport={beforeImport}
                  onImported={onImported}
                />
              </details>
            </div>
            <div
              id="settings-panel-agents"
              role="tabpanel"
              aria-labelledby="settings-tab-agents"
              hidden={tab !== "agents"}
              tabIndex={0}
            >
              <AgentSettings
                disabled={busy || installing || importing || backingUp}
              />
            </div>
          </div>
        </div>
        <div className="dialog-actions settings-footer">
          <span className="form-message" role="status">
            {busy
              ? "Working…"
              : importing
                ? "Importing…"
                : backingUp
                  ? "Backing up…"
                  : downloading
                    ? "Downloading model…"
                    : message || (dirty ? "Unsaved changes" : "")}
          </span>
          <button
            className="primary"
            disabled={busy || importing || backingUp || !dirty}
            onClick={() => act(save)}
          >
            <Check size={18} />
            Save changes
          </button>
        </div>
      </fieldset>
    </Dialog>
  );
}
