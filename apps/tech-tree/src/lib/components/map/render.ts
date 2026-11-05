// Canvas renderer of the solar system map. Orbits are cached Path2Ds in world units drawn with
// one transform; bodies, rings and labels are drawn in screen space so their size doesn't
// depend on the zoom. A frame is O(items) apart from label collision checks (grid-bucketed).
// Orbits of a tier are merged into one path, so dense bands don't add up to a solid ring.
import type { EntityKey } from '$lib/api';
import type { Camera } from '$lib/components/tree/camera';
import type { Theme } from '$lib/components/tree/render';
import type { MapItem, SystemScene } from './scene';

export interface MapRenderState {
	selected: EntityKey | null;
	hovered: EntityKey | null;
	theme: Theme;
	/** Items failing the filters; null = no filter. */
	filtered: ReadonlySet<EntityKey> | null;
	/** Filtered items are hidden (else dimmed). */
	hideFiltered: boolean;
	/** Label comets and asteroids too (zoomed in); they'd crowd out everything else. */
	minorLabels: boolean;
	/** Loaded body icons by key. */
	icons: ReadonlyMap<EntityKey, ImageBitmap>;
	/** Items shown although they fail the filters, because bodies inside them match (marked). */
	gateways?: ReadonlySet<EntityKey> | null;
	/** Screen rects (CSS px: x1, y1, x2, y2) covered by DOM overlays; no labels go there. */
	reserved?: readonly (readonly [number, number, number, number])[];
}

const DIM = 0.2;
const LABEL_GAP = 4;
const CELL = 64;
/** Orbit opacity by tier (tier 2 fades in further as you zoom; tier 3 only when emphasized). */
const ORBIT_ALPHA = [0.5, 0.22, 0.16];

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Merged orbit paths per tier and dimming (index tier * 2 + dimmed), rebuilt when filters change. */
interface OrbitLayers {
	filtered: ReadonlySet<EntityKey> | null;
	hideFiltered: boolean;
	paths: (Path2D | null)[];
}
const orbitLayers = new WeakMap<SystemScene, OrbitLayers>();
function layersOf(scene: SystemScene, state: MapRenderState) {
	const { filtered, hideFiltered } = state;
	let layers = orbitLayers.get(scene);
	if (layers?.filtered === filtered && layers.hideFiltered === hideFiltered) return layers.paths;
	const paths: (Path2D | null)[] = Array(ORBIT_ALPHA.length * 2).fill(null);
	for (const it of scene.items) {
		if (!it.orbit || it.orbitTier >= ORBIT_ALPHA.length) continue;
		const dimmed = filtered?.has(it.key) === true;
		if (dimmed && hideFiltered) continue;
		const i = it.orbitTier * 2 + (dimmed ? 1 : 0);
		(paths[i] ??= new Path2D()).addPath(it.orbit);
	}
	layers = { filtered, hideFiltered, paths };
	orbitLayers.set(scene, layers);
	return paths;
}

/** Label order: by priority, then larger first (stable per scene). */
const labelOrders = new WeakMap<SystemScene, MapItem[]>();
function labelOrder(scene: SystemScene) {
	let order = labelOrders.get(scene);
	if (!order) {
		order = [...scene.items].sort((a, b) => a.priority - b.priority || b.size - a.size);
		labelOrders.set(scene, order);
	}
	return order;
}

const widths = new Map<string, number>();
function textWidth(ctx: CanvasRenderingContext2D, font: string, text: string) {
	const key = font + '\u0000' + text;
	let w = widths.get(key);
	if (w === undefined) {
		ctx.font = font;
		w = ctx.measureText(text).width;
		if (widths.size > 5000) widths.clear();
		widths.set(key, w);
	}
	return w;
}

/** Forget measured label widths (after web fonts load). */
export function clearTextCache() {
	widths.clear();
}

