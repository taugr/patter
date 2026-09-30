import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  Microphone,
  NotePencil,
  UploadSimple,
  Stop,
  X,
  Check,
  ArrowLeft,
} from "@phosphor-icons/react";
import {
  MeetingReminders,
  type ReminderState,
} from "./components/MeetingReminders";
import { useZoom } from "./lib/use-zoom";
import { Sidebar } from "./components/Sidebar";
import { MeetingPane } from "./components/MeetingPane";
import { Dialog } from "./components/Dialog";
import { Updates, type UpdateInfo } from "./components/Updates";
import { version as appVersion } from "../package.json";
import { StartupPermissions } from "./components/StartupPermissions";
import { Settings } from "./components/Settings";
import { recordingFlow } from "./lib/recording-flow";
import { mergeAgentRefresh } from "./lib/agent-refresh";
import * as storage from "./lib/storage";
import {
  matchesSearch,
  newMeeting,
  defaultPreferences,
  formatTime,
  type Meeting,
  type Preferences,
  type CalendarEvent,
} from "./lib/types";
export default function App() {
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo>({
    currentVersion: appVersion,
    update: null,
  });
  const [updateStatus, setUpdateStatus] = useState("");
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [installingUpdate, setInstallingUpdate] = useState(false);
  const installingRef = useRef(false);
  const checkingRef = useRef(false);
  async function checkUpdates(quiet = false) {
    if (!storage.native || checkingRef.current || installingRef.current) return;
    checkingRef.current = true;
    setCheckingUpdate(true);
    if (!quiet) setUpdateStatus("");
    try {
      const info = await invoke<UpdateInfo>("check_update");
      setUpdateInfo(info);
      if (!quiet) setUpdateStatus(info.update ? "" : "You’re up to date.");
      if (quiet && info.update)
        setNotice(
          `Patter ${info.update.version} is available in Settings → General → Updates.`,
        );
    } catch (e) {
      if (!quiet)
        setUpdateStatus(`Could not check for updates. Try again. ${String(e)}`);
    } finally {
      checkingRef.current = false;
      setCheckingUpdate(false);
    }
  }
  async function installUpdate() {
    if (!updateInfo.update || installingRef.current || recording || busy)
      return;
    installingRef.current = true;
    setInstallingUpdate(true);
    setUpdateStatus("Saving your changes…");
    try {
      await flush();
      setUpdateStatus("Downloading update…");
      await invoke("install_update", { version: updateInfo.update.version });
    } catch (e) {
      setUpdateStatus(`Update not installed. ${String(e)}`);
    } finally {
      installingRef.current = false;
      setInstallingUpdate(false);
    }
  }
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState("all");
  const [preferences, setPrefs] = useState<Preferences>(defaultPreferences);
  const [recordTarget, setRecordTarget] = useState<
    Meeting | CalendarEvent | null
  >(null);
  const [refreshingCalendar, setRefreshingCalendar] = useState(false);
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [modal, setModal] = useState<"record" | "settings" | "history" | null>(
    null,
  );
  const [reminders, setReminders] = useState<ReminderState>({
    events: [],
    alerts: [],
    requested: null,
    error: null,
    notificationError: null,
    lastUpdated: null,
  });
  const [versions, setVersions] = useState<Meeting[]>([]);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState("");
  const [saving, setSaving] = useState("");
  const [recording, setRecording] = useState<string | null>(null);
  const [recordingInterrupted, setRecordingInterrupted] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [ready, setReady] = useState(false);
  const [startupAccessOpen, setStartupAccessOpen] = useState(false);
  const [detailOpen, setDetailOpen] = useState(false);
  const inFlight = useRef<Promise<void> | null>(null);
  const capture = useRef(
    recordingFlow({
      start: storage.startRecording,
      stop: storage.stopRecording,
      status: storage.recordingStatus,
    }),
  );
  const openingEvent = useRef<Promise<void>>(Promise.resolve());
  const processingId = useRef<string | null>(null);
  const calendarRefresh = useRef<Promise<void> | null>(null);
  const recordingRef = useRef(recording);
  recordingRef.current = recording;
  const [conflict, setConflict] = useState(false);
  const recovering = useRef(false);
  const pending = useRef<Meeting | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const file = useRef<HTMLInputElement>(null);
  const meetingsRef = useRef(meetings);
  meetingsRef.current = meetings;
  const selected = meetings.find((m) => m.id === selectedId);
  const reportError = useCallback(
    (message: string) =>
      setError(
        message.includes("REVISION_CONFLICT")
          ? "This conversation changed elsewhere. Review the latest version before retrying."
          : message.replace(/^Error:\s*/, ""),
      ),
    [],
  );
  function replace(m: Meeting) {
    const next = [m, ...meetingsRef.current.filter((x) => x.id !== m.id)].sort(
      (a, b) => b.createdAt.localeCompare(a.createdAt),
    );
    meetingsRef.current = next;
    setMeetings(next);
  }
  async function flush(): Promise<void> {
    clearTimeout(timer.current);
    if (inFlight.current) {
      await inFlight.current;
      return flush();
    }
    const m = pending.current;
    if (!m) return;
    pending.current = null;
    setSaving("Saving…");
    const task = (async () => {
      try {
        const saved = await storage.saveMeeting(m);
        const queued = pending.current as Meeting | null;
        if (queued?.id === saved.id && queued.revision === m.revision)
          queued.revision = saved.revision;
        const next = meetingsRef.current.map((x) =>
          x.id === saved.id && x.revision === m.revision
            ? { ...x, revision: saved.revision }
            : x,
        );
        meetingsRef.current = next;
        setMeetings(next);
        if (!pending.current) setSaving("");
      } catch (e) {
        if (!pending.current) pending.current = m;
        if (String(e).includes("REVISION_CONFLICT")) {
          setConflict(true);
          setSaving("Draft kept — conversation changed elsewhere");
          setError("");
        } else {
          setSaving("Not saved — retry needed");
          reportError(`Your changes could not be saved: ${String(e)}`);
        }
        throw e;
      }
    })();
    inFlight.current = task;
    try {
      await task;
    } finally {
      inFlight.current = null;
    }
  }
  async function keepDraftCopy() {
    const draft = pending.current;
    if (!draft || recovering.current) return;
    recovering.current = true;
    setBusy("Saving your draft");
    clearTimeout(timer.current);
    try {
      const copied = await storage.saveMeeting({
        ...draft,
        id: crypto.randomUUID(),
        revision: 0,
        title: `${draft.title} (recovered draft)`,
        createdAt: new Date().toISOString(),
      });
      pending.current = null;
      const latest = await storage.listMeetings();
      meetingsRef.current = latest;
      setMeetings(latest);
      setSelectedId(copied.id);
      setMode(copied.archived ? "archive" : "all");
      setQuery("");
      setConflict(false);
      setError("");
      setSaving("");
      setNotice(
        "Your draft was saved as a separate conversation. The other changes are preserved in the original.",
      );
    } catch (e) {
      reportError(String(e));
    } finally {
      recovering.current = false;
      setBusy("");
    }
  }
  useEffect(() => {
    if (!storage.native) return;
    let disposed = false;
    const stops: (() => void)[] = [];
    async function refresh(openId?: string) {
      try {
        if (inFlight.current) await inFlight.current.catch(() => {});
        const latest = await storage.listMeetings();
        if (disposed) return;
        const draft = pending.current;
        const remote = latest.find((m) => m.id === draft?.id);
        if (draft && remote && remote.revision !== draft.revision) {
          clearTimeout(timer.current);
          setConflict(true);
          setSaving("Draft kept — conversation changed elsewhere");
        }
        const next = mergeAgentRefresh(meetingsRef.current, latest, draft?.id);
        meetingsRef.current = next;
        setMeetings(next);
        if (openId) {
          await flush();
          setSelectedId(openId);
          setDetailOpen(true);
          setMode(
            latest.find((m) => m.id === openId)?.archived ? "archive" : "all",
          );
          setQuery("");
        }
      } catch (e) {
        reportError(String(e));
      }
    }
    void Promise.all([
      listen<string>("patter-agent-changed", () => void refresh()),
      listen<string>(
        "patter-agent-open",
        ({ payload }) => void refresh(payload),
      ),
    ]).then((listeners) => {
      if (disposed) listeners.forEach((stop) => stop());
      else stops.push(...listeners);
    });
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, []);
  useEffect(() => {
    if (!storage.native) return;
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    const close = async () => {
      if (installingRef.current) {
        reportError("Wait for the update to finish before closing Patter.");
        return;
      }
      if (recordingRef.current || capture.current.working) {
        reportError("Stop the recording before closing Patter.");
        return;
      }
      try {
        await flush();
        await invoke("finish_quit");
      } catch (e) {
        reportError(String(e));
      }
    };
    void Promise.all([
      listen("patter-close-requested", () => {
        void close();
      }),
      getCurrentWindow().onCloseRequested((e) => {
        e.preventDefault();
        void close();
      }),
    ]).then((stops) => {
      if (disposed) stops.forEach((stop) => stop());
      else unlisteners.push(...stops);
    });
    return () => {
      disposed = true;
      unlisteners.forEach((stop) => stop());
    };
  }, []);
  useEffect(() => {
    if (!storage.native) return;
    let disposed = false;
    const stops: (() => void)[] = [];
    void Promise.all([
      listen<{ id: string; stage: string }>(
        "patter-recording-processing",
        ({ payload }) => {
          if (processingId.current === payload.id)
            setBusy(
              payload.stage === "transcript"
                ? "Transcribing on this Mac"
                : "Preparing your summary",
            );
        },
      ),
      listen("patter-check-updates", () => {
        setModal("settings");
        void checkUpdates();
      }),
      listen<{ downloaded?: number; total?: number; installing?: boolean }>(
        "patter-update-progress",
        ({ payload }) => {
          setUpdateStatus(
            payload.installing
              ? "Installing and restarting…"
              : payload.total
                ? `Downloading… ${Math.min(100, Math.round(((payload.downloaded ?? 0) / payload.total) * 100))}%`
                : `Downloading… ${((payload.downloaded ?? 0) / 1024 / 1024).toFixed(1)} MB`,
          );
        },
      ),
    ]).then((listeners) => {
      if (disposed) listeners.forEach((stop) => stop());
      else stops.push(...listeners);
    });
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, []);
  useEffect(() => {
    if (ready) void checkUpdates(true);
  }, [ready]);
  useEffect(() => {
    if (!storage.native || !ready) return;
    let cancelled = false;
    let polling = false;
    const refresh = async () => {
      if (polling) return;
      polling = true;
      try {
        const state = await invoke<ReminderState>("reminder_status");
        if (!cancelled) {
          setReminders(state);
          setEvents(state.events);
        }
      } catch (error) {
        if (!cancelled) reportError(String(error));
      } finally {
        polling = false;
      }
    };
    void refresh();
    const interval = setInterval(() => void refresh(), 3000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [ready, reportError]);
  async function refreshCalendar() {
    if (!storage.native || !preferences.calendarEnabled) return;
    if (calendarRefresh.current) return calendarRefresh.current;
    const task = (async () => {
      setRefreshingCalendar(true);
      try {
        const latest = await invoke<CalendarEvent[]>("refresh_calendar");
        setEvents(latest);
        setReminders((state) => ({ ...state, events: latest, error: null }));
      } catch (e) {
        reportError(`Calendar could not refresh: ${String(e)}`);
      } finally {
        setRefreshingCalendar(false);
      }
    })();
    calendarRefresh.current = task;
    try {
      await task;
    } finally {
      calendarRefresh.current = null;
    }
  }
  useEffect(() => {
    if (!ready || !storage.native || !preferences.calendarEnabled) return;
    const refresh = () => {
      if (!document.hidden) void refreshCalendar();
    };
    refresh();
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    return () => {
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
    };
  }, [ready, preferences.calendarEnabled]);
  function update(m: Meeting) {
    replace(m);
    pending.current = m;
    setSaving("Saving…");
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      void flush().catch(() => {});
    }, 600);
  }
  useEffect(() => {
    let cancelled = false;
    Promise.all([storage.listMeetings(), storage.getPreferences()])
      .then(async ([items, p]) => {
        if (cancelled) return;
        setMeetings(items);
        setSelectedId(items.find((m) => !m.archived)?.id ?? "");
        setPrefs(p);
        setReady(true);
      })
      .catch((e) => reportError(String(e)));
    return () => {
      cancelled = true;
    };
  }, []);
  useEffect(() => {
    const before = (e: BeforeUnloadEvent) => {
      if (pending.current || inFlight.current || recording) {
        e.preventDefault();
      }
    };
    const keys = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        document.querySelector<HTMLInputElement>(".search input")?.focus();
      }
    };
    const visibility = () => {
      if (document.hidden) void flush().catch(() => {});
    };
    window.addEventListener("beforeunload", before);
    window.addEventListener("keydown", keys);
    document.addEventListener("visibilitychange", visibility);
    return () => {
      window.removeEventListener("beforeunload", before);
      window.removeEventListener("keydown", keys);
      document.removeEventListener("visibilitychange", visibility);
    };
  }, [recording]);
  useEffect(() => {
    if (!recording) return;
    setElapsed(0);
    setRecordingInterrupted(false);
    const started = Date.now();
    const id = setInterval(() => {
      setElapsed((Date.now() - started) / 1000);
      void storage
        .recordingStatus()
        .then((s) => {
          if (s.error || !s.active) {
            setRecordingInterrupted(true);
            reportError(
              s.error ||
                "Recording stopped unexpectedly. Press Stop to recover the saved audio.",
            );
          }
        })
        .catch((e) => reportError(String(e)));
    }, 1000);
    return () => clearInterval(id);
  }, [recording]);
  async function act(fn: () => Promise<void>) {
    setError("");
    try {
      await flush();
      await fn();
    } catch (e) {
      reportError(String(e));
    }
  }
  const { zoom, setZoom } = useZoom(reportError);
  async function create(title?: string) {
    const m = await storage.saveMeeting(newMeeting(title));
    replace(m);
    setSelectedId(m.id);
    setMode("all");
    setQuery("");
    setDetailOpen(true);
    return m;
  }
  async function choose(m: Meeting) {
    await flush();
    setSelectedId(m.id);
    setDetailOpen(true);
  }
  async function openCalendarEvent(event: CalendarEvent): Promise<Meeting> {
    // Queue every open, including concurrent opens of different events.
    const task = openingEvent.current.then(() => resolveCalendarEvent(event));
    openingEvent.current = task.then(
      () => {},
      () => {},
    );
    return task;
  }
  async function resolveCalendarEvent(event: CalendarEvent): Promise<Meeting> {
    await flush();
    const existing = meetingsRef.current.find((m) => m.eventId === event.id);
    if (existing) {
      await choose(existing);
      setMode(existing.archived ? "archive" : "all");
      setQuery("");
      return existing;
    }
    const links = [...new Set([event.url, event.joinUrl].filter(Boolean))];
    const saved = await storage.saveMeeting({
      ...newMeeting(event.title),
      eventId: event.id,
      joinUrl: event.joinUrl ?? event.url,
      notes: [
        event.calendar,
        new Date(event.start).toLocaleString(),
        ...links,
      ].join("\n"),
    });
    replace(saved);
    setSelectedId(saved.id);
    setMode("all");
    setQuery("");
    setDetailOpen(true);
    return saved;
  }
  function requestRecording(target: Meeting | CalendarEvent | null) {
    if (capture.current.active) {
      setSelectedId(capture.current.active);
      setDetailOpen(true);
      return;
    }
    if (capture.current.working || busy || installingRef.current) return;
    setError("");
    setRecordTarget(target);
    setModal("record");
  }
  async function startRecording(target: Meeting | CalendarEvent | null) {
    if (
      busy ||
      installingRef.current ||
      capture.current.working ||
      capture.current.active
    )
      throw new Error("Finish the current recording or task first.");
    setError("");
    setNotice("");
    setBusy("Starting recording");
    try {
      const id = await capture.current.start(async () => {
        await flush();
        if (!target || !("recordings" in target)) {
          const meeting = target
            ? await openCalendarEvent(target)
            : await create("New conversation");
          // Retrying a denied/failed start reuses the same prepared conversation.
          setRecordTarget(meeting);
          return meeting;
        }
        const current = meetingsRef.current.find((m) => m.id === target.id);
        if (!current)
          throw new Error("This conversation is no longer available.");
        await choose(current);
        setMode(current.archived ? "archive" : "all");
        setQuery("");
        return current;
      });
      recordingRef.current = id;
      setRecording(id);
      setModal(null);
    } finally {
      setBusy("");
    }
  }
  async function recordCalendarEvent(event: CalendarEvent) {
    await startRecording(event);
  }
  async function stopRecording() {
    if (capture.current.working) return;
    setError("");
    setNotice("");
    setBusy("Finishing your recording");
    const previousGeneration = meetingsRef.current.find(
      (m) => m.id === capture.current.active,
    )?.recordingProcessing?.generation;
    try {
      const m = await capture.current.stop(flush);
      replace(m);
      recordingRef.current = null;
      setRecording(null);
      if (
        m.recordingProcessing?.status === "pending" &&
        m.recordingProcessing.generation !== previousGeneration
      )
        await finishRecordingProcessing(m.id);
      else setNotice("Recording saved. No new complete audio to process.");
    } catch (e) {
      reportError(String(e));
    } finally {
      recordingRef.current = capture.current.active;
      setRecording(capture.current.active);
      setBusy("");
    }
  }
  async function finishRecordingProcessing(id: string) {
    if (processingId.current) return;
    processingId.current = id;
    setNotice("");
    setBusy("Transcribing on this Mac");
    try {
      const m = await invoke<Meeting>("process_recording", { id });
      replace(m);
      setNotice(
        m.recordingProcessing?.status === "skipped"
          ? m.recordingProcessing.error || "Transcript saved."
          : "Recording, transcript and summary saved.",
      );
    } catch (e) {
      const latest = await storage.listMeetings();
      const m = latest.find((m) => m.id === id);
      if (m) replace(m);
      reportError(`Recording saved. Processing needs attention: ${String(e)}`);
    } finally {
      processingId.current = null;
      setBusy("");
    }
  }
  async function importFile(f?: File) {
    if (!storage.native && !f) {
      file.current?.click();
      return;
    }
    await act(async () => {
      const m = selected ?? (await create(f?.name.replace(/\.[^.]+$/, "")));
      const result = await storage.importAudio(m, f);
      if (result) {
        replace(result);
        setNotice("Audio imported.");
      }
    });
  }
  async function process(kind: "summary" | "transcript") {
    if (!selected) return;
    if (!storage.native) {
      setModal("settings");
      setNotice("Local AI is available in the Mac app.");
      return;
    }
    await act(async () => {
      setBusy(
        kind === "summary"
          ? "Preparing your summary"
          : "Transcribing on this Mac",
      );
      try {
        const m =
          kind === "summary"
            ? await storage.summarize(selected.id)
            : await storage.transcribe(selected.id);
        replace(m);
        setNotice(kind === "summary" ? "Summary ready." : "Transcript ready.");
      } finally {
        setBusy("");
      }
    });
  }
  const visible = meetings.filter(
    (m) =>
      m.archived === (mode === "archive") &&
      matchesSearch(m, query) &&
      (mode !== "today" ||
        new Date(m.createdAt).toDateString() === new Date().toDateString()),
  );
  const displayedEvents = storage.native
    ? events
    : [
        {
          id: "sample-event",
          title: "Design catch-up",
          joinUrl: "https://meet.google.com/abc-defg-hij",
          start: "2026-09-23T14:00:00+04:00",
          end: "2026-09-23T14:30:00+04:00",
          calendar: "Example calendar",
        },
      ];
  const linkedEvent = displayedEvents.find(
    (event) => event.id === selected?.eventId,
  );
  return (
    <div className={`app ${detailOpen ? "detail-open" : ""}`}>
      <Sidebar
        meetings={visible}
        selected={selectedId}
        query={query}
        setQuery={setQuery}
        mode={mode}
        setMode={(value) => {
          setMode(value);
          setDetailOpen(false);
        }}
        onSelect={(m) => void act(() => choose(m))}
        onRecord={() => requestRecording(selected ?? null)}
        recordingDisabled={!ready || !!busy || installingUpdate || conflict}
        calendarEnabled={storage.native ? preferences.calendarEnabled : true}
        calendarError={reminders.error}
        refreshingCalendar={refreshingCalendar}
        onRefreshCalendar={() => void refreshCalendar()}
        onRecordEvent={(event) => requestRecording(event)}
        onSettings={() =>
          void act(async () => {
            setModal("settings");
          })
        }
        onNew={() =>
          void act(async () => {
            await create();
          })
        }
        events={displayedEvents}
        onEvent={(event) =>
          void act(async () => {
            await openCalendarEvent(event);
          })
        }
      />
      <MeetingReminders
        state={reminders}
        disabled={
          !!busy ||
          !!recording ||
          !!modal ||
          conflict ||
          installingUpdate ||
          startupAccessOpen
        }
        onRecord={recordCalendarEvent}
        onOpen={async (event) => {
          await openCalendarEvent(event);
        }}
        onDismiss={(id) =>
          setReminders((state) => ({
            ...state,
            alerts: state.alerts.filter((event) => event.id !== id),
            requested: null,
          }))
        }
        onError={reportError}
      />
      {conflict && (
        <div className="agent-conflict" role="alert">
          <p>
            This conversation changed elsewhere. Your unsaved draft is still
            here.
          </p>
          <button
            className="secondary"
            disabled={!!busy}
            onClick={() => void keepDraftCopy()}
          >
            Save my draft as a separate conversation
          </button>
        </div>
      )}
      <button className="mobile-back" onClick={() => setDetailOpen(false)}>
        <ArrowLeft size={18} />
        Conversations
      </button>
      {selected ? (
        <MeetingPane
          meeting={selected}
          joinUrl={
            linkedEvent
              ? (linkedEvent.joinUrl ?? linkedEvent.url ?? null)
              : undefined
          }
          preferences={preferences}
          onUpdate={update}
          saving={saving}
          activeRecording={recording === selected.id}
          busy={busy}
          onArchive={() =>
            void act(async () => {
              const archived = !selected.archived;
              const current =
                meetingsRef.current.find((m) => m.id === selected.id) ??
                selected;
              const m = await storage.saveMeeting({ ...current, archived });
              replace(m);
              setMode(archived ? "archive" : "all");
              setNotice(
                archived ? "Conversation archived." : "Conversation restored.",
              );
            })
          }
          onResumeProcessing={() =>
            void act(() => finishRecordingProcessing(selected.id))
          }
          onRecord={() => requestRecording(selected)}
          recordingDisabled={
            !!recording || !!busy || conflict || installingUpdate
          }
          onImport={() => void importFile()}
          onHistory={() =>
            void act(async () => {
              setVersions(await storage.history(selected.id));
              setModal("history");
            })
          }
          onExport={() =>
            void act(async () => {
              storage.downloadJson(`${selected.title || "conversation"}.json`, {
                conversation: selected,
                versions: await storage.history(selected.id),
              });
              setNotice(
                "Notes and transcript exported. Use Library backup for audio files.",
              );
            })
          }
          onSummarize={() => void process("summary")}
          onTranscribe={() => void process("transcript")}
          onError={reportError}
        />
      ) : (
        <main className="welcome">
          <img src="/patter-mark.svg" alt="" />
          <h1>A place for your conversations.</h1>
          <p>
            {ready
              ? "Record a meeting, take notes, or import audio."
              : "Opening your notebook…"}
          </p>
          <button
            className="primary"
            onClick={() => requestRecording(null)}
            disabled={!ready}
          >
            <Microphone size={21} />
            Record a conversation
          </button>
          <button
            className="text-button"
            onClick={() =>
              void act(async () => {
                await create();
              })
            }
            disabled={!ready}
          >
            Start with a note
          </button>
        </main>
      )}
      {recording && (
        <div className="recording-banner" role="status">
          <Microphone size={20} />
          <strong>
            {recordingInterrupted ? "Recording interrupted" : "Recording"}
          </strong>
          <time>{formatTime(elapsed)}</time>
          <span>Microphone + computer audio</span>
          <button onClick={() => void stopRecording()} disabled={!!busy}>
            <Stop weight="fill" size={18} />
            Stop
          </button>
        </div>
      )}
      {(error || notice) && modal !== "record" && (
        <div
          className={`toast ${error ? "error" : ""}`}
          role={error ? "alert" : "status"}
        >
          {error ? (
            <span>{error}</span>
          ) : (
            <>
              <Check size={18} />
              <span>{notice}</span>
            </>
          )}
          {saving.startsWith("Not saved") && (
            <button onClick={() => void act(async () => {})}>Retry save</button>
          )}
          <button
            className="icon-button"
            aria-label="Dismiss message"
            onClick={() => {
              setError("");
              setNotice("");
            }}
          >
            <X size={18} />
          </button>
        </div>
      )}
      <input
        className="sr-only"
        ref={file}
        type="file"
        accept="audio/*"
        tabIndex={-1}
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) void importFile(f);
          e.target.value = "";
        }}
      />
      {ready && storage.native && (
        <StartupPermissions
          calendarEnabled={preferences.calendarEnabled}
          reminderEnabled={preferences.reminderEnabled}
          blocked={
            !!modal ||
            !!busy ||
            !!recording ||
            conflict ||
            installingUpdate ||
            !!reminders.requested
          }
          onVisible={setStartupAccessOpen}
        />
      )}
      {modal === "record" && (
        <Dialog
          title={
            recordTarget
              ? `Record ${recordTarget.title}`
              : "Start a conversation"
          }
          onClose={() => {
            if (!capture.current.working) setModal(null);
          }}
        >
          {error && (
            <p className="form-message" role="alert">
              {error}
            </p>
          )}
          {recordTarget && (
            <p>
              Save this recording in <strong>{recordTarget.title}</strong>. Your
              existing notes and audio are kept.
            </p>
          )}
          {!storage.native && (
            <div className="info-box">
              Recording is available in the Mac app. You can try notes and
              import audio in this browser preview.
            </div>
          )}
          <button
            className="choice-button"
            disabled={!storage.native || !!busy}
            onClick={() =>
              void startRecording(recordTarget).catch((e) =>
                reportError(String(e)),
              )
            }
          >
            <Microphone size={28} />
            <span>
              <strong>
                {busy ? "Starting…" : "Record microphone & computer audio"}
              </strong>
              <small>
                Start only when everyone is comfortable being recorded.
              </small>
            </span>
          </button>
          {recordTarget && (
            <button
              className="text-button"
              disabled={!!busy}
              onClick={() => setRecordTarget(null)}
            >
              Record a new conversation instead
            </button>
          )}
          {!recordTarget && (
            <>
              <button
                className="choice-button"
                disabled={!!busy}
                onClick={() =>
                  void act(async () => {
                    await create();
                    setModal(null);
                  })
                }
              >
                <NotePencil size={28} />
                <span>
                  <strong>Take notes</strong>
                </span>
              </button>
              <button
                className="choice-button"
                disabled={!!busy}
                onClick={() => {
                  setModal(null);
                  void importFile();
                }}
              >
                <UploadSimple size={28} />
                <span>
                  <strong>Import a recording</strong>
                </span>
              </button>
            </>
          )}
        </Dialog>
      )}
      {modal === "settings" && (
        <Settings
          calendarEvents={events}
          reminderError={reminders.error || reminders.notificationError}
          onPreviewReminder={() => {
            const event: CalendarEvent = {
              id: "preview-reminder",
              title: "Example meeting",
              joinUrl: "https://zoom.us/j/123456789?pwd=example",
              calendar: "Example calendar",
              start: new Date(Date.now() + 300000).toISOString(),
              end: new Date(Date.now() + 3600000).toISOString(),
            };
            setReminders((state) => ({ ...state, alerts: [event] }));
            setModal(null);
          }}
          zoom={zoom}
          onZoom={setZoom}
          beforeImport={flush}
          onImported={async () => {
            const items = await storage.listMeetings();
            setMeetings(items);
            if (!selectedId)
              setSelectedId(items.find((m) => !m.archived)?.id ?? "");
          }}
          preferences={preferences}
          installing={installingUpdate}
          updates={(settingsPending) => (
            <Updates
              info={updateInfo}
              status={updateStatus}
              checking={checkingUpdate}
              installing={installingUpdate}
              blocked={!!recording || !!busy || settingsPending}
              onCheck={() => void checkUpdates()}
              onInstall={() => void installUpdate()}
            />
          )}
          onSave={setPrefs}
          onClose={() => {
            if (!installingRef.current) setModal(null);
          }}
          onConnectCalendar={async () => {
            const es = await storage.calendarEvents();
            setEvents(es);
            const p = { ...preferences, calendarEnabled: true };
            await storage.setPreferences(p);
            setPrefs(p);
          }}
        />
      )}
      {modal === "history" && (
        <Dialog title="Version history" onClose={() => setModal(null)}>
          <p className="dialog-intro">Restoring adds a new version.</p>
          <div className="version-list">
            {versions.map((v) => (
              <button
                className="version-row"
                key={v.revision}
                onClick={() =>
                  void act(async () => {
                    if (!selected) return;
                    const m = await storage.saveMeeting({
                      ...v,
                      revision:
                        meetingsRef.current.find((m) => m.id === v.id)
                          ?.revision ?? selected.revision,
                      recordings: selected.recordings,
                      archived: selected.archived,
                    });
                    replace(m);
                    setModal(null);
                    setNotice(`Restored version ${v.revision}.`);
                  })
                }
              >
                <span>
                  <strong>Version {v.revision}</strong>
                  <small>
                    {v.notes.slice(0, 70) ||
                      v.summary.slice(0, 70) ||
                      "A fresh conversation"}
                  </small>
                  {v.agentChange && <small>Saved by a local agent</small>}
                  {v.summarySource && (
                    <small>Summary: {v.summarySource.templateName}</small>
                  )}
                </span>
                <span>Restore</span>
              </button>
            ))}
          </div>
        </Dialog>
      )}
    </div>
  );
}
