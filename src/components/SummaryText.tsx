import type { Meeting } from "../lib/types";

// Only the new, versioned format interprets topic boundaries. Older summaries
// keep their exact plain text rendering, even if they happen to contain Markdown.
export function SummaryText({ meeting }: { meeting: Meeting }) {
  if (meeting.summarySource?.summaryFormatVersion !== 1)
    return <p className="summary-text">{meeting.summary}</p>;
  return meeting.summary
    .split(/(?=^### )/m)
    .filter(Boolean)
    .map((part, i) => {
      const match = /^### ([^\n]+)\n([\s\S]*)$/.exec(part);
      return match ? (
        <div className="summary-topic" key={i}>
          <h3>{match[1]}</h3>
          <p className="summary-text">{match[2].trim()}</p>
        </div>
      ) : (
        <p className="summary-text" key={i}>
          {part}
        </p>
      );
    });
}
