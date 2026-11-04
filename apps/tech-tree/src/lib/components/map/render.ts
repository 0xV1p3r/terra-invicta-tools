// Canvas renderer of the solar system map. Orbits are cached Path2Ds in world units drawn with
// one transform; bodies, rings and labels are drawn in screen space so their size doesn't
// depend on the zoom. A frame is O(items) apart from label collision checks (grid-bucketed).
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
}

const DIM = 0.2;
const LABEL_GAP = 4;
const CELL = 64;

/** Hundreds of asteroid orbits are a hairball: only drawn when hovered or selected. */
const isMinor = (it: MapItem) => it.type === 'asteroid';

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
	const hidden = (it: MapItem) => hideFiltered && filtered?.has(it.key) === true;
	const dim = (it: MapItem) => !hideFiltered && filtered?.has(it.key) === true;
	const emphasized = (it: MapItem) => it.key === selected || it.key === hovered;

	// --- orbits (world space) ---------------------------------------------------
	ctx.setTransform(dpr * z, 0, 0, dpr * z, dpr * camera.x, dpr * camera.y);
	ctx.lineWidth = 1 / z;
	ctx.strokeStyle = theme.muted;
	for (const it of scene.items) {
		if (!it.orbit || hidden(it) || emphasized(it) || isMinor(it)) continue;
		ctx.globalAlpha = (it.type === 'comet' ? 0.25 : 0.45) * (dim(it) ? DIM : 1);
		ctx.stroke(it.orbit);
	}
	ctx.lineWidth = 1.5 / z;
	for (const key of [hovered, selected]) {
		const it = key ? scene.byKey.get(key) : undefined;
		if (!it?.orbit || hidden(it)) continue;
		ctx.globalAlpha = 0.9;
		ctx.strokeStyle = it.color;
		ctx.stroke(it.orbit);
	}

	// --- bodies (screen space) --------------------------------------------------
	ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
	const w = camera.width;
	const h = camera.height;
	const margin = 24;
	for (const it of scene.items) {
		it.sx = camera.x + it.wx * z;
		it.sy = camera.y + it.wy * z;
		it.onScreen =
			!hidden(it) && it.sx > -margin && it.sy > -margin && it.sx < w + margin && it.sy < h + margin;
	}

	// Dots batched by color and alpha (the asteroid belt is one fill).
	const batches = new Map<string, MapItem[]>();
	for (const it of scene.items) {
		if (!it.onScreen || it.type === 'lagrange') continue;
		if (state.icons.has(it.key) && !dim(it)) continue;
		const key = (dim(it) ? 'd' : 'n') + it.color;
		let list = batches.get(key);
		if (!list) batches.set(key, (list = []));
		list.push(it);
	}
	for (const [key, list] of batches) {
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
		if (it.type !== 'lagrange' || !it.onScreen) continue;
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
			if (it.type === 'lagrange' && it.onScreen && dim(it)) diamond(ctx, it.sx, it.sy, 3);
		ctx.stroke();
		ctx.globalAlpha = 1;
	}

	// Hover and selection rings.
	for (const key of [hovered, selected]) {
		const it = key ? scene.byKey.get(key) : undefined;
		if (!it?.onScreen) continue;
		ctx.globalAlpha = key === selected ? 1 : 0.55;
		ctx.strokeStyle = theme.foreground;
		ctx.lineWidth = key === selected ? 2 : 1.5;
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

/** Greedy labels: important ones first, skipped when they'd overlap a placed label or dot. */
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

	// Major bodies' dots block labels too, so names don't cover planets.
	for (const it of scene.items) {
		if (it.onScreen && it.priority <= 2)
			occupy(it.sx - it.size, it.sy - it.size, it.sx + it.size, it.sy + it.size);
	}

	const first = [hovered, selected].flatMap((k) => (k ? [scene.byKey.get(k)] : []));
	const placed = new Set<MapItem>();
	ctx.textBaseline = 'middle';
	for (const it of [...first, ...labelOrder(scene)]) {
		if (!it || placed.has(it) || !it.onScreen) continue;
		const forced = it.key === hovered || it.key === selected;
		if (!forced && (dim(it) || (it.priority >= 4 && !state.minorLabels))) continue;
		// Lagrange points read as "L4" next to their body unless hovered or selected.
		const text = it.point && !forced ? it.point.point : it.name;
		const major = it.priority <= 1;
		const font = `${major ? '600 12px' : it.type === 'lagrange' ? '10px' : '11px'} ${theme.font}`;
		const tw = textWidth(ctx, font, text);
		const x = it.sx + Math.max(it.size, 2) + LABEL_GAP;
		const x1 = x - 2;
		const y1 = it.sy - 8;
		const x2 = x + tw + 2;
		const y2 = it.sy + 8;
		if (!forced && (x2 < 0 || x1 > w || y2 < 0 || y1 > h || !free(x1, y1, x2, y2))) continue;
		occupy(x1, y1, x2, y2);
		placed.add(it);
		ctx.font = font;
		ctx.globalAlpha = dim(it) ? 0.6 : 1;
		// Halo in the background color keeps labels readable over orbits.
		ctx.lineWidth = 3;
		ctx.strokeStyle = theme.background;
		ctx.strokeText(text, x, it.sy);
		ctx.fillStyle = it.type === 'lagrange' || it.priority >= 4 ? theme.muted : theme.foreground;
		ctx.fillText(text, x, it.sy);
	}
	ctx.globalAlpha = 1;
}
