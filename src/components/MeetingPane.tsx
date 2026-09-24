import { useEffect, useState } from "react";
import {
  Archive,
  ArrowCounterClockwise,
  ClockCounterClockwise,
  DownloadSimple,
  Sparkle,
  UploadSimple,
  FileText,
  CircleNotch,
} from "@phosphor-icons/react";
import { AudioPlayer } from "./AudioPlayer";
import { SummaryOptions } from "./SummaryOptions";
import type { Meeting, Preferences } from "../lib/types";
import { formatTime } from "../lib/types";
export function MeetingPane({
  meeting,
  preferences,
  onUpdate,
  onArchive,
  onImport,
  onHistory,
  onExport,
  onSummarize,
  onTranscribe,
  onError,
  busy,
  saving,
  activeRecording,
}: {
  meeting: Meeting;
  preferences: Preferences;
  onUpdate: (m: Meeting) => void;
  onArchive: () => void;
  onImport: () => void;
  onHistory: () => void;
  onExport: () => void;
  onSummarize: () => void;
  onTranscribe: () => void;
  onError: (s: string) => void;
  busy: string;
  saving: string;
  activeRecording: boolean;
}) {
  const [tab, setTab] = useState("overview");
  const [seek, setSeek] = useState<{
    time: number;
    speaker?: string;
    key: number;
  } | null>(null);
  useEffect(() => {
    setTab(meeting.summary ? "overview" : "notes");
  }, [meeting.id]);
  return (
    <main className="meeting-pane">
      <header className="meeting-header">
        <div className="meeting-tools">
          <span className="context-label">
            {meeting.archived ? "Archived" : meeting.sample ? "Example" : ""}
          </span>
          <span className="save-state" role="status">
            {saving}
          </span>
          <div>
            {meeting.summary && (
              <button
                className="icon-button"
                disabled={!!busy}
                onClick={onSummarize}
                aria-label="Generate a new summary"
                title="Generate a new summary"
              >
                <Sparkle size={21} />
              </button>
            )}
            <button
              className="icon-button"
              disabled={!!busy}
              onClick={onHistory}
              title="Version history"
              aria-label="Version history"
            >
              <ClockCounterClockwise size={21} />
            </button>
            <button
              className="icon-button"
              onClick={onExport}
              title="Export conversation"
              aria-label="Export conversation"
            >
              <DownloadSimple size={21} />
            </button>
            <button
              className="icon-button"
              onClick={onArchive}
              disabled={activeRecording || !!busy}
              title={
                meeting.archived
                  ? "Restore conversation"
                  : "Archive conversation"
              }
              aria-label={
                meeting.archived
                  ? "Restore conversation"
                  : "Archive conversation"
              }
            >
              {meeting.archived ? (
                <ArrowCounterClockwise size={21} />
              ) : (
                <Archive size={21} />
              )}
            </button>
          </div>
        </div>
        <input
          className="meeting-title"
          aria-label="Conversation title"
          disabled={!!busy}
          value={meeting.title}
          onChange={(e) => onUpdate({ ...meeting, title: e.target.value })}
          placeholder="Untitled conversation"
        />
        <p className="meeting-date">
          {new Date(meeting.createdAt).toLocaleDateString("en-GB", {
            weekday: "short",
            day: "numeric",
            month: "long",
            year: "numeric",
          })}
          {meeting.duration > 0 && (
            <>
              {" "}
              <span>·</span> {Math.max(1, Math.round(meeting.duration / 60))}{" "}
              minutes
            </>
          )}
        </p>
      </header>
      <div role="tablist" aria-label="Conversation view" className="tabs">
        {[
          ["overview", "Overview"],
          ["notes", "Notes"],
          ["transcript", "Transcript"],
        ].map(([key, label]) => (
          <button
            key={key}
            id={`tab-${key}`}
            role="tab"
            aria-controls={`panel-${key}`}
            aria-selected={tab === key}
            tabIndex={tab === key ? 0 : -1}
            onKeyDown={(e) => {
              if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
                const keys = ["overview", "notes", "transcript"];
                const next =
                  keys[
                    (keys.indexOf(tab) + (e.key === "ArrowRight" ? 1 : 2)) % 3
                  ];
                setTab(next);
                document.getElementById(`tab-${next}`)?.focus();
              }
            }}
            className={tab === key ? "selected" : ""}
            onClick={() => setTab(key)}
          >
            {label}
          </button>
        ))}
      </div>
      <div
        key={`${meeting.id}-${tab}`}
        className="meeting-content"
        role="tabpanel"
        id={`panel-${tab}`}
        aria-labelledby={`tab-${tab}`}
        tabIndex={0}
      >
        {tab === "overview" && (
          <SummaryOptions
            meeting={meeting}
            preferences={preferences}
            onUpdate={onUpdate}
            busy={!!busy}
          />
        )}
        {tab === "overview" ? (
          meeting.summary ? (
            <>
              <section>
                <h2>Summary</h2>
                <p className="summary-text">{meeting.summary}</p>
              </section>
              {meeting.decisions.length > 0 && (
                <section>
                  <h2>What we decided</h2>
                  <ul className="decision-list">
                    {meeting.decisions.map((decision, i) => (
                      <li key={i}>{decision}</li>
                    ))}
                  </ul>
                </section>
              )}
              {meeting.actions.length > 0 && (
                <section className="next-steps">
                  <h2>Next steps</h2>
                  {meeting.actions.map((action) => (
                    <label
                      key={action.id}
                      className={`action-row ${action.done ? "done" : ""}`}
                    >
                      <input
                        type="checkbox"
                        disabled={!!busy}
                        checked={action.done}
                        onChange={() =>
                          onUpdate({
                            ...meeting,
                            actions: meeting.actions.map((a) =>
                              a.id === action.id ? { ...a, done: !a.done } : a,
                            ),
                          })
                        }
                      />
                      <span>{action.text}</span>
                    </label>
                  ))}
                </section>
              )}
            </>
          ) : (
            <div className="empty-content">
              <Sparkle size={35} />
              <h2>No summary yet</h2>
              <p>Summarize your notes or transcript with your local model.</p>
              <button
                className="secondary"
                disabled={
                  !!busy ||
                  (!meeting.notes.trim() && !meeting.transcript.length)
                }
                onClick={onSummarize}
              >
                <Sparkle size={19} />
                Create summary
              </button>
            </div>
          )
        ) : tab === "notes" ? (
          <>
            <label className="sr-only" htmlFor="notes">
              Meeting notes
            </label>
            <textarea
              id="notes"
              disabled={!!busy}
              className="notes-editor"
              placeholder="A thought, a question, something to remember…"
              value={meeting.notes}
              onChange={(e) => onUpdate({ ...meeting, notes: e.target.value })}
            />
          </>
        ) : meeting.transcript.length ? (
          <>
            <div className="section-toolbar">
              <button
                className="text-button"
                disabled={!!busy || !meeting.recordings.length}
                onClick={onTranscribe}
              >
                <ArrowCounterClockwise size={17} />
                Transcribe again
              </button>
            </div>
            <div className="transcript">
              {meeting.transcript.map((s, i) => (
                <div key={i} className="transcript-row">
                  <button
                    disabled={!meeting.recordings.length}
                    onClick={() =>
                      setSeek({
                        time: s.start,
                        speaker: s.speaker,
                        key: Date.now(),
                      })
                    }
                  >
                    {formatTime(s.start)}
                  </button>
                  <p>
                    {s.speaker && <strong>{s.speaker}</strong>}
                    {s.text}
                  </p>
                </div>
              ))}
            </div>
          </>
        ) : (
          <div className="empty-content">
            <FileText size={34} />
            <h2>No transcript yet</h2>
            <p>
              {meeting.recordings.length
                ? "Transcribe your recording with your local Whisper model."
                : "Record a conversation or import audio to create a transcript."}
            </p>
            <button
              className="secondary"
              disabled={!!busy}
              onClick={meeting.recordings.length ? onTranscribe : onImport}
            >
              {meeting.recordings.length ? (
                <>
                  <FileText size={19} />
                  Transcribe recording
                </>
              ) : (
                <>
                  <UploadSimple size={19} />
                  Import audio
                </>
              )}
            </button>
          </div>
        )}
      </div>
      <footer className="meeting-bottom">
        {busy && (
          <div className="processing" role="status">
            <CircleNotch className="spin" size={18} />
            {busy}
          </div>
        )}
        {meeting.recordings.length ? (
          <AudioPlayer
            key={meeting.id}
            recordings={meeting.recordings}
            seek={seek}
            onError={onError}
          />
        ) : (
          <button
            className="import-strip"
            onClick={onImport}
            disabled={activeRecording || !!busy}
          >
            <UploadSimple size={18} />
            <strong>Import audio</strong>
          </button>
        )}
      </footer>
    </main>
  );
}
