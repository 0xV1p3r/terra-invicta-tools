// Canvas renderer of the research tree. Everything is drawn in world coordinates with
// one transform per frame; detail (text, icons, badges) depends on the zoom so a frame
// costs about the same whether 30 or 900 cards are visible.
import type { EntityKey, TechCategory } from '$lib/api';
import { CATEGORIES } from '$lib/categories';
import type { Camera } from './camera';
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
/**
 * Opacity of the fill, border and category bar of cards outside the selection's lineage
 * or the filter matches. Their text switches to the muted color instead of fading, so it
 * stays legible.
 */
const DIM = 0.3;
/** Opacity of the icon and badges of such cards. */
const DIM_CONTENT = 0.55;

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
	const fade = Math.min(1, Math.max(0.2, z * 2)) * (overview ? 0.5 : 1);
	ctx.globalAlpha = lineage ? 0.15 * fade : matches ? 0.4 * fade : fade;
	strokeEdges(ctx, scene.edges, theme.muted, Math.max(1.5, 0.5 * px), px);
	if (lineage) {
		ctx.globalAlpha = 1;
		strokeEdges(ctx, lineage.edges, theme.foreground, Math.max(2, px), px);
	}
	const highlight = lineage && state.selected ? scene.edgesOf.get(state.selected) : undefined;
	if (highlight) {
		ctx.globalAlpha = 1;
		strokeEdges(ctx, highlight, theme.foreground, Math.max(3, 1.5 * px), px);
	}

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
				ctx.strokeStyle = dashed && !outside ? theme.muted : theme.border;
				ctx.lineWidth = px;
				ctx.setLineDash(dashed ? [6 * px, 4 * px] : []);
				ctx.stroke(body);
				ctx.setLineDash([]);
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

const rankingCache = new WeakMap<
	Scene,
	{ font: string; ranking: { node: SceneNode; width: number }[] }
>();

/** Nodes by importance with their label width in the current font (cached per scene). */
function landmarkRanking(ctx: CanvasRenderingContext2D, scene: Scene) {
	const cached = rankingCache.get(scene);
	if (cached?.font === ctx.font) return cached.ranking;
	const importance = importanceOf(scene);
	const ranking = [...scene.nodes]
		.sort((a, b) => (importance.get(b.key) ?? 0) - (importance.get(a.key) ?? 0))
		.map((node) => ({ node, width: ctx.measureText(node.item.name).width }));
	rankingCache.set(scene, { font: ctx.font, ranking });
	return ranking;
}

/** Labels of the most important visible research, skipping ones that would overlap. */
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
	const taken: { x: number; y: number; w: number; h: number }[] = [];
	let count = 0;
	for (const { node: n, width } of landmarkRanking(ctx, scene)) {
		if (count >= LANDMARKS) break;
		if (!shown.has(n) || dimmed(n)) continue;
		const label = n.item.name;
		const tw = width + 12;
		const th = 20;
		const cx = camera.x + (n.x + scene.width / 2) * z;
		const cy = camera.y + (n.y + scene.height / 2) * z;
		const r = { x: cx - tw / 2, y: cy - th / 2, w: tw, h: th };
		const pad = 4;
		if (
			taken.some(
				(t) =>
					r.x < t.x + t.w + pad &&
					r.x + r.w + pad > t.x &&
					r.y < t.y + t.h + pad &&
					r.y + r.h + pad > t.y
			)
		)
			continue;
		taken.push(r);
		count++;
		ctx.globalAlpha = 0.92;
		ctx.fillStyle = theme.card;
		ctx.beginPath();
		ctx.roundRect(r.x, r.y, r.w, r.h, 6);
		ctx.fill();
		ctx.globalAlpha = 1;
		ctx.strokeStyle = accent(theme, n);
		ctx.lineWidth = 1.5;
		ctx.stroke();
		ctx.fillStyle = theme.cardForeground;
		ctx.fillText(label, r.x + 6, cy);
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
		ctx.globalAlpha = 1;
		ctx.fillStyle = secondary ? theme.muted : filled ? theme.secondaryForeground : text;
		ctx.fillText(label, bx + 8, by + 10.5);
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
		ctx.save();
		ctx.globalAlpha = marks;
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