export function render(
	ctx: CanvasRenderingContext2D,
	dpr: number,
	camera: Camera,
	scene: SystemScene | null,
	state: MapRenderState
) {
	const { theme, filtered, hideFiltered, selected, hovered } = state;
	ctx.setTransform(1, 0, 0, 1, 0, 0);
	ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
	if (!scene) return;
	const z = camera.zoom;
	const w = camera.width;
	const h = camera.height;
	const hidden = (it: MapItem) => hideFiltered && filtered?.has(it.key) === true;
	const dim = (it: MapItem) => !hideFiltered && filtered?.has(it.key) === true;
	const emphasized = (it: MapItem) => it.key === selected || it.key === hovered;
	const marked = [hovered, selected].flatMap((k) => {
		const it = k ? scene.byKey.get(k) : undefined;
		return it ? [it] : [];
	});
	/** The body a Lagrange point belongs to. */
	const related = (it: MapItem) => (it.point ? scene.byKey.get(it.point.body) : undefined);

	// --- orbits (world space) ---------------------------------------------------
	ctx.setTransform(dpr * z, 0, 0, dpr * z, dpr * camera.x, dpr * camera.y);
	ctx.lineWidth = 1 / z;
	ctx.strokeStyle = theme.muted;
	// How far past the fitted view we are: faint orbits only come forward when zoomed in.
	const fade = clamp(((z * Math.min(w, h)) / 2 / scene.extent - 1.2) / 2, 0.3, 1);
	const layers = layersOf(scene, state);
	for (let i = 0; i < layers.length; i++) {
		const path = layers[i];
		if (!path) continue;
		const tier = i >> 1;
		ctx.globalAlpha = ORBIT_ALPHA[tier] * (tier === 2 ? fade : 1) * (i & 1 ? DIM : 1);
		ctx.stroke(path);
	}
	ctx.lineWidth = 1.5 / z;
	for (const it of marked) {
		// A Lagrange point highlights the orbit it travels on.
		const orbiter = it.point ? related(it) : it;
		if (!orbiter?.orbit || hidden(orbiter)) continue;
		ctx.globalAlpha = it.point ? 0.6 : 0.9;
		ctx.strokeStyle = orbiter.color;
		ctx.stroke(orbiter.orbit);
	}

	// --- bodies (screen space) --------------------------------------------------
	ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
	const margin = 24;
	const majors: MapItem[] = [];
	for (const it of scene.items) {
		it.sx = camera.x + it.wx * z;
		it.sy = camera.y + it.wy * z;
		it.onScreen =
			!hidden(it) && it.sx > -margin && it.sy > -margin && it.sx < w + margin && it.sy < h + margin;
		if (it.onScreen && it.priority <= 1) majors.push(it);
	}
	// Small fry right on top of the Sun or a planet (zoomed out, linear scale) isn't drawn.
	const covered = (it: MapItem) =>
		it.priority >= 3 &&
		!emphasized(it) &&
		majors.some((m) => Math.abs(it.sx - m.sx) + Math.abs(it.sy - m.sy) < m.size + it.size + 3);

	// Dots batched by color and alpha (the asteroid belt is one fill); minor batches first.
	const batches = new Map<string, MapItem[]>();
	for (const it of scene.items) {
		if (!it.onScreen || it.type === 'lagrange' || covered(it)) continue;
		if (state.icons.has(it.key) && !dim(it)) continue;
		const key = (dim(it) ? 'd' : 'n') + it.color;
		let list = batches.get(key);
		if (!list) batches.set(key, (list = []));
		list.push(it);
	}
	// Draw order: small fry, then the center, then planets on top (they sit on the Sun in linear scale).
	const rank = (list: MapItem[]) => {
		const p = list[0].priority;
		return p < 0 ? 1.5 : p;
	};
	let halos = false;
	for (const [key, list] of [...batches].sort((a, b) => rank(b[1]) - rank(a[1]))) {
		if (!halos && rank(list) < 2) {
			// A background halo separates major bodies from whatever they overlap.
			halos = true;
			ctx.globalAlpha = 1;
			ctx.lineWidth = 3;
			ctx.strokeStyle = theme.background;
			ctx.beginPath();
			for (const m of majors) {
				ctx.moveTo(m.sx + m.size, m.sy);
				ctx.arc(m.sx, m.sy, m.size, 0, 2 * Math.PI);
			}
			ctx.stroke();
		}
		ctx.globalAlpha = key[0] === 'd' ? DIM : 1;
		ctx.fillStyle = key.slice(1);
		ctx.beginPath();
		for (const it of list) {
			ctx.moveTo(it.sx + it.size, it.sy);
			ctx.arc(it.sx, it.sy, it.size, 0, 2 * Math.PI);
		}
		ctx.fill();
	}
	ctx.globalAlpha = 1;
	for (const it of scene.items) {
		const icon = state.icons.get(it.key);
		if (!icon || !it.onScreen || dim(it)) continue;
		const s = it.size * 2.6;
		ctx.drawImage(icon, it.sx - s / 2, it.sy - s / 2, s, s);
	}

	// Lagrange points: small diamonds.
	ctx.lineWidth = 1.2;
	ctx.beginPath();
	let dimmedPoints = false;
	for (const it of scene.items) {
		if (it.type !== 'lagrange' || !it.onScreen || covered(it)) continue;
		if (dim(it)) {
			dimmedPoints = true;
			continue;
		}
		diamond(ctx, it.sx, it.sy, 3);
	}
	ctx.strokeStyle = theme.muted;
	ctx.stroke();
	if (dimmedPoints) {
		ctx.globalAlpha = DIM;
		ctx.beginPath();
		for (const it of scene.items)
			if (it.type === 'lagrange' && it.onScreen && dim(it) && !covered(it))
				diamond(ctx, it.sx, it.sy, 3);
		ctx.stroke();
		ctx.globalAlpha = 1;
	}

	// Gateways (bodies inside match the filters): a thin dashed ring.
	if (state.gateways?.size) {
		ctx.beginPath();
		for (const it of scene.items) {
			if (!it.onScreen || !state.gateways.has(it.key)) continue;
			const r = Math.max(it.size, 3) + 7;
			ctx.moveTo(it.sx + r, it.sy);
			ctx.arc(it.sx, it.sy, r, 0, 2 * Math.PI);
		}
		ctx.setLineDash([2, 2]);
		ctx.lineWidth = 1;
		ctx.globalAlpha = 0.6;
		ctx.strokeStyle = theme.foreground;
		ctx.stroke();
		ctx.setLineDash([]);
		ctx.globalAlpha = 1;
	}

	// Hover and selection rings; a Lagrange point gets a connector to its body.
	for (const it of marked) {
		const body = related(it);
		if (body && !hidden(body) && !hidden(it)) {
			ctx.globalAlpha = 0.6;
			ctx.strokeStyle = theme.muted;
			ctx.lineWidth = 1;
			ctx.setLineDash([3, 3]);
			ctx.beginPath();
			ctx.moveTo(it.sx, it.sy);
			ctx.lineTo(body.sx, body.sy);
			ctx.stroke();
			ctx.setLineDash([]);
		}
		if (!it.onScreen) continue;
		ctx.globalAlpha = it.key === selected ? 1 : 0.55;
		ctx.strokeStyle = theme.foreground;
		ctx.lineWidth = it.key === selected ? 2 : 1.5;
		ctx.beginPath();
		ctx.arc(it.sx, it.sy, Math.max(it.size, 3) + 4, 0, 2 * Math.PI);
		ctx.stroke();
	}
	ctx.globalAlpha = 1;

	drawLabels(ctx, scene, state, dim, w, h);
}

