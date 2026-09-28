import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FolderOpen } from "@phosphor-icons/react";
import { native } from "../lib/storage";

type Preview = {
  fingerprint: string;
  sessions: { title: string; recordings: number; alreadyImported: boolean }[];
  warnings: string[];
};
export function AnarlogImport({
  disabled,
  onBusy,
  beforeImport,
  onImported,
}: {
  disabled: boolean;
  onBusy: (busy: boolean) => void;
  beforeImport: () => Promise<void>;
  onImported: () => Promise<void>;
}) {
  const [path, setPath] = useState("");
  const [preview, setPreview] = useState<Preview | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const count = preview?.sessions.filter((s) => !s.alreadyImported).length ?? 0;
  async function act(fn: () => Promise<void>) {
    setBusy(true);
    onBusy(true);
    setMessage("");
    try {
      await fn();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
      onBusy(false);
    }
  }
  return (
    <section className="settings-section anarlog-import">
      <h3>Import from Anarlog</h3>
      <p>
        Import conversations, notes, summaries and audio from Anarlog 1.0.27.
      </p>
      <details>
        <summary>Moving from another Mac</summary>
        <p>
          In Anarlog, open Settings → Storage and find the Content folder. Quit
          Anarlog, then copy that folder to this Mac using AirDrop or an
          external drive. The default is ~/Library/Application Support/hyprnote.
        </p>
        <p>
          Choose the copied folder below. Your originals stay untouched. Custom
          templates, app settings and contacts are not imported; original
          session files and attachments are preserved with each conversation.
        </p>
      </details>
      <button
        className="secondary"
        disabled={disabled || busy || !native}
        onClick={() =>
          void act(async () => {
            const { open } = await import("@tauri-apps/plugin-dialog");
            const selected = await open({
              directory: true,
              title: "Choose your copied Anarlog Content folder",
            });
            if (typeof selected !== "string") return;
            setPreview(null);
            setPath(selected);
            setMessage("Reading conversations and checking files…");
            const result = await invoke<Preview>("preview_anarlog", {
              path: selected,
            });
            setPreview(result);
            setMessage("");
          })
        }
      >
        <FolderOpen size={18} />
        Choose Anarlog folder
      </button>
      {preview && (
        <div className="import-preview">
          <p>
            <strong>
              {count} {count === 1 ? "conversation" : "conversations"} to import
            </strong>{" "}
            · {preview.sessions.filter((s) => s.alreadyImported).length} already
            imported
          </p>
          <small className="import-path">{path}</small>
          <ul className="import-session-list">
            {preview.sessions.map((s, i) => (
              <li key={i}>
                <span>{s.title}</span>
                <small>
                  {s.alreadyImported
                    ? "Already imported"
                    : `${s.recordings} audio ${s.recordings === 1 ? "file" : "files"}`}
                </small>
              </li>
            ))}
          </ul>
          {preview.warnings.length > 0 && (
            <details>
              <summary>
                {preview.warnings.length} import{" "}
                {preview.warnings.length === 1 ? "note" : "notes"}
              </summary>
              <ul>
                {preview.warnings.map((w, i) => (
                  <li key={i}>{w}</li>
                ))}
              </ul>
            </details>
          )}
          <small>
            Changed source conversations are added as separate copies. Existing
            Patter conversations keep your edits.
          </small>
          <button
            className="primary"
            disabled={disabled || busy || count === 0}
            onClick={() =>
              void act(async () => {
                await beforeImport();
                setMessage("Copying conversations and recordings…");
                const result = await invoke<{
                  imported: number;
                  skipped: number;
                }>("import_anarlog", {
                  path,
                  fingerprint: preview.fingerprint,
                });
                setPreview(null);
                setMessage(
                  `Imported ${result.imported} conversations. Skipped ${result.skipped} already imported. Your original files are unchanged.`,
                );
                await onImported();
              })
            }
          >
            {busy ? "Importing…" : "Import conversations"}
          </button>
        </div>
      )}
      {message && (
        <p className="form-message" role="status">
          {message}
        </p>
      )}
    </section>
  );
}
