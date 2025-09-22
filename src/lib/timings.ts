export function computeUniformTimings(pages: number, durationSec: number): { slide_index: number; time_seconds: number }[] {
  if (!pages || pages <= 0 || !isFinite(durationSec) || durationSec <= 0) return [];
  const step = durationSec / pages;
  return Array.from({ length: pages }, (_, i) => ({ slide_index: i, time_seconds: +(i * step).toFixed(3) }));
}

export function clampDurationsToTotal(durations: number[], total: number): number[] {
  const copy = durations.slice();
  const sum = copy.reduce((a, b) => a + b, 0);
  if (sum > total && copy.length) {
    copy[copy.length - 1] = Math.max(0.1, copy[copy.length - 1] - (sum - total));
  }
  return copy;
}

