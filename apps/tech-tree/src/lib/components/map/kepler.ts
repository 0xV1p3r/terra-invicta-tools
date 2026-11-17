// Two-body propagation of the game's orbital elements, following the decompiled game code:
// `TISpaceObjectState.ToLocalCartesianStateAtTime` -> `OrbitalElementsState.ToCartesianStateAtTime`.
//
// - Epoch: `TIDateTime.SetTime(julianEpoch)` = January 1st of year ⌊J⌋, plus frac(J) × 31,557,600 s
//   (Julian years), plus 12 h. So J2000.0 is 2000-01-01 12:00, as usual.
// - Mean anomaly: M = M₀ + 2π·Δt/P (Δt in seconds since the epoch, P the period around the parent).
// - Kepler's equation by Newton's method, true anomaly via atan2, r = a(1 − e²)/(1 + e·cos ν).
// - Orientation: `AnglesToCartesianState(Ω, u = ν + ω, i)`; z is out of the ecliptic, so the
//   top-down map uses (x, y).
// - The templates carry nodal/apsidal precession periods for moons, but the game never applies
//   them (Ω and ω are copied from the template once). `precession: true` turns them on anyway.
//
// Self-check (Earth at J2000, M₀ = 358.617°, Ω = 348.7394°, ω = 114.20783°, e = 0.0167):
// ν ≈ −1.4°, so the heliocentric longitude is Ω + ω + ν ≈ 101.5°, matching the real ≈ 100.5°
// for 2000-01-01 (the Sun's apparent longitude of ≈ 280.5° plus 180°).
import type { OrbitElements } from '#lib/api.ts';

const DEG = Math.PI / 180;
const TAU = 2 * Math.PI;
const DAY_MS = 86_400_000;
const JULIAN_YEAR_MS = 31_557_600_000;

export interface Vec3 {
	x: number;
	y: number;
	z: number;
}

/** UTC timestamp (ms) of a Julian-year epoch, the way the game converts it. */
export function epochMs(julianYear: number): number {
	const year = Math.trunc(julianYear);
	const jan1 = new Date(0);
	jan1.setUTCFullYear(year, 0, 1);
	jan1.setUTCHours(12, 0, 0, 0);
	return jan1.getTime() + Math.trunc((julianYear - year) * 31_557_600) * 1000;
}

/** UTC timestamp (ms) of a `YYYY-MM-DD` date at midnight. */
export function parseDate(date: string): number {
	const [y, m, d] = date.split('-').map(Number);
	const t = new Date(0);
	t.setUTCFullYear(y, (m || 1) - 1, d || 1);
	return t.getTime();
}

const wrap = (a: number) => ((a % TAU) + TAU) % TAU;

/** Eccentric anomaly E with E − e·sin E = M (0 ≤ e < 1). Newton with a bisection fallback. */
export function eccentricAnomaly(M: number, e: number): number {
	M = wrap(M);
	if (e === 0) return M;
	// Start at π for high eccentricities (Newton from M can overshoot near periapsis).
	let E = e < 0.8 ? M : Math.PI;
	for (let k = 0; k < 50; k++) {
		const f = E - e * Math.sin(E) - M;
		const step = f / (1 - e * Math.cos(E));
		E -= step;
		if (Math.abs(step) < 1e-12) return E;
	}
	// Didn't converge (shouldn't happen for e < 1): bisect, f is monotonic on [0, 2π].
	let lo = 0;
	let hi = TAU;
	for (let k = 0; k < 64; k++) {
		E = (lo + hi) / 2;
		if (E - e * Math.sin(E) - M > 0) hi = E;
		else lo = E;
	}
	return E;
}

/** Hyperbolic anomaly H with e·sinh H − H = M (e > 1). */
function hyperbolicAnomaly(M: number, e: number): number {
	let H = Math.abs(M) > 10 ? Math.sign(M) * Math.log(Math.abs(M) / e) : M;
	for (let k = 0; k < 100; k++) {
		const step = (e * Math.sinh(H) - H - M) / (e * Math.cosh(H) - 1);
		H -= step;
		if (Math.abs(step) < 1e-12) break;
	}
	return H;
}

/** Rotate a point at true anomaly ν and distance r of the orbit into the parent's frame. */
function orient(r: number, nu: number, omega: number, node: number, inc: number): Vec3 {
	const u = nu + omega;
	const su = Math.sin(u);
	const cu = Math.cos(u);
	const sn = Math.sin(node);
	const cn = Math.cos(node);
	const ci = Math.cos(inc);
	return {
		x: r * (cn * cu - sn * su * ci),
		y: r * (sn * cu + cn * su * ci),
		z: r * su * Math.sin(inc)
	};
}

interface Angles {
	node: number;
	omega: number;
	inc: number;
}

function angles(o: OrbitElements, t: number, precession: boolean): Angles {
	let node = o.longAscendingNodeDeg * DEG;
	let omega = o.argPeriapsisDeg * DEG;
	if (precession) {
		const years = (t - epochMs(o.epochJulianYear)) / JULIAN_YEAR_MS;
		// Prograde moons' nodes regress, their periapses advance.
		if (o.nodalPrecessionYears) node -= (TAU * years) / o.nodalPrecessionYears;
		if (o.apsidalPrecessionYears) omega += (TAU * years) / o.apsidalPrecessionYears;
	}
	return { node, omega, inc: o.inclinationDeg * DEG };
}

/** Position (km) relative to the parent at UTC timestamp `t` (ms). */
export function positionAt(o: OrbitElements, t: number, precession = false): Vec3 {
	const a = o.semiMajorAxisKm;
	const e = o.eccentricity;
	const { node, omega, inc } = angles(o, t, precession);
	const dtDays = (t - epochMs(o.epochJulianYear)) / DAY_MS;
	const M = o.meanAnomalyAtEpochDeg * DEG + (o.periodDays > 0 ? (TAU * dtDays) / o.periodDays : 0);
	let nu: number;
	if (e < 1) {
		const E = eccentricAnomaly(M, e);
		nu = 2 * Math.atan2(Math.sqrt(1 + e) * Math.sin(E / 2), Math.sqrt(1 - e) * Math.cos(E / 2));
	} else {
		const H = hyperbolicAnomaly(M, e);
		nu = 2 * Math.atan(Math.sqrt((e + 1) / (e - 1)) * Math.tanh(H / 2));
	}
	const r = (a * (1 - e * e)) / (1 + e * Math.cos(nu));
	return orient(r, nu, omega, node, inc);
}

/**
 * The orbit projected onto the ecliptic as `n` points (x0, y0, x1, y1, …, km), sampled evenly in
 * eccentric anomaly (denser near periapsis, where the curve bends most). Bound orbits only.
 */
export function orbitPath(o: OrbitElements, n = 128, t = 0, precession = false): Float64Array {
	const a = o.semiMajorAxisKm;
	const e = Math.min(o.eccentricity, 0.9999);
	const { node, omega, inc } = angles(o, t, precession);
	const out = new Float64Array(n * 2);
	const k = Math.sqrt((1 + e) / (1 - e));
	for (let j = 0; j < n; j++) {
		const E = (TAU * j) / n;
		const nu = 2 * Math.atan(k * Math.tan(E / 2));
		const p = orient(a * (1 - e * Math.cos(E)), nu, omega, node, inc);
		out[2 * j] = p.x;
		out[2 * j + 1] = p.y;
	}
	return out;
}
