import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { native } from "../lib/storage";

type Config = {
  enabled: boolean;
  allowEdit: boolean;
  allowProcessing: boolean;
};
type AgentState = {
  error?: string;
  config: Config;
  connection: unknown;
  recent: {
    tool: string;
    conversationId: string;
    outcome: string;
    at: string;
  }[];
  jobs: {
    id: string;
    kind: string;
    status: string;
    stage: string;
    error?: string;
  }[];
};
const off: Config = {
  enabled: false,
  allowEdit: false,
  allowProcessing: false,
};
export function AgentSettings({ disabled }: { disabled: boolean }) {
  const [state, setState] = useState<AgentState | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  useEffect(() => {
    if (!native) return;
    let disposed = false;
    const refresh = () =>
      invoke<AgentState>("agent_status")
        .then((s) => {
          if (!disposed) setState(s);
        })
        .catch((e) => {
          if (!disposed) setMessage(String(e));
        });
    void refresh();
    const timer = setInterval(() => void refresh(), 3000);
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  }, []);
  const config = state?.config ?? off;
  async function configure(next: Config) {
    setBusy(true);
    setMessage("");
    try {
      await invoke("agent_configure", { config: next });
      setState(await invoke<AgentState>("agent_status"));
      setMessage(
        next.enabled ? "Agent permissions saved." : "Agent access turned off.",
      );
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="settings-section agent-settings">
      <p>Connect Codex, Claude or another MCP agent while Patter is open.</p>
      <p className="agent-privacy">
        Cloud agents may send library text to their provider. These permissions
        apply to every connected agent.
      </p>
      <label className="agent-option">
        <input
          type="checkbox"
          checked={config.enabled}
          disabled={!native || !state || !!state.error || busy || disabled}
          onChange={(e) =>
            void configure({ ...config, enabled: e.target.checked })
          }
        />
        Read entire library
      </label>
      {state?.error && (
        <p role="alert">Agent access is unavailable: {state.error}</p>
      )}
      {config.enabled && (
        <>
          <label className="agent-option">
            <input
              type="checkbox"
              checked={config.allowEdit}
              disabled={busy || disabled}
              onChange={(e) =>
                void configure({ ...config, allowEdit: e.target.checked })
              }
            />
            Edit notes, titles and actions
          </label>
          <label className="agent-option">
            <input
              type="checkbox"
              checked={config.allowProcessing}
              disabled={busy || disabled}
              onChange={(e) =>
                void configure({ ...config, allowProcessing: e.target.checked })
              }
            />
            Transcribe and summarize
          </label>
          <details>
            <summary>Connection setup</summary>
            <p>
              Merge this entry into your agent’s MCP settings. For Claude
              Desktop, use Settings → Developer → Edit Config, then restart
              Claude.
            </p>
            <pre className="agent-config">
              {JSON.stringify(state?.connection, null, 2)}
            </pre>
            <button
              type="button"
              className="secondary"
              onClick={() => {
                void navigator.clipboard
                  .writeText(JSON.stringify(state?.connection, null, 2))
                  .then(() => setMessage("Settings copied."))
                  .catch(() =>
                    setMessage("Select and copy the settings above."),
                  );
              }}
            >
              Copy settings
            </button>
          </details>
          <details className="settings-help">
            <summary>How permissions work</summary>
            <p>
              Changes apply immediately and prevent running jobs from saving.
              Saved edits stay in version history. Patter’s transcription and
              summaries run locally.
            </p>
          </details>
        </>
      )}
      {!!state?.jobs.length && (
        <div aria-live="polite">
          {state.jobs.slice(-5).map((job) => (
            <p key={job.id}>
              {job.kind === "summary" ? "Summary" : "Transcription"}:{" "}
              {job.status}
              {job.error ? ` — ${job.error}` : ""}
            </p>
          ))}
        </div>
      )}
      {!!state?.recent.length && (
        <details>
          <summary>Recent activity</summary>
          <ul>
            {state.recent.slice(0, 10).map((event, i) => (
              <li key={`${event.at}-${i}`}>
                {event.tool === "edit_conversation"
                  ? "Conversation edit"
                  : "Local processing"}{" "}
                · {event.outcome} · {new Date(event.at).toLocaleTimeString()}
              </li>
            ))}
          </ul>
          <small>
            This session only. Saved edits also appear in version history.
          </small>
        </details>
      )}
      {message && (
        <p className="form-message" role="status">
          {message}
        </p>
      )}
    </section>
  );
}
