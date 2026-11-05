// Canvas renderer of the research tree. Everything is drawn in world coordinates with
// one transform per frame; detail (text, icons, badges) depends on the zoom so a frame
// costs about the same whether 30 or 900 cards are visible.
import type { EntityKey, TechCategory } from '$lib/api';
import { CATEGORIES } from '$lib/categories';
import type { Camera, Rect } from './camera';
import {
	importanceOf,
	lineageOf,
	nodesIn,
	type EdgePaths,
	type Lineage,
	type Scene,
	type SceneNode
} from './scene';

export interface Theme {
	background: string;
	card: string;
	cardForeground: string;
	border: string;
	muted: string;
	foreground: string;
	secondary: string;
	secondaryForeground: string;
	destructive: string;
	/** Category colors for small marks and text (darker in the light theme). */
	categories: Partial<Record<TechCategory, string>>;
	font: string;
}

/** Theme colors from the CSS variables in effect on `el`. */
export function readTheme(el: Element): Theme {
	const s = getComputedStyle(el);
	const v = (name: string) => s.getPropertyValue(name).trim();
	return {
		background: v('--background'),
		card: v('--card'),
		cardForeground: v('--card-foreground'),
		border: v('--border'),
		muted: v('--muted-foreground'),
		foreground: v('--foreground'),
		secondary: v('--secondary'),
		secondaryForeground: v('--secondary-foreground'),
		destructive: v('--destructive'),
		categories: Object.fromEntries(
			Object.entries(CATEGORIES).map(([key, info]) => {
				const name = info.cssVar.slice(4, -1); // var(--category-x) → --category-x
				return [key, v(`${name}-fg`) || v(name) || info.color];
			})
		),
		font: s.fontFamily || 'sans-serif'
	};
}

/** Category color of a card for small marks and text in the current theme. */
const accent = (theme: Theme, n: SceneNode) => theme.categories[n.item.category] ?? n.color;

export interface RenderState {
	selected: EntityKey | null;
	hovered: EntityKey | null;
	/** Subtree root (dashed outline). */
	root: EntityKey | null;
	theme: Theme;
	icons: Partial<Record<TechCategory, HTMLImageElement>>;
	/** Flag glyphs as Path2D (24×24 box), by flag key. */
	glyphs: Map<string, Path2D>;
	/** Filter matches in highlight mode (others are dimmed); null = no highlighting. */
	matches: ReadonlySet<EntityKey> | null;
	/** Draw the far-out overview (category blocks, landmark labels) below `OVERVIEW_ZOOM`. */
	overview: boolean;
}

/** Zoom from which cards show icon, name and badges. */
export const DETAIL_ZOOM = 0.45;
/** Zoom from which zoomed-out cards show their (large) name; below it landmarks label the tree. */
const NAME_ZOOM = 0.18;
/** Zoom below which the overview (if enabled) replaces cards with category blocks. */
export const OVERVIEW_ZOOM = 0.1;
/** Most landmark labels in the overview. */
const LANDMARKS = 40;
/** Widest one-line landmark label (screen px, padding included) where there is room. */
const LANDMARK_WIDE = 250;
/**
 * Width (screen px, padding included) of two-line landmark labels and of the shortened
 * one-line fallback where space is tight.
 */
const LANDMARK_WIDTH = 150;
const LANDMARK_LINE = 14;
/** Landmark candidates tried per frame, so dense views don't scan the whole ranking. */
const LANDMARK_TRIES = 240;
/**
 * Opacity of the fill, border and category bar of cards outside the selection's lineage
 * or the filter matches. Their text switches to the muted color instead of fading, so it
 * stays legible.
 */
const DIM = 0.3;
/** Opacity of their border, kept higher so the card shapes stay apparent. */
const DIM_BORDER = 0.75;
/** Opacity of their icon and badges (secondary metadata, so dimmed further than the name). */
const DIM_CONTENT = 0.4;

const RADIUS = 14;
const BIG_FONT_PX = 26;

const lineageCache = new WeakMap<Scene, { key: EntityKey; lineage: Lineage }>();