function diamond(ctx: CanvasRenderingContext2D, x: number, y: number, r: number) {
	ctx.moveTo(x, y - r);
	ctx.lineTo(x + r, y);
	ctx.lineTo(x, y + r);
	ctx.lineTo(x - r, y);
	ctx.closePath();
}

/** Directions tried for labels pushed away from a crowded body (drawn with a leader line). */
const LEADER_DIRS = [
	[1, -1],
	[1, 1],
	[-1, -1],
	[-1, 1],
	[0, -1],
	[0, 1],
	[1, 0],
	[-1, 0]
];
const LEADER_DISTS = [22, 40, 60];

/**
 * Greedy labels: important ones first, each trying right, left, above and below its body
 * (major bodies also further out with a leader line). A label must fit the viewport and
 * not overlap a placed label or a major body's dot, else it's skipped.
 */
function drawLabels(
	ctx: CanvasRenderingContext2D,
	scene: SystemScene,
	state: MapRenderState,
	dim: (it: MapItem) => boolean,
	w: number,
	h: number
) {
	const { theme, hovered, selected } = state;
	const grid = new Map<number, number[]>();
	const rects: number[] = []; // x1, y1, x2, y2 per rect
	const cols = Math.ceil(w / CELL) + 2;
	const cellsOf = (
		x1: number,
		y1: number,
		x2: number,
		y2: number,
		fn: (cell: number) => boolean
	) => {
		const col = (x: number) => Math.min(cols - 1, Math.max(0, Math.floor(x / CELL) + 1));
		const row = (y: number) => Math.max(0, Math.floor(y / CELL) + 1);
		const [cx1, cx2, cy1, cy2] = [col(x1), col(x2), row(y1), row(y2)];
		for (let cy = cy1; cy <= cy2; cy++)
			for (let cx = cx1; cx <= cx2; cx++) if (fn(cy * cols + cx)) return true;
		return false;
	};
	const free = (x1: number, y1: number, x2: number, y2: number) =>
		!cellsOf(x1, y1, x2, y2, (cell) => {
			for (const i of grid.get(cell) ?? []) {
				const r = i * 4;
				if (x1 < rects[r + 2] && x2 > rects[r] && y1 < rects[r + 3] && y2 > rects[r + 1])
					return true;
			}
			return false;
		});
	const occupy = (x1: number, y1: number, x2: number, y2: number) => {
		const i = rects.length / 4;
		rects.push(x1, y1, x2, y2);
		cellsOf(x1, y1, x2, y2, (cell) => {
			let list = grid.get(cell);
			if (!list) grid.set(cell, (list = []));
			list.push(i);
			return false;
		});
	};

	for (const [x1, y1, x2, y2] of state.reserved ?? []) occupy(x1, y1, x2, y2);
	// Major bodies' dots block labels too, so names don't cover planets.
	for (const it of scene.items) {
		if (it.onScreen && it.priority <= 2)
			occupy(it.sx - it.size, it.sy - it.size, it.sx + it.size, it.sy + it.size);
	}

	// Candidate text origins (left, middle) and leader ends (NaN: no leader), reused per label.
	const cand: number[] = [];
	const add = (tx: number, ty: number, lx = NaN, ly = NaN) => cand.push(tx, ty, lx, ly);
	const fits = (x1: number, y1: number, x2: number, y2: number) =>
		x1 >= 0 && y1 >= 0 && x2 <= w && y2 <= h;

	const first = [hovered, selected].flatMap((k) => (k ? [scene.byKey.get(k)] : []));
	const placed = new Set<MapItem>();
	ctx.textBaseline = 'middle';
	for (const it of [...first, ...labelOrder(scene)]) {
		if (!it || placed.has(it) || !it.onScreen) continue;
		const forced = it.key === hovered || it.key === selected;
		if (!forced && (dim(it) || (it.priority >= 4 && !state.minorLabels))) continue;
		if (!forced && (it.sx < 0 || it.sy < 0 || it.sx > w || it.sy > h)) continue;
		// L1/L2 hugging their body would read as a second name for it.
		const owner = it.point && scene.byKey.get(it.point.body);
		if (!forced && owner && Math.hypot(it.sx - owner.sx, it.sy - owner.sy) < 30) continue;
		const major = it.priority <= 1;
		const font = `${major ? '600 12px' : it.type === 'lagrange' ? '10px' : '11px'} ${theme.font}`;
		// Lagrange points read as "Earth L4" (or just "L4" when crowded) unless hovered or selected.
		const body = it.point && it.body ? it.body.info.entity.name : null;
		const texts = it.point && !forced ? [`${body} ${it.point.point}`, it.point.point] : [it.name];
		const r = Math.max(it.size, 2) + LABEL_GAP;
		let spot = -1;
		let text = texts[0];
		let tw = 0;
		for (const t of texts) {
			tw = textWidth(ctx, font, t);
			cand.length = 0;
			add(it.sx + r, it.sy);
			add(it.sx - r - tw, it.sy);
			add(it.sx - tw / 2, it.sy - r - 7);
			add(it.sx - tw / 2, it.sy + r + 7);
			if (major || forced) {
				for (const d of LEADER_DISTS)
					for (const [dx, dy] of LEADER_DIRS) {
						const ax = it.sx + dx * d;
						const ay = it.sy + dy * d * (dx ? 0.7 : 1);
						if (!dx) add(ax - tw / 2, ay, ax, ay - dy * 8);
						else add(dx > 0 ? ax + 2 : ax - tw - 2, ay, ax, ay);
					}
			}
			for (let c = 0; c < cand.length; c += 4) {
				const [x1, y1, x2, y2] = [cand[c] - 2, cand[c + 1] - 8, cand[c] + tw + 2, cand[c + 1] + 8];
				if (fits(x1, y1, x2, y2) && free(x1, y1, x2, y2)) {
					spot = c;
					break;
				}
			}
			text = t;
			if (spot >= 0) break;
		}
		if (spot < 0) {
			if (!forced) continue;
			// Hovered and selected names show regardless, inside the viewport if possible.
			spot = 0;
			for (let c = 0; c < cand.length; c += 4)
				if (fits(cand[c] - 2, cand[c + 1] - 8, cand[c] + tw + 2, cand[c + 1] + 8)) {
					spot = c;
					break;
				}
		}
		const [x, y, lx, ly] = cand.slice(spot, spot + 4);
		occupy(x - 2, y - 8, x + tw + 2, y + 8);
		placed.add(it);
		ctx.globalAlpha = dim(it) ? 0.6 : 1;
		if (!Number.isNaN(lx)) {
			// Leader from the dot's edge to the displaced label.
			const d = Math.hypot(lx - it.sx, ly - it.sy) || 1;
			const k = (Math.max(it.size, 2) + 2) / d;
			ctx.lineWidth = 1;
			ctx.strokeStyle = theme.foreground;
			ctx.globalAlpha *= 0.55;
			ctx.beginPath();
			ctx.moveTo(it.sx + (lx - it.sx) * k, it.sy + (ly - it.sy) * k);
			ctx.lineTo(lx, ly);
			ctx.stroke();
			ctx.globalAlpha = dim(it) ? 0.6 : 1;
		}
		ctx.font = font;
		// Halo in the background color keeps labels readable over orbits.
		ctx.lineWidth = 3;
		ctx.strokeStyle = theme.background;
		ctx.strokeText(text, x, y);
		ctx.fillStyle = it.type === 'lagrange' || it.priority >= 4 ? theme.muted : theme.foreground;
		ctx.fillText(text, x, y);
	}
	ctx.globalAlpha = 1;
}
