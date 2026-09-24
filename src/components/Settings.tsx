import { type ReactNode, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  FolderOpen,
  Check,
  DownloadSimple,
  ArrowSquareOut,
} from "@phosphor-icons/react";
import { Dialog } from "./Dialog";
import {
  modelList,
  native,
  setPreferences,
  downloadJson,
  listMeetings,
} from "../lib/storage";
import type { Preferences } from "../lib/types";
export function Settings({
  preferences,
  onSave,
  onClose,
  onConnectCalendar,
  updates,
  installing,
}: {
  updates: ReactNode;
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
    <Dialog title="Settings" onClose={onClose}>
      {updates}
      <fieldset className="settings-fields" disabled={installing}>
        <section className="settings-section">
          <h3>Calendar</h3>
          <p>Use calendars connected to your Mac, including Google Calendar.</p>
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
            Connect Mac calendar
            <ArrowSquareOut size={16} />
          </button>
          {!native && <small>Available in the Mac app.</small>}
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
                placeholder="Choose or enter a model"
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
                      ? `${found.length} models found. Choose one in the model field.`
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
          <h3>Transcription</h3>
          <p>Choose a local Whisper model (.bin).</p>
          <button
            className="secondary"
            disabled={busy || !native}
            onClick={() =>
              act(async () => {
                const { open } = await import("@tauri-apps/plugin-dialog");
                const path = await open({
                  filters: [{ name: "Whisper model", extensions: ["bin"] }],
                });
                if (typeof path === "string")
                  setDraft({ ...draft, whisperModel: path });
              })
            }
          >
            <FolderOpen size={18} />
            {draft.whisperModel ? "Change model" : "Choose model file"}
          </button>
          {draft.whisperModel && (
            <small className="file-path">
              {draft.whisperModel.split("/").at(-1)}
            </small>
          )}
        </section>
        <section className="settings-section">
          <h3>Library</h3>
          <p>
            Conversations, audio and previous versions are kept, even when
            archived.
          </p>
          <button
            className="secondary"
            disabled={busy}
            onClick={() =>
              act(async () => {
                if (native) {
                  const { open } = await import("@tauri-apps/plugin-dialog");
                  const path = await open({ directory: true });
                  if (typeof path === "string") {
                    const saved = await invoke<string>("backup_library", {
                      destination: path,
                    });
                    setMessage(`Complete library backup saved to ${saved}`);
                  }
                } else {
                  downloadJson(
                    "patter-preview-notes.json",
                    await listMeetings(),
                  );
                  setMessage(
                    "Notes exported. This preview export does not include audio files.",
                  );
                }
              })
            }
          >
            <DownloadSimple size={18} />
            {native ? "Back up library" : "Export preview notes"}
          </button>
        </section>
        {message && (
          <p className="form-message" role="status">
            {message}
          </p>
        )}
        <div className="dialog-actions">
          <button className="text-button" onClick={onClose}>
            Close
          </button>
          <button className="primary" disabled={busy} onClick={() => act(save)}>
            <Check size={18} />
            Save
          </button>
        </div>
      </fieldset>
    </Dialog>
  );
}
