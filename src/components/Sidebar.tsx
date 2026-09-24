import {
  Archive,
  CalendarBlank,
  GearSix,
  MagnifyingGlass,
  Microphone,
  Note,
  ArrowLeft,
  Plus,
} from "@phosphor-icons/react";
import type { CalendarEvent, Meeting } from "../lib/types";
import { native } from "../lib/storage";
export function Sidebar({
  meetings,
  selected,
  query,
  setQuery,
  mode,
  setMode,
  onSelect,
  onRecord,
  onSettings,
  onNew,
  events,
  onEvent,
}: {
  meetings: Meeting[];
  selected?: string;
  query: string;
  setQuery: (value: string) => void;
  mode: string;
  setMode: (value: string) => void;
  onSelect: (m: Meeting) => void;
  onRecord: () => void;
  onSettings: () => void;
  onNew: () => void;
  events: CalendarEvent[];
  onEvent: (e: CalendarEvent) => void;
}) {
  return (
    <aside className="sidebar">
      <div className="brand">
        <img src="/patter-mark.png" alt="" />
        <div>
          <strong>Patter</strong>
        </div>
      </div>
      <button className="primary record-button" onClick={onRecord}>
        <Microphone size={22} />
        Record
      </button>
      <label className="search">
        <MagnifyingGlass size={22} />
        <input
          placeholder="Find a conversation"
          aria-label="Find a conversation"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <kbd>⌘ K</kbd>
      </label>
      <div className="segments" aria-label="Conversation filter">
        <button
          aria-pressed={mode === "all"}
          className={mode === "all" ? "selected" : ""}
          onClick={() => setMode("all")}
        >
          All
        </button>
        <button
          aria-pressed={mode === "today"}
          className={mode === "today" ? "selected" : ""}
          onClick={() => setMode("today")}
        >
          Today
        </button>
      </div>
      {events.length > 0 && mode !== "archive" && (
        <section className="upcoming">
          <h2>Upcoming</h2>
          {events.slice(0, 2).map((event) => (
            <button
              key={event.id}
              onClick={() => onEvent(event)}
              className="event-row"
            >
              <CalendarBlank size={23} />
              <span>{event.title}</span>
              <time>
                {new Date(event.start).toDateString() !==
                  new Date().toDateString() && (
                  <small>
                    {new Date(event.start).toLocaleDateString([], {
                      weekday: "short",
                      day: "numeric",
                      month: "short",
                    })}
                    <br />
                  </small>
                )}
                {new Date(event.start).toLocaleTimeString([], {
                  hour: "2-digit",
                  minute: "2-digit",
                })}
              </time>
            </button>
          ))}
        </section>
      )}
      <div className="list-heading">
        <h2>
          {mode === "archive"
            ? "Archived"
            : query
              ? "Search results"
              : "Recent"}
        </h2>
        <button
          className="icon-button"
          title="New note"
          aria-label="New note"
          onClick={onNew}
        >
          <Plus size={19} />
        </button>
      </div>
      <div className="conversation-list">
        {meetings.map((m) => (
          <button
            className={`conversation-row ${selected === m.id ? "active" : ""}`}
            key={m.id}
            onClick={() => onSelect(m)}
            aria-current={selected === m.id ? "page" : undefined}
          >
            <Note size={26} />
            <span>
              <strong>{m.title || "Untitled conversation"}</strong>
              <small>
                {new Date(m.createdAt).toLocaleDateString([], {
                  weekday: "short",
                  day: "numeric",
                  month: "short",
                })}
                {m.duration
                  ? ` · ${Math.max(1, Math.round(m.duration / 60))} min`
                  : ""}
              </small>
            </span>
          </button>
        ))}
        {!meetings.length && (
          <p className="list-empty">
            {query
              ? "No conversations match your search."
              : mode === "archive"
                ? "Nothing archived yet."
                : mode === "today"
                  ? "A fresh page for today."
                  : "Your conversations will appear here."}
          </p>
        )}
      </div>
      <footer className="sidebar-footer">
        <button
          className={
            mode === "archive" ? "footer-button chosen" : "footer-button"
          }
          onClick={() => setMode(mode === "archive" ? "all" : "archive")}
        >
          {mode === "archive" ? <ArrowLeft size={22} /> : <Archive size={22} />}{" "}
          {mode === "archive" ? "Back to conversations" : "Archive"}
        </button>
        <button className="footer-button" onClick={onSettings}>
          <GearSix size={23} />
          Settings
        </button>
        {!native && (
          <span className="preview-label">
            Browser preview · sample library
          </span>
        )}
      </footer>
    </aside>
  );
}
