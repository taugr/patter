import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DownloadSimple, FolderOpen } from "@phosphor-icons/react";
import { native } from "../lib/storage";
import type { Preferences } from "../lib/types";
import catalog from "../../src-tauri/transcription-models.json";

type Model = {
  id: string;
  name: string;
  description: string;
  size: number;
  installed: boolean;
  license: string;
};
type Progress = {
  id: string;
  downloaded: number;
  total: number;
  phase: string;
};
type Status = { models: Model[]; download: Progress | null };
const preview: Status = {
  models: catalog.map((m) => ({
    id: m.id,
    name: m.name,
    description: m.description,
    size: m.files.reduce((s, f) => s + f.size, 0),
    installed: false,
    license: m.license,
  })),
  download: null,
};
const megabytes = (bytes: number) => `${Math.round(bytes / 1_000_000)} MB`;

export function TranscriptionSettings({
  draft,
  onChange,
  onBusy,
}: {
  draft: Preferences;
  onChange: (draft: Preferences) => void;
  onBusy: (busy: boolean) => void;
}) {
  const [status, setStatus] = useState(preview);
  const [message, setMessage] = useState("");
  const [starting, setStarting] = useState(false);
  const active = !!status.download || starting;
  useEffect(() => {
    onBusy(active);
  }, [active, onBusy]);
  useEffect(() => {
    if (!native) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const next = await invoke<Status>("transcription_models");
        if (!stopped) setStatus(next);
      } catch (e) {
        if (!stopped) setMessage(String(e));
      }
      if (!stopped) timer = setTimeout(refresh, 1500);
    }
    void refresh();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, []);
  const selected = status.models.find((m) => m.id === draft.transcriptionModel);
  async function download() {
    if (!selected) return;
    setStarting(true);
    setMessage("");
    try {
      await invoke("download_transcription_model", { id: selected.id });
      setMessage(`${selected.name} is ready. Save to use your selected model.`);
    } catch (e) {
      setMessage(String(e));
    } finally {
      try {
        setStatus(await invoke<Status>("transcription_models"));
      } catch (e) {
        setMessage(String(e));
      }
      setStarting(false);
    }
  }
  return (
    <section className="settings-section">
      <h3>Transcription</h3>
      <p>
        Download a model once, then transcribe on your Mac. Your recordings stay
        local.
      </p>
      <label>
        Transcription model
        <select
          value={draft.transcriptionModel}
          onChange={(e) => {
            onChange({ ...draft, transcriptionModel: e.target.value });
            setMessage("");
          }}
        >
          {status.models.map((m) => (
            <option value={m.id} key={m.id}>
              {m.name}
            </option>
          ))}
          <option value="whisper-file">Whisper — choose a file</option>
        </select>
      </label>
      {selected ? (
        <>
          <p>{selected.description}</p>
          <div className="model-download-row">
            <span>
              {megabytes(selected.size)} ·{" "}
              {selected.installed ? "Downloaded" : "Not downloaded"}
            </span>
            <button
              type="button"
              className="secondary"
              disabled={!native || active}
              onClick={() => void download()}
            >
              <DownloadSimple size={18} />
              {selected.installed ? "Verify / repair" : "Download model"}
            </button>
          </div>
          <small>
            From Hugging Face · {selected.license}. Stored in Patter’s local
            models folder.
          </small>
        </>
      ) : (
        <>
          <button
            type="button"
            className="secondary"
            disabled={!native}
            onClick={async () => {
              try {
                const { open } = await import("@tauri-apps/plugin-dialog");
                const path = await open({
                  multiple: false,
                  filters: [{ name: "Whisper model", extensions: ["bin"] }],
                });
                if (typeof path === "string")
                  onChange({ ...draft, whisperModel: path });
              } catch (e) {
                setMessage(String(e));
              }
            }}
          >
            <FolderOpen size={18} />
            {draft.whisperModel ? "Change model file" : "Choose model file"}
          </button>
          {draft.whisperModel && (
            <small>{draft.whisperModel.split("/").at(-1)}</small>
          )}
        </>
      )}
      {status.download && (
        <div className="model-progress" role="status">
          <span>
            {status.models.find((m) => m.id === status.download!.id)?.name} ·{" "}
            {status.download.phase}
          </span>
          <progress
            aria-label="Model download progress"
            max={status.download.total}
            value={status.download.downloaded}
          />
          <div>
            <small>
              {megabytes(status.download.downloaded)} of{" "}
              {megabytes(status.download.total)}
            </small>
            <button
              type="button"
              className="text-button"
              onClick={() => {
                void invoke("cancel_model_download")
                  .then(() => setMessage("Cancelling download…"))
                  .catch((e) => setMessage(String(e)));
              }}
            >
              Cancel download
            </button>
          </div>
        </div>
      )}
      {!native && <small>Model downloads are available in the Mac app.</small>}
      {message && (
        <p className="form-message" role="status">
          {message}
        </p>
      )}
    </section>
  );
}
