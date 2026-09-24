import { ArrowClockwise, DownloadSimple } from "@phosphor-icons/react";
import { native } from "../lib/storage";
export type UpdateInfo = {
  currentVersion: string;
  update: { version: string; notes: string | null } | null;
};
export function Updates({
  info,
  status,
  checking,
  installing,
  blocked,
  onCheck,
  onInstall,
}: {
  info: UpdateInfo;
  status: string;
  checking: boolean;
  installing: boolean;
  blocked: boolean;
  onCheck: () => void;
  onInstall: () => void;
}) {
  return (
    <section className="settings-section" id="updates">
      <h3>Updates</h3>
      <p>
        Patter {info.currentVersion}
        {!native && " · Browser preview"}
      </p>
      {info.update && (
        <>
          <p>
            <strong>Version {info.update.version} is available.</strong>
          </p>
          {info.update.notes && (
            <p className="release-notes">{info.update.notes}</p>
          )}
        </>
      )}
      <div className="update-actions">
        {info.update && (
          <button
            className="primary"
            disabled={blocked || installing || checking}
            onClick={onInstall}
          >
            <DownloadSimple size={18} />
            Install and restart
          </button>
        )}
        <button
          className="secondary"
          disabled={!native || checking || installing}
          onClick={onCheck}
        >
          <ArrowClockwise size={18} />
          {checking ? "Checking…" : "Check for updates"}
        </button>
      </div>
      {blocked && info.update && (
        <small>
          Save settings and finish recording or processing before installing.
        </small>
      )}
      {!native && <small>Available in the Mac app.</small>}
      <p className="update-status" role="status">
        {status}
      </p>
    </section>
  );
}
