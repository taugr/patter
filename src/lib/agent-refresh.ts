import type { Meeting } from "./types";
// A remote refresh must never replace an unsaved local draft or a newer local save.
export function mergeAgentRefresh(
  current: Meeting[],
  incoming: Meeting[],
  draftId?: string,
) {
  const merged = incoming.map((remote) => {
    const local = current.find((m) => m.id === remote.id);
    return local && (local.id === draftId || local.revision > remote.revision)
      ? local
      : remote;
  });
  return [
    ...merged,
    ...current.filter(
      (local) => !incoming.some((remote) => remote.id === local.id),
    ),
  ].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
}
