import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { SummaryText } from "./SummaryText";
import { MeetingPane } from "./MeetingPane";
import { Settings } from "./Settings";
import { defaultPreferences, newMeeting, type Meeting } from "../lib/types";

const unified: Meeting = {
  ...newMeeting("Synthetic planning"),
  summary:
    "### Launch timing\nFriday is tentative.\n\n### Access\nTwo options remain open.",
  summarySource: {
    summaryFormatVersion: 1,
    templateId: "unified",
    templateName: "Summary and action items",
    instructions: "fixture",
    extraInstructions: "",
    model: "fixture",
    generatedAt: "2026-10-15T00:00:00Z",
  },
};
const noop = () => {};
function pane(meeting: Meeting) {
  return renderToStaticMarkup(
    <MeetingPane
      meeting={meeting}
      onUpdate={noop}
      onArchive={noop}
      onImport={noop}
      onHistory={noop}
      onExport={noop}
      onSummarize={noop}
      onTranscribe={noop}
      onRecord={noop}
      onResumeProcessing={noop}
      recordingDisabled={false}
      onError={noop}
      busy=""
      saving="Saved"
      activeRecording={false}
    />,
  );
}

describe("unified summary rendering", () => {
  it("renders topic headings as text without interpreting model HTML", () => {
    const html = renderToStaticMarkup(<SummaryText meeting={unified} />);
    expect(html).toContain("<h3>Launch timing</h3>");
    expect(html).toContain("<h3>Access</h3>");
    expect(html).not.toContain("###");
    const malicious = {
      ...unified,
      summary: "### <script>bad()</script>\n<img src=x onerror=bad()>",
    };
    const safe = renderToStaticMarkup(<SummaryText meeting={malicious} />);
    expect(safe).not.toContain("<script>");
    expect(safe).not.toContain("<img");
  });
  it("keeps legacy summary text and decisions readable and removes template controls", () => {
    const old = {
      ...newMeeting(),
      summary: "### Original literal text\nKeep it.",
      decisions: ["Retained decision"],
      summaryTemplate: "custom",
      summaryInstructions: "Legacy instruction",
      actions: [{ id: "old", text: "Retained task", done: true }],
    };
    const html = pane(old);
    expect(html).toContain("### Original literal text");
    expect(html).not.toContain("<h3>Original literal text");
    expect(html).toContain("Retained decision");
    expect(html).toContain("Retained task");
    expect(html).toContain("checked");
    expect(html).not.toContain("Extra instructions");
    expect(html).not.toContain("summary-options");
  });
  it("shows Summary followed by Action items, including an honest empty state", () => {
    const html = pane(unified);
    expect(html.indexOf("<h2>Summary</h2>")).toBeLessThan(
      html.indexOf("<h2>Action items</h2>"),
    );
    expect(html).toContain("No action items were agreed.");
    expect(html).not.toContain("What we decided");
    expect(html).not.toContain("Next steps");
    const actions = pane({
      ...unified,
      actions: [
        { id: "a", text: "Rae: send the draft by Friday", done: false },
      ],
    });
    expect(actions).toContain("Rae: send the draft by Friday");
    expect(actions).not.toContain("No action items were agreed.");
  });
  it("loads legacy custom settings without exposing selectors or instruction editing", () => {
    const html = renderToStaticMarkup(
      <Settings
        preferences={{
          ...defaultPreferences,
          summaryTemplate: "unknown-old",
          templateInstructions: { custom: "keep private" },
        }}
        calendarEvents={[]}
        reminderError={null}
        onPreviewReminder={noop}
        zoom={100}
        onZoom={noop}
        onSave={noop}
        onClose={noop}
        onConnectCalendar={async () => {}}
        updates={() => null}
        installing={false}
        beforeImport={async () => {}}
        onImported={async () => {}}
      />,
    );
    expect(html).toContain("Every summary groups the discussion by topic");
    expect(html).not.toContain("Edit instructions");
    expect(html).not.toContain("Reset instructions");
    expect(html).not.toContain("keep private");
    expect(html).not.toContain("<h3>Templates</h3>");
  });
});
