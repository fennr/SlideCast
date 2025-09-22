import { describe, it, expect } from 'vitest';
import { computeUniformTimings, clampDurationsToTotal } from '../lib/timings';

describe('timings utils', () => {
  it('computeUniformTimings even distribution', () => {
    const res = computeUniformTimings(4, 100);
    expect(res.map(r => r.time_seconds)).toEqual([0, 25, 50, 75]);
  });

  it('clampDurationsToTotal trims last duration', () => {
    const out = clampDurationsToTotal([30, 40, 50], 100);
    expect(out).toEqual([30, 40, 30]);
  });
});