function selectedLineage(scene: Scene, key: EntityKey | null): Lineage | null {
	if (key === null || !scene.byKey.has(key)) return null;
	const cached = lineageCache.get(scene);
	if (cached?.key === key) return cached.lineage;
	const lineage = lineageOf(scene, key);
	lineageCache.set(scene, { key, lineage });
	return lineage;
}

export function render(
	ctx: CanvasRenderingContext2D,
	dpr: number,
	camera: Camera,
	scene: Scene | null,
	state: RenderState
) {
	const { theme } = state;
	ctx.setTransform(1, 0, 0, 1, 0, 0);
	ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
	if (!scene) return;
	const z = camera.zoom;
	ctx.setTransform(dpr * z, 0, 0, dpr * z, dpr * camera.x, dpr * camera.y);

	const view = camera.visible();
	const visible = nodesIn(scene, view);
	const detail = z >= DETAIL_ZOOM;
	const w = scene.width;
	const h = scene.height;
	const lineage = selectedLineage(scene, state.selected);
	const { matches } = state;
	const unrelated = (n: SceneNode) =>
		(lineage !== null && !lineage.nodes.has(n.key)) || (matches !== null && !matches.has(n.key));
	/** Secondary cards: unrelated ones, and unobtainable ones (which also get a dashed border). */
	const secondary = (n: SceneNode) => unrelated(n) || !n.item.flags.obtainable;
	const px = 1 / z; // one screen pixel in world units
	const overview = state.overview && z < OVERVIEW_ZOOM;

	// --- category bands --------------------------------------------------------
	const bandPad = 40;
	for (const b of scene.graph.bands) {
		ctx.globalAlpha = 0.07;
		ctx.fillStyle = CATEGORIES[b.category]?.color ?? theme.muted;
		ctx.fillRect(
			scene.bounds.x - bandPad,
			b.top - bandPad,
			scene.bounds.width + 2 * bandPad,
			b.bottom - b.top + 2 * bandPad
		);
	}
	ctx.globalAlpha = 1;

	// --- edges ---------------------------------------------------------------
	// Plain edges are bundled onto shared trunks and stay quiet so they don't knot up; only
	// the hovered and selected nodes' own edges are drawn as strong individual curves.
	const fade = Math.min(1, Math.max(0.2, z * 2)) * (overview ? 0.5 : 1);
	ctx.globalAlpha = (lineage ? 0.08 : matches ? 0.25 : 0.45) * fade;
	strokeEdges(ctx, scene.bundled, theme.muted, Math.max(1.5, 0.5 * px), px);
	if (lineage) {
		if (lineage.nodes.size > scene.nodes.length / 2) {
			// Big lineages (a sub-tree's root reaches everything) are the structure itself:
			// bundled like plain edges, just a little stronger.
			ctx.globalAlpha = 0.55 * fade;
			strokeEdges(ctx, lineage.bundled, theme.muted, Math.max(1.5, 0.75 * px), px);
		} else {
			ctx.globalAlpha = 0.55;
			strokeEdges(ctx, lineage.edges, theme.foreground, Math.max(1.5, px), px);
		}
	}
	const highlight = lineage && state.selected ? scene.edgesOf.get(state.selected) : undefined;
	if (highlight) {
		ctx.globalAlpha = 1;
		strokeEdges(ctx, highlight, theme.foreground, Math.max(3, 1.5 * px), px);
	}
	const hoverEdges =
		state.hovered && state.hovered !== state.selected
			? scene.edgesOf.get(state.hovered)
			: undefined;
	if (hoverEdges) {
		ctx.globalAlpha = 0.9;
		strokeEdges(ctx, hoverEdges, theme.foreground, Math.max(2, px), px);
	}
	ctx.globalAlpha = 1;

	// --- card bodies, borders and category bars (batched per style) -----------
	const bar = detail ? 4 : 12;
	for (const dashed of [false, true]) {
		for (const outside of [false, true]) {
			const group = visible.filter(
				(n) => n.item.flags.obtainable === !dashed && unrelated(n) === outside
			);
			if (!group.length) continue;
			ctx.globalAlpha = outside ? DIM : 1;
			if (overview) {
				// Plain category blocks.
				const blocks = new Map<string, Path2D>();
				for (const n of group) {
					let p = blocks.get(n.color);
					if (!p) blocks.set(n.color, (p = new Path2D()));
					p.roundRect(n.x, n.y, w, h, RADIUS);
				}
				for (const [color, p] of blocks) {
					ctx.fillStyle = color;
					ctx.fill(p);
				}
				continue;
			}
			const body = new Path2D();
			for (const n of group) body.roundRect(n.x, n.y, w, h, RADIUS);
			if (outside) {
				// Opaque base so edges don't show through the faded cards.
				ctx.globalAlpha = 1;
				ctx.fillStyle = theme.background;
				ctx.fill(body);
				ctx.globalAlpha = DIM;
			}
			ctx.fillStyle = theme.card;
			ctx.fill(body);
			if (z > 0.12) {
				if (outside) ctx.globalAlpha = DIM_BORDER;
				ctx.strokeStyle = dashed && !outside ? theme.muted : theme.border;
				ctx.lineWidth = px;
				ctx.setLineDash(dashed ? [6 * px, 4 * px] : []);
				ctx.stroke(body);
				ctx.setLineDash([]);
				if (outside) ctx.globalAlpha = DIM;
			}
			// Bars are narrower than the corner radius, so draw them as plain rects clipped to
			// the card shapes (a rounded bar can't follow the card's corners and sticks out).
			const bars = new Map<string, Path2D>();
			for (const n of group) {
				const color = accent(theme, n);
				let p = bars.get(color);
				if (!p) bars.set(color, (p = new Path2D()));
				p.rect(n.x, n.y, bar, h);
			}
			ctx.save();
			ctx.clip(body);
			for (const [color, p] of bars) {
				ctx.fillStyle = color;
				ctx.fill(p);
			}
			ctx.restore();
		}
	}

	// --- content --------------------------------------------------------------
	ctx.globalAlpha = 1;
	if (detail) {
		for (const n of visible) drawDetail(ctx, n, scene, state, secondary(n), unrelated(n));
	} else if (z >= NAME_ZOOM && !overview) {
		ctx.font = `600 ${BIG_FONT_PX}px ${theme.font}`;
		ctx.textBaseline = 'middle';
		const maxWidth = w - bar - 24;
		const lineHeight = BIG_FONT_PX * 1.15;
		for (const n of visible) {
			ctx.fillStyle = secondary(n) ? theme.muted : theme.cardForeground;
			const lines = wrap(ctx, n, maxWidth, 2);
			const top = n.y + h / 2 - ((lines.length - 1) * lineHeight) / 2;
			lines.forEach((line, i) => ctx.fillText(line, n.x + bar + 12, top + i * lineHeight));
		}
	}
	ctx.globalAlpha = 1;

	// --- "any of" requirement groups (focus view) -------------------------------
	if (scene.graph.anyOf.length) {
		ctx.strokeStyle = theme.muted;
		ctx.fillStyle = theme.muted;
		ctx.lineWidth = Math.max(1.5, px);
		ctx.font = `500 12px ${theme.font}`;
		ctx.textAlign = 'center';
		ctx.textBaseline = 'middle';
		for (const g of scene.graph.anyOf) {
			const x = g.x - 14 - 32 * g.level;
			ctx.beginPath();
			ctx.moveTo(x + 8, g.top);
			ctx.lineTo(x, g.top);
			ctx.lineTo(x, g.bottom);
			ctx.lineTo(x + 8, g.bottom);
			ctx.stroke();
			ctx.save();
			ctx.translate(x - 10, (g.top + g.bottom) / 2);
			ctx.rotate(-Math.PI / 2);
			ctx.fillText('any of', 0, 0);
			ctx.restore();
		}
		ctx.textAlign = 'start';
	}

	// --- hover, selection, subtree root ----------------------------------------
	const outline = (key: EntityKey | null, draw: (n: SceneNode) => void) => {
		const n = key ? scene.byKey.get(key) : undefined;
		if (n) draw(n);
	};
	outline(state.hovered, (n) => {
		ctx.strokeStyle = accent(theme, n);
		ctx.lineWidth = 2 * px;
		ctx.beginPath();
		ctx.roundRect(n.x - px, n.y - px, w + 2 * px, h + 2 * px, RADIUS + px);
		ctx.stroke();
	});
	outline(state.root, (n) => {
		const gap = 5 * px;
		ctx.strokeStyle = theme.foreground;
		ctx.lineWidth = 2 * px;
		ctx.setLineDash([6 * px, 4 * px]);
		ctx.beginPath();
		ctx.roundRect(n.x - gap, n.y - gap, w + 2 * gap, h + 2 * gap, RADIUS + gap);
		ctx.stroke();
		ctx.setLineDash([]);
	});
	outline(state.selected, (n) => {
		// A crisp border with a soft halo outside it.
		ctx.strokeStyle = accent(theme, n);
		ctx.beginPath();
		ctx.roundRect(n.x - 5 * px, n.y - 5 * px, w + 10 * px, h + 10 * px, RADIUS + 5 * px);
		ctx.globalAlpha = 0.18;
		ctx.lineWidth = 4 * px;
		ctx.stroke();
		ctx.globalAlpha = 1;
		ctx.beginPath();
		ctx.roundRect(n.x - 1.5 * px, n.y - 1.5 * px, w + 3 * px, h + 3 * px, RADIUS + 1.5 * px);
		ctx.lineWidth = 2.5 * px;
		ctx.stroke();
	});

	// --- screen-space labels (constant size) -------------------------------------
	ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
	if (z < NAME_ZOOM) drawLandmarks(ctx, camera, scene, visible, state, unrelated);
	if (scene.graph.bands.length) drawBandLabels(ctx, camera, scene, theme, bandPad);
	ctx.globalAlpha = 1;
}

