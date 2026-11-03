// Pan/zoom state of the tree view: screen = world * zoom + (x, y), in CSS pixels.
// Deliberately not reactive: it changes every frame while panning; listeners schedule
// a redraw instead of going through Svelte's reactivity.

export const MIN_ZOOM = 0.04;
export const MAX_ZOOM = 2;
/** Below this zoom a whole-tree fit is unreadable: fit the width and center vertically instead. */
const MIN_FIT_ZOOM = 0.15;

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

	/** View showing `bounds` (or, for huge trees, its full width centered vertically). */
	fitted(bounds: Rect, padding = 0.08): View {
		const fit =
			Math.min(this.width / bounds.width, this.height / bounds.height) * (1 - 2 * padding);
		const cx = bounds.x + bounds.width / 2;
		const cy = bounds.y + bounds.height / 2;
		if (fit >= MIN_FIT_ZOOM) return this.centered(cx, cy, Math.min(fit, 1));
		const zoom = Math.min(Math.max((this.width / bounds.width) * 0.95, MIN_FIT_ZOOM), 1);
		const x = zoom * bounds.width <= this.width ? cx : bounds.x + this.width / zoom / 2 - 40;
		return this.centered(x, cy, zoom);
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
