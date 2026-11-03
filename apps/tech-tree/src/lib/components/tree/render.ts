// Canvas renderer of the research tree. Everything is drawn in world coordinates with
// one transform per frame; detail (text, icons, badges) depends on the zoom so a frame
// costs about the same whether 30 or 900 cards are visible.
import type { EntityKey, TechCategory } from '$lib/api';
import type { Camera } from './camera';
import { nodesIn, type EdgePaths, type Scene, type SceneNode } from './scene';

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
		font: s.fontFamily || 'sans-serif'
	};
}

export interface RenderState {
	selected: EntityKey | null;
	hovered: EntityKey | null;
	/** Subtree root (dashed outline). */
	root: EntityKey | null;
	theme: Theme;
	icons: Partial<Record<TechCategory, HTMLImageElement>>;
	/** Flag glyphs as Path2D (24×24 box), by flag key. */
	glyphs: Map<string, Path2D>;
}

/** Zoom from which cards show icon, name and badges. */
export const DETAIL_ZOOM = 0.45;
/** Zoom from which zoomed-out cards show their (large) name. */
const NAME_ZOOM = 0.18;

const RADIUS = 14;
const BIG_FONT_PX = 26;

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
	const px = 1 / z; // one screen pixel in world units

	// --- edges ---------------------------------------------------------------
	const fade = Math.min(1, Math.max(0.2, z * 2));
	ctx.globalAlpha = state.selected ? 0.15 * fade : fade;
	strokeEdges(ctx, scene.edges, theme.muted, Math.max(1.5, 0.5 * px), px);
	const highlight = state.selected ? scene.edgesOf.get(state.selected) : undefined;
	if (highlight) {
		ctx.globalAlpha = 1;
		strokeEdges(ctx, highlight, theme.foreground, Math.max(3, 1.5 * px), px);
	}

	// --- card bodies, borders and category bars (batched per style) -----------
	const bar = detail ? 4 : 12;
	for (const dim of [false, true]) {
		const group = visible.filter((n) => n.item.flags.obtainable === !dim);
		if (!group.length) continue;
		ctx.globalAlpha = dim ? 0.5 : 1;
		const body = new Path2D();
		for (const n of group) body.roundRect(n.x, n.y, w, h, RADIUS);
		ctx.fillStyle = theme.card;
		ctx.fill(body);
		if (z > 0.12) {
			ctx.strokeStyle = theme.border;
			ctx.lineWidth = px;
			ctx.setLineDash(dim ? [6 * px, 4 * px] : []);
			ctx.stroke(body);
			ctx.setLineDash([]);
		}
		const bars = new Map<string, Path2D>();
		for (const n of group) {
			let p = bars.get(n.color);
			if (!p) bars.set(n.color, (p = new Path2D()));
			p.roundRect(n.x, n.y, bar, h, [RADIUS, 0, 0, RADIUS]);
		}
		for (const [color, p] of bars) {
			ctx.fillStyle = color;
			ctx.fill(p);
		}
	}

	// --- content --------------------------------------------------------------
	if (detail) {
		for (const n of visible) drawDetail(ctx, n, scene, state);
	} else if (z >= NAME_ZOOM) {
		ctx.font = `600 ${BIG_FONT_PX}px ${theme.font}`;
		ctx.textBaseline = 'middle';
		ctx.fillStyle = theme.cardForeground;
		const maxWidth = w - bar - 24;
		const lineHeight = BIG_FONT_PX * 1.15;
		for (const n of visible) {
			ctx.globalAlpha = n.item.flags.obtainable ? 1 : 0.5;
			const lines = wrap(ctx, n, maxWidth, 2);
			const top = n.y + h / 2 - ((lines.length - 1) * lineHeight) / 2;
			lines.forEach((line, i) => ctx.fillText(line, n.x + bar + 12, top + i * lineHeight));
		}
	}
	ctx.globalAlpha = 1;

	// --- hover, selection, subtree root ----------------------------------------
	const outline = (key: EntityKey | null, draw: (n: SceneNode) => void) => {
		const n = key ? scene.byKey.get(key) : undefined;
		if (n) draw(n);
	};
	outline(state.hovered, (n) => {
		ctx.strokeStyle = n.color;
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
		ctx.strokeStyle = n.color;
		ctx.globalAlpha = 0.35;
		ctx.lineWidth = 12 * px;
		ctx.beginPath();
		ctx.roundRect(n.x - 3 * px, n.y - 3 * px, w + 6 * px, h + 6 * px, RADIUS + 3 * px);
		ctx.stroke();
		ctx.globalAlpha = 1;
		ctx.lineWidth = 3 * px;
		ctx.stroke();
	});
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

/** Full card content: icon, name, cost/kind badges and flag glyphs (world px, like the old DOM card). */
function drawDetail(ctx: CanvasRenderingContext2D, n: SceneNode, scene: Scene, state: RenderState) {
	const { theme } = state;
	const h = scene.height;
	ctx.globalAlpha = n.item.flags.obtainable ? 1 : 0.5;
	const icon = state.icons[n.item.category];
	const iconX = n.x + 4 + 12;
	if (icon?.complete && icon.naturalWidth) ctx.drawImage(icon, iconX, n.y + h / 2 - 18, 36, 36);

	const textX = iconX + 36 + 12;
	const maxWidth = n.x + scene.width - 12 - textX;
	const top = n.y + (h - 44) / 2;
	ctx.font = `500 14px ${theme.font}`;
	ctx.textBaseline = 'middle';
	ctx.fillStyle = theme.cardForeground;
	ctx.fillText(ellipsize(ctx, n, maxWidth), textX, top + 9);

	// Badges (h-5 = 20px), as shadcn's secondary/outline Badge.
	ctx.font = `500 12px ${theme.font}`;
	let bx = textX;
	const by = top + 24;
	const badge = (text: string, filled: boolean) => {
		const width = ctx.measureText(text).width + 16;
		ctx.beginPath();
		ctx.roundRect(bx, by, width, 20, 10);
		if (filled) {
			ctx.fillStyle = theme.secondary;
			ctx.fill();
			ctx.fillStyle = theme.secondaryForeground;
		} else {
			ctx.strokeStyle = theme.border;
			ctx.lineWidth = 1;
			ctx.stroke();
			ctx.fillStyle = theme.cardForeground;
		}
		ctx.fillText(text, bx + 8, by + 10.5);
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