/** Category name at the top left of each band, kept on screen while the band is visible. */
function drawBandLabels(
	ctx: CanvasRenderingContext2D,
	camera: Camera,
	scene: Scene,
	theme: Theme,
	pad: number
) {
	const z = camera.zoom;
	const left = Math.max(12, camera.x + (scene.bounds.x - pad) * z + 12);
	ctx.font = `600 15px ${theme.font}`;
	ctx.textBaseline = 'top';
	for (const b of scene.graph.bands) {
		const top = camera.y + (b.top - pad) * z;
		const bottom = camera.y + (b.bottom + pad) * z;
		if (bottom < 0 || top > camera.height) continue;
		// Stick to the top of the screen, but stay inside the band.
		const y = Math.min(Math.max(top + 10, 10), bottom - 28);
		const info = CATEGORIES[b.category];
		ctx.fillStyle = theme.categories[b.category] ?? theme.foreground;
		ctx.fillText(info?.label ?? b.category, left, y);
	}
}

interface LandmarkLabel {
	lines: string[];
	/** Widest line (text only). */
	width: number;
}

const rankingCache = new WeakMap<
	Scene,
	{ font: string; ranking: { node: SceneNode; labels: LandmarkLabel[] }[] }
>();

/**
 * Nodes by importance with their landmark label variants in the current font, most
 * complete first: the whole name on one line, then on two lines, then shortened. A
 * shortened label that would read the same as another node's keeps the name's end instead
 * ("Antimatter…Reactor III").
 */
