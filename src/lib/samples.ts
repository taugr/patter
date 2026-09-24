import type { Meeting } from "./types";
export const samples: Meeting[] = [
  {
    id: "sample-trail",
    title: "Trail Atlas ideas",
    createdAt: "2026-09-22T10:00:00+04:00",
    duration: 1934,
    notes:
      "A simpler starting point for planning a walk.\n\nKeep the map visible while comparing trails. Try the route picker on a small screen before adding more filters.",
    summary:
      "We explored how to make planning a walk feel simpler, from choosing a trail to finding the starting point.",
    decisions: [
      "Keep the map at the heart of the experience.",
      "Make nearby trails easier to compare.",
    ],
    actions: [
      { id: "a1", text: "Sketch the simpler trail view", done: false },
      { id: "a2", text: "Try the route picker on a phone", done: false },
    ],
    transcript: [
      {
        start: 0,
        text: "We explored how to make planning a walk feel simpler, from choosing a trail to finding the starting point.",
      },
      {
        start: 7,
        text: "We decided to keep the map at the heart of the experience, and make nearby trails easier to compare.",
      },
      {
        start: 14,
        text: "Next, sketch the simpler trail view and try the route picker on a phone.",
      },
    ],
    recordings: [
      {
        id: "sample-audio",
        name: "Example audio",
        url: "/sample-conversation.wav",
        track: "Example narration",
        offset: 0,
      },
    ],
    archived: false,
    revision: 0,
    sample: true,
  },
  {
    id: "sample-weekly",
    title: "Weekly catch-up",
    createdAt: "2026-09-21T11:00:00+04:00",
    duration: 1680,
    notes: "",
    summary:
      "A little time to compare progress, talk through the open questions, and choose what comes next.",
    decisions: ["Focus on one small, useful improvement at a time."],
    actions: [{ id: "w1", text: "Write down the next milestone", done: false }],
    transcript: [],
    recordings: [],
    archived: false,
    revision: 0,
    sample: true,
  },
  {
    id: "sample-project",
    title: "A new project",
    createdAt: "2026-09-18T14:00:00+04:00",
    duration: 2760,
    notes: "",
    summary:
      "An early conversation about a personal meeting notebook: simple to use, local by default, and a place to keep every conversation.",
    decisions: ["Start with the Mac app."],
    actions: [],
    transcript: [],
    recordings: [],
    archived: false,
    revision: 0,
    sample: true,
  },
];
