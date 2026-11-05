// Scene of the solar system map: one "system" at a time (a body and what orbits it), in world
// units after a radial distance mapping (log or linear). Orbits are static (the game doesn't
// precess them), so their paths are built once per system and scale; only positions follow
// the date.
import type { BodyType, EntityKey, LagrangePointInfo, SolarSystem, SpaceBodyInfo } from '$lib/api';
import type { MapScale } from '$lib/settings.svelte';
import { orbitPath, positionAt, type Vec3 } from './kepler';

export type ItemType = BodyType | 'lagrange';

export interface MapBody {
	info: SpaceBodyInfo;
	key: EntityKey;
	parent: MapBody | null;
	children: MapBody[];
	/** Lagrange points of this body (as the secondary) with its parent. */
	points: LagrangePointInfo[];
}

export interface SolarIndex {
	data: SolarSystem;
	bodies: Map<EntityKey, MapBody>;
	/** The Sun (first body). */
	root: MapBody | null;
	points: Map<EntityKey, LagrangePointInfo>;
}

export function buildIndex(data: SolarSystem): SolarIndex {
	const bodies = new Map<EntityKey, MapBody>();
	let root: MapBody | null = null;
	for (const info of data.bodies) {
		const parent = info.parent ? (bodies.get(info.parent) ?? null) : null;
		const body: MapBody = { info, key: info.entity.key, parent, children: [], points: [] };
		bodies.set(body.key, body);
		parent?.children.push(body);
		if (!info.parent && !root) root = body;
	}
	const points = new Map<EntityKey, LagrangePointInfo>();
	for (const p of data.lagrangePoints) {
		points.set(p.entity.key, p);
		bodies.get(p.body)?.points.push(p);
	}
	return { data, bodies, root, points };
}

/** System (center body key) an entity is shown in, or undefined if it's not on the map. */
export function systemOf(index: SolarIndex, key: EntityKey): EntityKey | undefined {
	const body = index.bodies.get(key);
	if (body) return (body.parent ?? body).key;
	const point = index.points.get(key);
	const secondary = point && index.bodies.get(point.body);
	return secondary ? (secondary.parent ?? secondary).key : undefined;
}

/** Bodies from the root down to `key` (for the breadcrumb). */
export function pathTo(index: SolarIndex, key: EntityKey | null): MapBody[] {
	const out: MapBody[] = [];
	for (let b = key ? index.bodies.get(key) : index.root; b; b = b.parent ?? undefined)
		out.unshift(b);
	return out;
}

export interface MapItem {
	key: EntityKey;
	name: string;
	type: ItemType;
	body: MapBody | null;
	point: LagrangePointInfo | null;
	/** Label priority: lower is more important. */
	priority: number;
	color: string;
	/** Dot radius in screen px. */
	size: number;
	/** Orbit around the center in world units (null for the center and Lagrange points). */
	orbit: Path2D | null;
	/** How prominent the orbit is: 0 major, 1 notable, 2 faint (fades in when zoomed), 3 only when emphasized. */
	orbitTier: OrbitTier;
	/** World position at the current date. */
	wx: number;
	wy: number;
	/** Screen position (CSS px), set while rendering; `onScreen` false when culled. */
	sx: number;
	sy: number;
	onScreen: boolean;
}

export type OrbitTier = 0 | 1 | 2 | 3;

export interface SystemScene {
	center: MapBody;
	/** The center first, then its satellites, then Lagrange points. */
	items: MapItem[];
	byKey: Map<EntityKey, MapItem>;
	/** Distance (km) -> world units. */
	scale: (km: number) => number;
	/** World radius the initial view fits. */
	extent: number;
}

const PRIORITY: Record<ItemType, number> = {
	star: 0,
	planet: 1,
	dwarfPlanet: 2,
	planetaryMoon: 2,
	asteroidalMoon: 3,
	comet: 4,
	asteroid: 5,
	lagrange: 6
};

/** Dot colors by type; mid-tones that read on light and dark backgrounds. */
export const TYPE_COLORS: Record<ItemType, string> = {
	star: '#f2b705',
	planet: '#4f8fd6',
	dwarfPlanet: '#a274d8',
	planetaryMoon: '#8c97a8',
	asteroidalMoon: '#9a8c7c',
	asteroid: '#a8865a',
	comet: '#2fb3c9',
	lagrange: '#7c8a99'
};