function landmarkRanking(ctx: CanvasRenderingContext2D, scene: Scene) {
	const cached = rankingCache.get(scene);
	if (cached?.font === ctx.font) return cached.ranking;
	const importance = importanceOf(scene);
	const label = (lines: string[]) => ({
		lines,
		width: Math.max(...lines.map((l) => ctx.measureText(l).width))
	});
	const short = new Map(scene.nodes.map((n) => [n, ellipsize(ctx, n, LANDMARK_WIDTH - 12)]));
	const uses = new Map<string, number>();
	for (const text of short.values()) uses.set(text, (uses.get(text) ?? 0) + 1);
	const ranking = [...scene.nodes]
		.sort((a, b) => (importance.get(b.key) ?? 0) - (importance.get(a.key) ?? 0))
		.map((node) => {
			const name = node.item.name;
			const full = label([name]);
			if (full.width <= LANDMARK_WIDTH - 12) return { node, labels: [full] };
			const labels: LandmarkLabel[] = [];
			if (full.width <= LANDMARK_WIDE - 12) labels.push(full);
			const lines = wrap(ctx, node, LANDMARK_WIDTH - 12, 2);
			if (lines.join(' ') === name.replace(/\s+/g, ' ')) labels.push(label(lines));
			const cut = short.get(node)!;
			labels.push(
				label([(uses.get(cut) ?? 0) > 1 ? middleEllipsize(ctx, name, LANDMARK_WIDTH - 12) : cut])
			);
			return { node, labels };
		});
	rankingCache.set(scene, { font: ctx.font, ranking });
	return ranking;
}

