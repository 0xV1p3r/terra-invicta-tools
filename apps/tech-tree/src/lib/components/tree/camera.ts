// Pan/zoom state of the tree view: screen = world * zoom + (x, y), in CSS pixels.
// Deliberately not reactive: it changes every frame while panning; listeners schedule
// a redraw instead of going through Svelte's reactivity.

export const MIN_ZOOM = 0.04;
export const MAX_ZOOM = 2;
/** Below this zoom a whole-tree fit is unreadable: fit the width and center vertically instead. */
const MIN_FIT_ZOOM = 0.12;

export interface Rect {
	x: number;
	y: number;
	width: number;
	height: number;
}

export interface View {
	x: number;
	y: number;
	zoom: number;
}

const clampZoom = (z: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));
const easeOut = (t: number) => 1 - Math.pow(1 - t, 3);

export class Camera {
	x = 0;
	y = 0;
	zoom = 1;
	/** Viewport size in CSS pixels. */
	width = 0;
	height = 0;
	/** Screen rectangles covered by floating overlays (toolbar, minimap, controls). */
	overlays: Rect[] = [];

	private listeners = new Set<() => void>();
	private animation = 0;
	/** Settles the running `animateTo` promise (with `false` when interrupted). */
	private settle: ((completed: boolean) => void) | null = null;

	/** Called after every change; returns an unsubscribe function. */
	subscribe(listener: () => void) {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	}

	private apply(x: number, y: number, zoom: number) {
		this.x = x;
		this.y = y;
		this.zoom = zoom;
		for (const l of this.listeners) l();
	}

	set(view: View) {
		this.stop();
		this.apply(view.x, view.y, clampZoom(view.zoom));
	}

	stop() {
		cancelAnimationFrame(this.animation);
		this.animation = 0;
		this.finish(false);
	}

	private finish(completed: boolean) {
		const settle = this.settle;
		this.settle = null;
		settle?.(completed);
	}

	resize(width: number, height: number) {
		this.width = width;
		this.height = height;
		for (const l of this.listeners) l();
	}

	toWorld(sx: number, sy: number) {
		return { x: (sx - this.x) / this.zoom, y: (sy - this.y) / this.zoom };
	}

	/** Visible world rectangle. */
	visible(): Rect {
		return {
			x: -this.x / this.zoom,
			y: -this.y / this.zoom,
			width: this.width / this.zoom,
			height: this.height / this.zoom
		};
	}

	panBy(dx: number, dy: number) {
		this.stop();
		this.apply(this.x + dx, this.y + dy, this.zoom);
	}

	/** Zoom by `factor` keeping the screen point (sx, sy) fixed. */
	zoomAt(sx: number, sy: number, factor: number) {
		this.stop();
		const zoom = clampZoom(this.zoom * factor);
		const k = zoom / this.zoom;
		this.apply(sx - (sx - this.x) * k, sy - (sy - this.y) * k, zoom);
	}

	/** View that centers world point (wx, wy) at `zoom`. */
	centered(wx: number, wy: number, zoom = this.zoom): View {
		const z = clampZoom(zoom);
		return { x: this.width / 2 - wx * z, y: this.height / 2 - wy * z, zoom: z };
	}

	/**
	 * Largest screen area clear of the overlays that fits `bounds`: each overlay is avoided
	 * either by its rows (top/bottom strip) or by its columns (left/right strip).
	 */
	private freeArea(bounds: Rect): Rect {
		const rects = this.overlays
			.filter(
				(r) => r.x < this.width && r.y < this.height && r.x + r.width > 0 && r.y + r.height > 0
			)
			.slice(0, 8);
		let best: Rect = { x: 0, y: 0, width: this.width, height: this.height };
		let bestScore = -1;
		for (let mask = 0; mask < 1 << rects.length; mask++) {
			let [left, top, right, bottom] = [0, 0, this.width, this.height];
			rects.forEach((r, i) => {
				const byRows = (mask >> i) & 1;
				if (byRows && r.y + r.height / 2 < this.height / 2) top = Math.max(top, r.y + r.height);
				else if (byRows) bottom = Math.min(bottom, r.y);
				else if (r.x + r.width / 2 < this.width / 2) left = Math.max(left, r.x + r.width);
				else right = Math.min(right, r.x);
			});
			const [w, h] = [right - left, bottom - top];
			if (w <= 0 || h <= 0) continue;
			// Prefer the larger zoom (as `fitted` picks it), then the larger area.
			const fit = Math.min(w / bounds.width, h / bounds.height);
			const score = (fit >= MIN_FIT_ZOOM ? fit : w / bounds.width) * 1e6 + w * h * 1e-6;
			if (score > bestScore) {
				bestScore = score;
				best = { x: left, y: top, width: w, height: h };
			}
		}
		return best;
	}

	/**
	 * View showing `bounds` clear of the overlays (or, for huge trees, its full width
	 * centered vertically). `margin` (screen px) leaves room for outlines and labels.
	 */
	fitted(bounds: Rect, padding = 0.04, margin = 24): View {
		const a = this.freeArea(bounds);
		const aw = Math.max(1, a.width - 2 * margin);
		const ah = Math.max(1, a.height - 2 * margin);
		const fit = Math.min(aw / bounds.width, ah / bounds.height) * (1 - 2 * padding);
		const cy = bounds.y + bounds.height / 2;
		const at = (wx: number, sx: number, zoom: number): View => {
			const z = clampZoom(zoom);
			return { x: sx - wx * z, y: a.y + a.height / 2 - cy * z, zoom: z };
		};
		if (fit >= MIN_FIT_ZOOM)
			return at(bounds.x + bounds.width / 2, a.x + a.width / 2, Math.min(fit, 1));
		const zoom = Math.min(Math.max(aw / bounds.width, MIN_FIT_ZOOM), 1);
		return zoom * bounds.width <= aw
			? at(bounds.x + bounds.width / 2, a.x + a.width / 2, zoom)
			: at(bounds.x, a.x + margin, zoom);
	}

	/**
	 * Animate to `target` (interpolating in screen space, zoom geometrically). Resolves to
	 * whether it completed (`false` if interrupted by another camera change).
	 */
	animateTo(target: View, duration = 300): Promise<boolean> {
		this.stop();
		const from: View = { x: this.x, y: this.y, zoom: this.zoom };
		const to: View = { ...target, zoom: clampZoom(target.zoom) };
		if (duration <= 0) {
			this.apply(to.x, to.y, to.zoom);
			return Promise.resolve(true);
		}
		// Interpolate the world point at the screen center so the motion looks straight.
		const center = (v: View) => ({
			x: (this.width / 2 - v.x) / v.zoom,
			y: (this.height / 2 - v.y) / v.zoom
		});
		const c0 = center(from);
		const c1 = center(to);
		const start = performance.now();
		return new Promise((resolve) => {
			this.settle = resolve;
			const step = (now: number) => {
				const t = Math.min(1, (now - start) / duration);
				const e = easeOut(t);
				const zoom = from.zoom * Math.pow(to.zoom / from.zoom, e);
				const cx = c0.x + (c1.x - c0.x) * e;
				const cy = c0.y + (c1.y - c0.y) * e;
				this.apply(this.width / 2 - cx * zoom, this.height / 2 - cy * zoom, zoom);
				if (t < 1) this.animation = requestAnimationFrame(step);
				else {
					this.animation = 0;
					this.finish(true);
				}
			};
			this.animation = requestAnimationFrame(step);
		});
	}
}
