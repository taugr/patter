import type { Meeting, Preferences } from "../lib/types";
import {
  effectiveTemplate,
  instructionLimit,
  summaryTemplate,
  summaryTemplates,
  templateInstructions,
} from "../lib/templates";

export function SummaryOptions({
  meeting,
  preferences,
  onUpdate,
  busy,
}: {
  meeting: Meeting;
  preferences: Preferences;
  onUpdate: (meeting: Meeting) => void;
  busy: boolean;
}) {
  const template = effectiveTemplate(meeting, preferences);
  return (
    <details className="summary-options">
      <summary>
        Template <span>{template.name}</span>
      </summary>
      <fieldset disabled={busy}>
        <label>
          Template
          <select
            value={meeting.summaryTemplate || ""}
            onChange={(e) =>
              onUpdate({ ...meeting, summaryTemplate: e.target.value })
            }
          >
            <option value="">
              Default ({summaryTemplate(preferences.summaryTemplate).name})
            </option>
            {summaryTemplates.map((t) => (
              <option key={t.id} value={t.id}>
                {t.name}
              </option>
            ))}
          </select>
        </label>
        <p>{templateInstructions(template.id, preferences)}</p>
        <label>
          Extra instructions
          <textarea
            rows={3}
            maxLength={instructionLimit}
            value={meeting.summaryInstructions ?? ""}
            placeholder="Anything to focus on for this conversation?"
            onChange={(e) =>
              onUpdate({ ...meeting, summaryInstructions: e.target.value })
            }
          />
        </label>
        <small>Applies to the next summary.</small>
        {meeting.summarySource && (
          <small>
            Last generated with {meeting.summarySource.templateName}.
          </small>
        )}
      </fieldset>
    </details>
  );
}