/** Familiar colors for the major bodies (by template dataName). */
const BODY_COLORS: Record<string, string> = {
	Sol: '#f2b705',
	Mercury: '#9a9a9a',
	Venus: '#d9b26a',
	Earth: '#3b82f6',
	Luna: '#a3a3a3',
	Mars: '#d1553a',
	Jupiter: '#c99a6b',
	Saturn: '#d9c27e',
	Uranus: '#6cc7e0',
	Neptune: '#4d6fd9',
	Ceres: '#a3998c',
	Pluto: '#c9a98a',
	Io: '#d9c24a',
	Europa: '#bfa98a',
	Ganymede: '#9a8e80',
	Callisto: '#7d746a',
	Titan: '#d99a3a',
	Triton: '#b8c4cc'
};

export const TYPE_LABELS: Record<ItemType, string> = {
	star: 'Star',
	planet: 'Planet',
	dwarfPlanet: 'Dwarf planet',
	planetaryMoon: 'Moon',
	asteroidalMoon: 'Asteroid moon',
	asteroid: 'Asteroid',
	comet: 'Comet',
	lagrange: 'Lagrange point'
};

/** Planets and sizeable moons get full orbits; the dozens of small bodies stay in the background. */
function orbitTier(info: SpaceBodyInfo): OrbitTier {
	switch (info.bodyType) {
		case 'star':
		case 'planet':
			return 0;
		case 'planetaryMoon':
			return info.radiusKm >= 150 ? 0 : 1;
		case 'dwarfPlanet':
			return info.radiusKm >= 700 ? 1 : 2;
		case 'asteroidalMoon':
		case 'comet':
			return 2;
		case 'asteroid':
			return 3;
	}
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

function dotSize(info: SpaceBodyInfo): number {
	const r = Math.max(info.radiusKm, 1);
	switch (info.bodyType) {
		case 'star':
			return 9;
		case 'planet':
			return clamp(3 + 2 * Math.log10(r / 1000), 3.5, 8);
		case 'dwarfPlanet':
			return clamp(2.5 + 1.2 * Math.log10(r / 100), 2.5, 5);
		case 'planetaryMoon':
			return clamp(2 + 1.5 * Math.log10(r / 100), 2, 5.5);
		case 'asteroidalMoon':
			return 1.8;
		case 'comet':
			return 2;
		case 'asteroid':
			return clamp(1.4 + 0.6 * Math.log10(r / 10), 1.4, 2.5);
	}
}

/** Usable orbit: bound, with a positive size (some templates are stubs). */
function hasOrbit(b: MapBody) {
	const o = b.info.orbit;
	return !!o && o.semiMajorAxisKm > 0 && o.eccentricity < 1 && Number.isFinite(o.periodDays);
}

/** Bodies orbiting `center` that the map can place. */
function satellites(center: MapBody) {
	return center.children.filter(hasOrbit);
}

function quantile(sorted: number[], q: number) {
	if (!sorted.length) return 0;
	return sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
}

/** World units per `r0` (linear) or per e-fold beyond `r0` (log). */
const UNIT = 100;

export function buildSystem(center: MapBody, mode: MapScale): SystemScene {
	const sats = satellites(center);
	const axes = sats.map((b) => b.info.orbit!.semiMajorAxisKm).sort((a, b) => a - b);
	// Reference distance: well inside the innermost orbits, so log spacing reaches them too.
	const r0 = Math.max(quantile(axes, 0.05) * 0.15, center.info.radiusKm * 2, 1);
	const scale =
		mode === 'log' ? (km: number) => UNIT * Math.log1p(km / r0) : (km: number) => (UNIT * km) / r0;

	const items: MapItem[] = [];
	const item = (
		key: EntityKey,
		name: string,
		type: ItemType,
		body: MapBody | null,
		point: LagrangePointInfo | null,
		size: number,
		color: string
	): MapItem => ({
		key,
		name,
		type,
		body,
		point,
		priority: PRIORITY[type],
		color,
		size,
		orbit: null,
		orbitTier: 3,
		wx: 0,
		wy: 0,
		sx: 0,
		sy: 0,
		onScreen: false
	});

	const centerItem = item(
		center.key,
		center.info.entity.name,
		center.info.bodyType,
		center,
		null,
		center.info.bodyType === 'star' ? 10 : clamp(dotSize(center.info) * 1.6, 6, 11),
		BODY_COLORS[center.info.entity.dataName] ?? TYPE_COLORS[center.info.bodyType]
	);
	centerItem.priority = -1;
	items.push(centerItem);

	const apoapses: number[] = [];
	for (const b of sats) {
		const it = item(
			b.key,
			b.info.entity.name,
			b.info.bodyType,
			b,
			null,
			dotSize(b.info),
			BODY_COLORS[b.info.entity.dataName] ?? TYPE_COLORS[b.info.bodyType]
		);
		it.orbit = orbitToPath(orbitPath(b.info.orbit!, orbitSamples(b.info.bodyType)), scale);
		it.orbitTier = orbitTier(b.info);
		items.push(it);
		const o = b.info.orbit!;
		apoapses.push(scale(o.semiMajorAxisKm * (1 + o.eccentricity)));
	}
	for (const b of sats) {
		for (const p of b.points) {
			items.push(item(p.entity.key, p.entity.name, 'lagrange', b, p, 2.5, TYPE_COLORS.lagrange));
		}
	}

	apoapses.sort((a, b) => a - b);
	const extent = Math.max(quantile(apoapses, 0.9), scale(center.info.radiusKm * 4), 1) * 1.08;
	return { center, items, byKey: new Map(items.map((i) => [i.key, i])), scale, extent };
}

function orbitSamples(type: BodyType) {
	return type === 'asteroid' || type === 'asteroidalMoon' ? 96 : 192;
}

/** Map a (km) polyline radially into world units; y flips so prograde orbits run counterclockwise. */
function orbitToPath(points: Float64Array, scale: (km: number) => number): Path2D {
	const path = new Path2D();
	for (let j = 0; j < points.length; j += 2) {
		const [x, y] = mapPoint(points[j], points[j + 1], scale);
		if (j === 0) path.moveTo(x, y);
		else path.lineTo(x, y);
	}
	path.closePath();
	return path;
}

function mapPoint(x: number, y: number, scale: (km: number) => number): [number, number] {
	const r = Math.hypot(x, y);
	if (r === 0) return [0, 0];
	const k = scale(r) / r;
	return [x * k, -y * k];
}

/** Positions of every item at UTC timestamp `t` (ms). O(items). */
export function updatePositions(scene: SystemScene, t: number) {
	const km = new Map<EntityKey, Vec3>();
	const primaryMass = scene.center.info.massKg;
	for (const it of scene.items) {
		if (it.body && !it.point && it.body !== scene.center) {
			const p = positionAt(it.body.info.orbit!, t);
			km.set(it.key, p);
			[it.wx, it.wy] = mapPoint(p.x, p.y, scene.scale);
		} else if (it.point && it.body) {
			const s = km.get(it.body.key);
			if (!s) continue;
			const [x, y] = lagrangePosition(it.point.point, s, it.body.info.massKg, primaryMass);
			[it.wx, it.wy] = mapPoint(x, y, scene.scale);
		}
	}
}

/**
 * Lagrange point (km, ecliptic x/y) from the secondary's position relative to the primary.
 * L1/L2 sit a Hill radius r·∛(m/3M) inside/outside, L3 opposite, L4/L5 60° ahead/behind.
 */
export function lagrangePosition(
	point: LagrangePointInfo['point'],
	secondary: Vec3,
	secondaryMass: number | null,
	primaryMass: number | null
): [number, number] {
	const { x, y } = secondary;
	const r = Math.hypot(x, y) || 1;
	const hill =
		secondaryMass && primaryMass ? r * Math.cbrt(secondaryMass / (3 * primaryMass)) : r * 0.01;
	switch (point) {
		case 'L1':
			return [x * (1 - hill / r), y * (1 - hill / r)];
		case 'L2':
			return [x * (1 + hill / r), y * (1 + hill / r)];
		case 'L3':
			return [-x, -y];
		case 'L4':
		case 'L5': {
			const a = ((point === 'L4' ? 1 : -1) * Math.PI) / 3;
			const c = Math.cos(a);
			const s = Math.sin(a);
			return [x * c - y * s, x * s + y * c];
		}
	}
}