interface Box {
	x: number;
	y: number;
	w: number;
	h: number;
}

const overlaps = (a: Box, b: Box, pad: number) =>
	a.x < b.x + b.w + pad && a.x + a.w + pad > b.x && a.y < b.y + b.h + pad && a.y + a.h + pad > b.y;

/**
 * Labels of the most important visible research. Each sits on its card, or just above or
 * below it with a short leader, wherever it covers the fewest neighbouring cards; labels
 * that would overlap another label, an overlay or the canvas edge are skipped.
 */
function drawLandmarks(
	ctx: CanvasRenderingContext2D,
	camera: Camera,
	scene: Scene,
	visible: SceneNode[],
	state: RenderState,
	dimmed: (n: SceneNode) => boolean
) {
	const { theme } = state;
	const z = camera.zoom;
	ctx.font = `600 12px ${theme.font}`;
	ctx.textBaseline = 'middle';
	const shown = new Set(visible);
	const edge = 6;
	const blocked: Box[] = camera.overlays.map((r: Rect) => ({
		x: r.x,
		y: r.y,
		w: r.width,
		h: r.height
	}));
	const taken: Box[] = [];
	const cw = scene.width * z;
	const ch = scene.height * z;
	let count = 0;
	let tries = 0;
	for (const { node: n, labels } of landmarkRanking(ctx, scene)) {
		if (count >= LANDMARKS || tries >= LANDMARK_TRIES) break;
		if (!shown.has(n) || dimmed(n)) continue;
		tries++;
		const card = { x: camera.x + n.x * z, y: camera.y + n.y * z, w: cw, h: ch };
		const cx = card.x + cw / 2;
		// The most complete label that covers no neighbouring card, else the one covering fewest.
		let best: [Box, number, LandmarkLabel] | null = null;
		let bestCovered = 3; // more covered neighbours than this: skip the label
		for (const label of labels) {
			const tw = label.width + 12;
			const th = label.lines.length * LANDMARK_LINE + 6;
			// Centred on the card, shifted inward at the canvas edges while still over the card.
			const left = Math.min(
				Math.max(cx - tw / 2, edge, card.x - tw + 12),
				camera.width - edge - tw,
				card.x + cw - 12
			);
			const candidates: [Box, number][] = [
				[{ x: left, y: card.y + ch / 2 - th / 2, w: tw, h: th }, 0],
				[{ x: left, y: card.y - th - 3, w: tw, h: th }, -1],
				[{ x: left, y: card.y + ch + 3, w: tw, h: th }, 1]
			];
			for (const [r, side] of candidates) {
				if (r.x < edge || r.y < edge || r.x + r.w > camera.width - edge) continue;
				if (r.y + r.h > camera.height - edge) continue;
				if (blocked.some((b) => overlaps(r, b, 4)) || taken.some((t) => overlaps(r, t, 4)))
					continue;
				const world = {
					x: (r.x - camera.x) / z,
					y: (r.y - camera.y) / z,
					width: r.w / z,
					height: r.h / z
				};
				let covered = 0;
				for (const m of nodesIn(scene, world)) if (m !== n) covered++;
				if (covered < bestCovered) {
					best = [r, side, label];
					bestCovered = covered;
					if (!covered) break;
				}
			}
			if (best && !bestCovered) break;
		}
		if (!best) continue;
		const [r, side, label] = best;
		taken.push(r);
		count++;
		const color = accent(theme, n);
		if (side) {
			// Leader from the label to the middle of its card.
			const from = side < 0 ? r.y + r.h : r.y;
			const to = side < 0 ? card.y + Math.min(ch / 2, 4) : card.y + ch - Math.min(ch / 2, 4);
			ctx.strokeStyle = color;
			ctx.lineWidth = 1.5;
			ctx.beginPath();
			ctx.moveTo(cx, from);
			ctx.lineTo(cx, to);
			ctx.stroke();
			ctx.fillStyle = color;
			ctx.beginPath();
			ctx.arc(cx, to, 2.5, 0, 2 * Math.PI);
			ctx.fill();
		}
		ctx.globalAlpha = 0.92;
		ctx.fillStyle = theme.card;
		ctx.beginPath();
		ctx.roundRect(r.x, r.y, r.w, r.h, 6);
		ctx.fill();
		ctx.globalAlpha = 1;
		ctx.strokeStyle = color;
		ctx.lineWidth = 1.5;
		ctx.stroke();
		ctx.fillStyle = theme.cardForeground;
		label.lines.forEach((line, i) =>
			ctx.fillText(line, r.x + 6, r.y + 3 + (i + 0.5) * LANDMARK_LINE)
		);
	}
}

