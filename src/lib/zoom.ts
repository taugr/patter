export const zoomLevels = [75, 80, 90, 100, 110, 125, 150, 175, 200];
export function normalizeZoom(value: number): number {
  return zoomLevels.includes(value) ? value : 100;
}
export function nextZoom(value: number, direction: number): number {
  const index = zoomLevels.indexOf(normalizeZoom(value));
  return zoomLevels[
    Math.max(0, Math.min(zoomLevels.length - 1, index + direction))
  ];
}
