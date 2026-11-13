import { describe, expect, it } from 'vitest';
import type { OrbitElements } from '$lib/api';
import { eccentricAnomaly, epochMs, orbitPath, parseDate, positionAt } from './kepler';

const orbit: OrbitElements = {
	semiMajorAxisKm: 100,
	eccentricity: 0,
	inclinationDeg: 0,
	longAscendingNodeDeg: 0,
	argPeriapsisDeg: 0,
	meanAnomalyAtEpochDeg: 0,
	epochJulianYear: 2000,
	periodDays: 4,
	nodalPrecessionYears: null,
	apsidalPrecessionYears: null
};
const day = 86_400_000;

describe('orbital propagation', () => {
	it('converts epochs and calendar dates in UTC, including years below 100', () => {
		expect(epochMs(2000)).toBe(Date.UTC(2000, 0, 1, 12));
		expect(epochMs(2000.5) - epochMs(2000)).toBe(15_778_800_000);
		expect(new Date(parseDate('0099-02-03')).toISOString()).toBe('0099-02-03T00:00:00.000Z');
	});
	it.each([
		[0, 100, 0],
		[1, 0, 100],
		[2, -100, 0],
		[3, 0, -100],
		[4, 100, 0],
		[-1, 0, -100]
	])('places a circular orbit after %i days at (%i, %i)', (days, x, y) => {
		const p = positionAt(orbit, epochMs(2000) + days * day);
		expect(p.x).toBeCloseTo(x, 8);
		expect(p.y).toBeCloseTo(y, 8);
		expect(p.z).toBeCloseTo(0, 8);
	});
	it('reaches a(1-e) at periapsis and a(1+e) at apoapsis', () => {
		const o = { ...orbit, eccentricity: 0.6 };
		for (const [days, radius] of [
			[0, 40],
			[2, 160]
		]) {
			const p = positionAt(o, epochMs(2000) + days * day);
			expect(Math.hypot(p.x, p.y, p.z)).toBeCloseTo(radius, 8);
		}
	});
	it.each([0, 1e-10, 0.001, Math.PI, 2 * Math.PI - 1e-8])(
		'solves Kepler near e=1 for M=%s',
		(m) => {
			const e = 0.999999;
			const E = eccentricAnomaly(m, e);
			expect(Number.isFinite(E)).toBe(true);
			expect(E - e * Math.sin(E)).toBeCloseTo(m, 10);
		}
	);
	it('rotates an inclined orbit out of the ecliptic', () => {
		const p = positionAt({ ...orbit, inclinationDeg: 90 }, epochMs(2000) + day);
		expect(p.x).toBeCloseTo(0, 8);
		expect(p.y).toBeCloseTo(0, 8);
		expect(p.z).toBeCloseTo(100, 8);
	});
	it('samples the circular path at quarter turns', () => {
		const points = orbitPath(orbit, 4);
		[100, 0, 0, 100, -100, 0, 0, -100].forEach((v, i) => expect(points[i]).toBeCloseTo(v, 8));
	});
});
