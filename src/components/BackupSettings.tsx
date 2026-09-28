import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowSquareOut,
  CloudArrowUp,
  ArrowClockwise,
} from "@phosphor-icons/react";
import { native } from "../lib/storage";

type BackupState = {
  config: {
    connected: boolean;
    enabled: boolean;
    email: string;
    time: string;
    clientId: string;
  };
  status: {
    phase: string;
    message: string;
    lastSuccess: string | null;
    completedFiles: number;
    totalFiles: number;
    uploadedBytes: number;
    pending: string | null;
  };
  schedule: { installed: boolean; registered: boolean };
  timezone: string;
  running: boolean;
  restoreReady: boolean;
  needsReconnect: boolean;
};
type Snapshot = { id: string; name: string; createdAt: string };
export function BackupSettings({
  disabled,
  beforeBackup,
  onBusy,
}: {
  disabled: boolean;
  beforeBackup: () => Promise<void>;
  onBusy: (value: boolean) => void;
}) {
  const [data, setData] = useState<BackupState | null>(null);
  const [time, setTime] = useState("02:00");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [snapshots, setSnapshots] = useState<Snapshot[] | null>(null);
  const [selected, setSelected] = useState("");
  const refresh = useCallback(async () => {
    if (!native) return;
    const state = await invoke<BackupState>("backup_status");
    setData(state);
    return state;
  }, []);
  useEffect(() => {
    let active = true;
    if (native)
      void refresh()
        .then((state) => {
          if (active && state) setTime(state.config.time);
        })
        .catch((e) => {
          if (active) setMessage(String(e));
        });
    const timer = setInterval(() => {
      if (native) void refresh().catch(() => {});
    }, 3000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [refresh]);
  async function act(fn: () => Promise<void>) {
    setBusy(true);
    onBusy(true);
    setMessage("");
    try {
      await fn();
    } catch (e) {
      setMessage(String(e));
    } finally {
      await refresh().catch(() => {});
      setBusy(false);
      onBusy(false);
    }
  }
  async function connect(chooseFile: boolean) {
    let credentialsPath: string | null = null;
    if (chooseFile) {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const path = await open({
        title: "Choose Google Desktop OAuth credentials",
        filters: [{ name: "Google OAuth setup", extensions: ["json"] }],
      });
      if (typeof path !== "string") return;
      credentialsPath = path;
    }
    setMessage("Finish connecting in your browser, then return to Patter.");
    await invoke("backup_connect", { credentialsPath });
    setMessage(
      "Google Drive connected. You can back up now or enable nightly backups.",
    );
  }
  const locked = disabled || busy || Boolean(data?.running) || !native;
  return (
    <section className="settings-section drive-backup">
      <h3>Google Drive backup</h3>
      <p>
        Back up conversations, recordings, summaries and settings to your Drive.
      </p>
      {!data?.config.connected ? (
        <>
          <button
            className="secondary"
            disabled={locked}
            onClick={() => void act(() => connect(true))}
          >
            <CloudArrowUp size={18} />
            Connect Google Drive
          </button>
          <details className="backup-setup">
            <summary>Google setup</summary>
            <p>
              In Google Cloud Console, enable the Drive API and create an OAuth
              client with application type <strong>Desktop app</strong>.
              Download its JSON file, then choose it when connecting.
            </p>
            <p>
              Use the same client on each Mac so they can find your backups. For
              unattended backups, set the OAuth audience to Production; Testing
              access expires after seven days.
            </p>
            <small>
              Patter creates a private “Patter Backups” folder. Sign-in
              credentials stay in macOS Keychain.
            </small>
          </details>
        </>
      ) : (
        <>
          <div className="backup-account">
            <span>{data.config.email}</span>
            <button
              className="text-button"
              disabled={locked}
              onClick={() =>
                void act(async () => {
                  await invoke("backup_open_folder");
                })
              }
            >
              Open Drive folder <ArrowSquareOut size={15} />
            </button>
          </div>
          {data.needsReconnect && (
            <div className="backup-restore" role="alert">
              <p>
                Google Drive needs reconnecting. This can happen after a Patter
                update or when your login Keychain is locked. Unlock your
                Keychain, or select your Google setup JSON and sign in again.
                Your existing backups and schedule are kept.
              </p>
              <button
                className="primary"
                disabled={locked}
                onClick={() => void act(() => connect(true))}
              >
                Reconnect Google Drive
              </button>
            </div>
          )}
          <div className="backup-schedule">
            <label>
              Nightly time
              <input
                type="time"
                value={time}
                onChange={(e) => setTime(e.target.value)}
                disabled={locked}
              />
            </label>
            <button
              className="secondary"
              disabled={locked || !time}
              onClick={() =>
                void act(async () => {
                  await invoke("backup_configure", { enabled: true, time });
                  setMessage(`Nightly backups enabled for ${time}.`);
                })
              }
            >
              {data.config.enabled ? "Save time" : "Enable nightly backups"}
            </button>
            {data.config.enabled && (
              <button
                className="text-button"
                disabled={locked}
                onClick={() =>
                  void act(async () => {
                    await invoke("backup_configure", { enabled: false, time });
                    setMessage(
                      "Nightly backups paused. Your backups remain in Drive.",
                    );
                  })
                }
              >
                Pause
              </button>
            )}
          </div>
          <small>
            {data.config.enabled
              ? `Scheduled daily at ${data.config.time}`
              : "Nightly backups are paused"}{" "}
            · Mac time ({data.timezone}). Runs with Patter closed and catches up
            after sleep or your next login.
          </small>
          {data.config.enabled && !data.schedule.registered && (
            <p className="form-message" role="alert">
              The macOS schedule is not running. Choose Save time to register it
              again.
            </p>
          )}
          <div className="backup-status" aria-live="polite">
            <strong>
              {data.running
                ? "Backup in progress…"
                : data.status.lastSuccess
                  ? `Last verified backup: ${new Date(data.status.lastSuccess).toLocaleString()}`
                  : "No verified backup yet"}
            </strong>
            {data.running && data.status.totalFiles > 0 && (
              <progress
                max={data.status.totalFiles}
                value={data.status.completedFiles}
                aria-label="Backup files verified"
              />
            )}
            {data.status.message && <span>{data.status.message}</span>}
            {data.status.phase === "error" &&
              data.config.enabled &&
              !data.needsReconnect && (
                <small>
                  Patter will retry. If sign-in needs attention, use Reconnect
                  below.
                </small>
              )}
          </div>
          <div className="backup-actions">
            <button
              className="secondary"
              disabled={locked || data.restoreReady || data.needsReconnect}
              onClick={() =>
                void act(async () => {
                  await beforeBackup();
                  await invoke("backup_now");
                  setMessage("Backup verified in Google Drive.");
                })
              }
            >
              <CloudArrowUp size={18} />
              {data.status.pending ? "Resume backup" : "Back up now"}
            </button>
            <button
              className="text-button"
              disabled={locked || data.restoreReady || data.needsReconnect}
              onClick={() =>
                void act(async () => {
                  const rows = await invoke<Snapshot[]>(
                    "backup_list_snapshots",
                  );
                  setSnapshots(rows);
                  setSelected(rows[0]?.id ?? "");
                  if (!rows.length)
                    setMessage(
                      "No completed backups were found for this Google account and OAuth app.",
                    );
                })
              }
            >
              Restore a backup
            </button>
          </div>
          {snapshots && snapshots.length > 0 && !data.restoreReady && (
            <div className="backup-restore">
              <label>
                Backup to restore
                <select
                  value={selected}
                  disabled={locked}
                  onChange={(e) => setSelected(e.target.value)}
                >
                  {snapshots.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name}
                    </option>
                  ))}
                </select>
              </label>
              <p>
                Patter will download and verify this backup. After a restart, it
                becomes your library. Your current library stays in a separate
                safety folder. This does not merge libraries.
              </p>
              <button
                className="secondary"
                disabled={locked || !selected}
                onClick={() =>
                  void act(async () => {
                    await beforeBackup();
                    setMessage(
                      "Downloading and checking your backup. This may take a while…",
                    );
                    setMessage(
                      await invoke<string>("backup_prepare_restore", {
                        snapshotId: selected,
                      }),
                    );
                  })
                }
              >
                Download and verify restore
              </button>
            </div>
          )}
          <details>
            <summary>Connection</summary>
            <div className="backup-actions">
              <button
                className="text-button"
                disabled={locked}
                onClick={() =>
                  void act(() => connect(Boolean(data.needsReconnect)))
                }
              >
                Reconnect
              </button>
              <button
                className="text-button"
                disabled={locked}
                onClick={() => void act(() => connect(true))}
              >
                Change OAuth setup file
              </button>
              <button
                className="text-button"
                disabled={locked}
                onClick={() =>
                  void act(async () => {
                    await invoke("backup_disconnect");
                    setSnapshots(null);
                    setMessage(
                      "Disconnected. All backups remain in Google Drive.",
                    );
                  })
                }
              >
                Disconnect
              </button>
            </div>
          </details>
        </>
      )}
      {data?.restoreReady && (
        <div className="backup-restore">
          <p>
            Your restore has been verified. Restart to open it. Reconnect Google
            Drive afterward.
          </p>
          <button
            className="primary"
            disabled={locked}
            onClick={() =>
              void act(async () => {
                await beforeBackup();
                await invoke("backup_restart");
              })
            }
          >
            <ArrowClockwise size={18} />
            Restart and open restored library
          </button>
        </div>
      )}
      <small>
        Files are readable in Drive; previous backups are kept. Models and
        passwords are excluded. After restoring, download models and reconnect
        accounts.
      </small>
      {message && (
        <p className="form-message" role="status">
          {message}
        </p>
      )}
    </section>
  );
}
