import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CloudWarning } from "@phosphor-icons/react";
import { native } from "../lib/storage";
export function BackupHealth({ onOpen }: { onOpen: () => void }) {
  const [attention, setAttention] = useState(false);
  useEffect(() => {
    if (!native) return;
    let active = true;
    async function check() {
      try {
        const s = await invoke<{
          config: { connected: boolean; enabled: boolean };
          status: { phase: string; lastSuccess: string | null };
          schedule: { registered: boolean };
          running: boolean;
          needsReconnect: boolean;
        }>("backup_status");
        const overdue =
          s.config.enabled &&
          (!s.status.lastSuccess ||
            Date.now() - new Date(s.status.lastSuccess).getTime() >
              48 * 60 * 60 * 1000);
        if (active)
          setAttention(
            s.config.connected &&
              !s.running &&
              (s.needsReconnect ||
                s.status.phase === "error" ||
                overdue ||
                (s.config.enabled && !s.schedule.registered)),
          );
      } catch {
        /* Connection errors do not replace the user's current task. */
      }
    }
    void check();
    const timer = setInterval(() => void check(), 60000);
    window.addEventListener("focus", check);
    return () => {
      active = false;
      clearInterval(timer);
      window.removeEventListener("focus", check);
    };
  }, []);
  return attention ? (
    <button className="footer-button backup-attention" onClick={onOpen}>
      <CloudWarning size={22} />
      Backup needs attention
    </button>
  ) : null;
}
