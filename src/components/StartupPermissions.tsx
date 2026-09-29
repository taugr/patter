import { useEffect, useMemo, useRef, useState } from "react";
import { Dialog } from "./Dialog";
import {
  accessService,
  missingAccess,
  type AccessService,
  type AccessSnapshot,
  type PermissionId,
} from "../lib/startup-permissions";

export function StartupPermissions({
  calendarEnabled,
  reminderEnabled,
  blocked,
  onVisible,
  service: supplied,
}: {
  calendarEnabled: boolean;
  reminderEnabled: boolean;
  blocked: boolean;
  onVisible: (visible: boolean) => void;
  service?: AccessService;
}) {
  const service = useMemo(
    () => supplied ?? accessService(calendarEnabled, reminderEnabled),
    [supplied, calendarEnabled, reminderEnabled],
  );
  const [snapshot, setSnapshot] = useState<AccessSnapshot | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const working = useRef(false);
  const mounted = useRef(false);
  const rows = snapshot ? missingAccess(snapshot) : [];
  const visible = !dismissed && !blocked && (rows.length > 0 || !!message);
  useEffect(() => {
    onVisible(visible);
    return () => onVisible(false);
  }, [visible, onVisible]);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useEffect(() => {
    if (dismissed) return;
    let disposed = false;
    const refresh = async () => {
      if (working.current) return;
      try {
        const next = await service.check();
        if (disposed || working.current) return;
        setSnapshot(next);
        setMessage("");
        if (!next || !missingAccess(next).length) setDismissed(true);
      } catch (error) {
        if (!disposed && !working.current)
          setMessage(
            `Could not check access. ${String(error).replace(/^Error:\s*/, "")}`,
          );
      }
    };
    void refresh();
    window.addEventListener("focus", refresh);
    return () => {
      disposed = true;
      window.removeEventListener("focus", refresh);
    };
  }, [service, dismissed]);

  async function act(kind: "allow" | "settings" | "check", id?: PermissionId) {
    if (working.current) return;
    working.current = true;
    setBusy(true);
    setMessage("");
    let failure = "";
    try {
      if (kind === "allow" && id) await service.allow(id);
      if (kind === "settings" && id) await service.openSettings(id);
    } catch (error) {
      failure = String(error).replace(/^Error:\s*/, "");
    }
    try {
      const next = await service.check();
      if (mounted.current) {
        setSnapshot(next);
        if (!next || !missingAccess(next).length) setDismissed(true);
      }
    } catch (error) {
      failure ||= `Could not check access. ${String(error)}`;
    }
    if (mounted.current) {
      setMessage(
        failure ||
          (kind === "settings"
            ? "Allow Patter in macOS settings, then return here. Reopen Patter if requested."
            : ""),
      );
      setBusy(false);
    }
    working.current = false;
  }
  if (!visible) return null;
  return (
    <Dialog
      title="Set up access"
      onClose={() => {
        if (!working.current) setDismissed(true);
      }}
    >
      <p className="dialog-intro">
        Allow access for the features you use. You can keep taking notes without
        it.
      </p>
      <div className="startup-permissions">
        {rows.map((row) => (
          <div className="startup-permission" key={row.id}>
            <div>
              <strong>{row.label}</strong>
              <p>
                {row.action === "restricted"
                  ? "Restricted by macOS or your administrator."
                  : row.detail}
              </p>
            </div>
            <div className="startup-permission-actions">
              {row.action !== "restricted" && (
                <button
                  className="secondary"
                  disabled={busy}
                  aria-label={`${row.action === "settings" ? "Open settings for" : row.action === "check" ? "Check" : "Allow"} ${row.label}`}
                  onClick={() =>
                    void act(
                      row.action === "settings"
                        ? "settings"
                        : row.action === "check"
                          ? "check"
                          : "allow",
                      row.id,
                    )
                  }
                >
                  {row.action === "settings"
                    ? "Open settings"
                    : row.action === "check"
                      ? "Check again"
                      : "Allow"}
                </button>
              )}
              {row.id === "screen" && (
                <button
                  className="text-button"
                  disabled={busy}
                  onClick={() => void act("settings", row.id)}
                  aria-label="Open settings for Computer audio"
                >
                  Open settings
                </button>
              )}
            </div>
          </div>
        ))}
      </div>
      <p className="form-message" role="status">
        {busy ? "Waiting for macOS…" : message}
      </p>
      <div className="dialog-actions">
        <button
          className="text-button"
          disabled={busy}
          onClick={() => setDismissed(true)}
        >
          Later
        </button>
        <button
          className="secondary"
          disabled={busy}
          onClick={() => void act("check")}
        >
          Check again
        </button>
      </div>
    </Dialog>
  );
}