function strokeEdges(
	ctx: CanvasRenderingContext2D,
	paths: EdgePaths,
	color: string,
	width: number,
	px: number
) {
	ctx.strokeStyle = color;
	ctx.lineWidth = width;
	ctx.stroke(paths.solid);
	ctx.setLineDash([Math.max(6, 6 * px), Math.max(4, 4 * px)]);
	ctx.stroke(paths.dashed);
	ctx.setLineDash([]);
}

/**
 * Full card content: icon, name, cost/kind badges and flag glyphs (world px, like the old
 * DOM card). Secondary cards get muted text; faded ones also fade their icon and badges.
 */
function drawDetail(
	ctx: CanvasRenderingContext2D,
	n: SceneNode,
	scene: Scene,
	state: RenderState,
	secondary: boolean,
	faded: boolean
) {
	const { theme } = state;
	const h = scene.height;
	const text = secondary ? theme.muted : theme.cardForeground;
	const marks = faded ? DIM_CONTENT : 1;
	const icon = state.icons[n.item.category];
	const iconX = n.x + 4 + 12;
	ctx.globalAlpha = marks;
	if (icon?.complete && icon.naturalWidth) ctx.drawImage(icon, iconX, n.y + h / 2 - 18, 36, 36);
	ctx.globalAlpha = 1;

	const textX = iconX + 36 + 12;
	const maxWidth = n.x + scene.width - 12 - textX;
	const top = n.y + (h - 44) / 2;
	ctx.font = `500 14px ${theme.font}`;
	ctx.textBaseline = 'middle';
	ctx.fillStyle = text;
	ctx.fillText(ellipsize(ctx, n, maxWidth), textX, top + 9);

	// Badges (h-5 = 20px), as shadcn's secondary/outline Badge.
	ctx.font = `500 12px ${theme.font}`;
	let bx = textX;
	const by = top + 24;
	const badge = (label: string, filled: boolean) => {
		const width = ctx.measureText(label).width + 16;
		ctx.beginPath();
		ctx.roundRect(bx, by, width, 20, 10);
		ctx.globalAlpha = marks;
		if (filled) {
			ctx.fillStyle = theme.secondary;
			ctx.fill();
		} else {
			ctx.strokeStyle = theme.border;
			ctx.lineWidth = 1;
			ctx.stroke();
		}
		ctx.globalAlpha = faded ? 0.7 : 1;
		ctx.fillStyle = secondary ? theme.muted : filled ? theme.secondaryForeground : text;
		ctx.fillText(label, bx + 8, by + 10.5);
		ctx.globalAlpha = 1;
		bx += width + 4;
	};
	badge(n.cost, true);
	badge(n.kind, false);

	// Flag glyphs: 14px lucide icons in 20px boxes; what doesn't fit becomes "+N".
	ctx.lineWidth = 2;
	ctx.lineCap = 'round';
	ctx.lineJoin = 'round';
	const right = n.x + scene.width - 12;
	for (const [i, f] of n.flags.entries()) {
		const rest = n.flags.length - i;
		if (bx + 20 > right || (rest > 1 && bx + 44 > right)) {
			ctx.fillStyle = theme.muted;
			ctx.fillText(`+${rest}`, bx + 2, by + 10.5);
			break;
		}
		const glyph = state.glyphs.get(f.key);
		if (!glyph) continue;
		// Glyphs carry meaning: full contrast even on faded cards (like the text).
		ctx.save();
		ctx.translate(bx + 3, by + 3);
		ctx.scale(14 / 24, 14 / 24);
		ctx.strokeStyle = f.inverted ? theme.destructive : theme.muted;
		ctx.stroke(glyph);
		ctx.restore();
		bx += 24;
	}
	ctx.globalAlpha = 1;
}

