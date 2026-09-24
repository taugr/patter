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
import { Sidebar } from "./components/Sidebar";
import { MeetingPane } from "./components/MeetingPane";
import { Dialog } from "./components/Dialog";
import { Updates, type UpdateInfo } from "./components/Updates";
import { version as appVersion } from "../package.json";
import { Settings } from "./components/Settings";
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
          `Patter ${info.update.version} is available in Settings → Updates.`,
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
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [modal, setModal] = useState<"record" | "settings" | "history" | null>(
    null,
  );
  const [versions, setVersions] = useState<Meeting[]>([]);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState("");
  const [saving, setSaving] = useState("");
  const [recording, setRecording] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [ready, setReady] = useState(false);
  const [detailOpen, setDetailOpen] = useState(false);
  const inFlight = useRef<Promise<void> | null>(null);
  const recordingRef = useRef(recording);
  recordingRef.current = recording;
  const pending = useRef<Meeting | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const file = useRef<HTMLInputElement>(null);
  const meetingsRef = useRef(meetings);
  meetingsRef.current = meetings;
  const selected = meetings.find((m) => m.id === selectedId);
  const reportError = useCallback(
    (message: string) => setError(message.replace(/^Error:\s*/, "")),
    [],
  );
  function replace(m: Meeting) {
    setMeetings((ms) =>
      [m, ...ms.filter((x) => x.id !== m.id)].sort((a, b) =>
        b.createdAt.localeCompare(a.createdAt),
      ),
    );
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
        setMeetings((ms) =>
          ms.map((x) =>
            x.id === saved.id ? { ...x, revision: saved.revision } : x,
          ),
        );
        if (!pending.current) setSaving("");
      } catch (e) {
        if (!pending.current) pending.current = m;
        setSaving("Not saved — retry needed");
        reportError(`Your changes could not be saved: ${String(e)}`);
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
  useEffect(() => {
    if (!storage.native) return;
    let disposed = false;
    const unlisteners: (() => void)[] = [];
    const close = async () => {
      if (installingRef.current) {
        reportError("Wait for the update to finish before closing Patter.");
        return;
      }
      if (recordingRef.current) {
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
        if (storage.native && p.calendarEnabled) {
          try {
            setEvents(await storage.calendarEvents());
          } catch (e) {
            reportError(String(e));
          }
        }
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
    const started = Date.now();
    const id = setInterval(() => {
      setElapsed((Date.now() - started) / 1000);
      void storage
        .recordingStatus()
        .then((s) => {
          if (s.error) {
            reportError(s.error);
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
          start: "2026-09-23T14:00:00+04:00",
          end: "2026-09-23T14:30:00+04:00",
          calendar: "Example calendar",
        },
      ];
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
        onRecord={() => {
          if (recording) {
            setSelectedId(recording);
          } else setModal("record");
        }}
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
            const existing = meetings.find((m) => m.eventId === event.id);
            if (existing) {
              await choose(existing);
            } else {
              const m = await create(event.title);
              const saved = await storage.saveMeeting({
                ...m,
                eventId: event.id,
                notes: `${event.calendar}\n${new Date(event.start).toLocaleString()}${event.url ? `\n${event.url}` : ""}`,
              });
              replace(saved);
            }
          })
        }
      />
      <button className="mobile-back" onClick={() => setDetailOpen(false)}>
        <ArrowLeft size={18} />
        Conversations
      </button>
      {selected ? (
        <MeetingPane
          meeting={selected}
          preferences={preferences}
          onUpdate={update}
          saving={saving}
          activeRecording={recording === selected.id}
          busy={busy}
          onArchive={() =>
            void act(async () => {
              const archived = !selected.archived;
              const m = await storage.saveMeeting({ ...selected, archived });
              replace(m);
              setMode(archived ? "archive" : "all");
              setNotice(
                archived ? "Conversation archived." : "Conversation restored.",
              );
            })
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
            onClick={() => setModal("record")}
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
          <strong>Recording</strong>
          <time>{formatTime(elapsed)}</time>
          <span>Microphone + computer audio</span>
          <button
            onClick={() =>
              void act(async () => {
                setBusy("Finishing your recording");
                try {
                  const m = await storage.stopRecording();
                  replace(m);
                  setRecording(null);
                  setNotice("Recording saved.");
                } finally {
                  setRecording(null);
                  setBusy("");
                }
              })
            }
            disabled={!!busy}
          >
            <Stop weight="fill" size={18} />
            Stop
          </button>
        </div>
      )}
      {(error || notice) && (
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
      {modal === "record" && (
        <Dialog title="Start a conversation" onClose={() => setModal(null)}>
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
              void act(async () => {
                setBusy("Starting recording");
                try {
                  const m = await create("New conversation");
                  await storage.startRecording(m.id);
                  setRecording(m.id);
                  setModal(null);
                } finally {
                  setBusy("");
                }
              })
            }
          >
            <Microphone size={28} />
            <span>
              <strong>Record microphone & computer audio</strong>
              <small>
                macOS will ask for permission. Start only when everyone is
                comfortable being recorded.
              </small>
            </span>
          </button>
          <button
            className="choice-button"
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
        </Dialog>
      )}
      {modal === "settings" && (
        <Settings
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
