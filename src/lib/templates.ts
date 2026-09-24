import templates from "./summary-templates.json";
import type { Meeting, Preferences } from "./types";

export const summaryTemplates = templates;
export const instructionLimit = 4000;
export function summaryTemplate(id?: string) {
  return templates.find((template) => template.id === id) ?? templates[0];
}
export function effectiveTemplate(meeting: Meeting, preferences: Preferences) {
  return summaryTemplate(
    meeting.summaryTemplate || preferences.summaryTemplate,
  );
}
export function templateInstructions(id: string, preferences: Preferences) {
  return (
    preferences.templateInstructions?.[id] ?? summaryTemplate(id).instructions
  );
}
