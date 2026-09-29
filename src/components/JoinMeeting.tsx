import { useState } from "react";
import { ArrowSquareOut } from "@phosphor-icons/react";
import { meetingLink, openMeetingLink } from "../lib/meeting-links";

export function JoinMeeting({
  url,
  title,
  compact = false,
}: {
  url?: string | null;
  title: string;
  compact?: boolean;
}) {
  const [opening, setOpening] = useState(false);
  const [message, setMessage] = useState("");
  const link = meetingLink(url);
  if (!link) return null;
  return (
    <div className={`join-meeting${compact ? " compact" : ""}`}>
      <button
        className="secondary"
        disabled={opening}
        aria-label={`Join ${title} on ${link.provider}`}
        title={`Open in ${link.provider}`}
        onClick={async () => {
          setOpening(true);
          setMessage("");
          try {
            setMessage((await openMeetingLink(link.url)) ?? "");
          } catch (error) {
            setMessage(String(error));
          } finally {
            setOpening(false);
          }
        }}
      >
        {opening ? "Opening…" : compact ? "Join" : "Join meeting"}
        <ArrowSquareOut size={16} aria-hidden="true" />
      </button>
      {message && (
        <p className="join-message" role="status">
          {message}
        </p>
      )}
    </div>
  );
}