/** Name cut to `maxWidth` with an ellipsis (cached per node and font). */
function ellipsize(ctx: CanvasRenderingContext2D, n: SceneNode, maxWidth: number) {
	const cacheKey = `e:${ctx.font}:${maxWidth}`;
	const cached = n.labels.get(cacheKey);
	if (cached) return cached[0];
	let text = n.item.name;
	if (ctx.measureText(text).width > maxWidth) {
		let lo = 0,
			hi = text.length;
		while (lo < hi) {
			const mid = (lo + hi + 1) >> 1;
			if (ctx.measureText(text.slice(0, mid) + '…').width <= maxWidth) lo = mid;
			else hi = mid - 1;
		}
		text = text.slice(0, lo).trimEnd() + '…';
	}
	n.labels.set(cacheKey, [text]);
	return text;
}

/** Name cut in the middle to `maxWidth`, keeping its first word and as much of its end as fits. */
function middleEllipsize(ctx: CanvasRenderingContext2D, name: string, maxWidth: number) {
	const words = name.split(/\s+/);
	const fits = (text: string) => ctx.measureText(text).width <= maxWidth;
	for (let i = 1; i < words.length; i++) {
		const text = `${words[0]}…${words.slice(i).join(' ')}`;
		if (fits(text)) return text;
	}
	// Even the last word is too long: keep as many of its last characters as fit.
	const last = words[words.length - 1];
	for (let i = 1; i < last.length; i++) {
		const text = `${words[0]}…${last.slice(i)}`;
		if (fits(text)) return text;
	}
	return `…${last.slice(-8)}`;
}

/** Name wrapped into at most `maxLines` lines (last one ellipsized; cached per node and font). */
function wrap(ctx: CanvasRenderingContext2D, n: SceneNode, maxWidth: number, maxLines: number) {
	const cacheKey = `w:${ctx.font}:${maxWidth}:${maxLines}`;
	const cached = n.labels.get(cacheKey);
	if (cached) return cached;
	const words = n.item.name.split(/\s+/);
	const lines: string[] = [];
	let line = '';
	for (let i = 0; i < words.length; i++) {
		const candidate = line ? `${line} ${words[i]}` : words[i];
		if (ctx.measureText(candidate).width <= maxWidth || !line) {
			line = candidate;
			continue;
		}
		if (lines.length === maxLines - 1) {
			line = `${line} ${words.slice(i).join(' ')}`;
			break;
		}
		lines.push(line);
		line = words[i];
	}
	if (line) lines.push(line);
	const last = lines.length - 1;
	if (ctx.measureText(lines[last]).width > maxWidth) {
		let text = lines[last];
		while (text.length > 1 && ctx.measureText(text + '…').width > maxWidth)
			text = text.slice(0, -1);
		lines[last] = text.trimEnd() + '…';
	}
	n.labels.set(cacheKey, lines);
	return lines;
}
