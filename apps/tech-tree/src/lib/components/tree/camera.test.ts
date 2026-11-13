import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { Camera, MAX_ZOOM, MIN_ZOOM } from './camera';

beforeEach(() => vi.stubGlobal('cancelAnimationFrame', vi.fn()));
afterEach(() => vi.unstubAllGlobals());

it('round-trips world coordinates through the screen transform', () => {
	const c = new Camera();
	c.set({ x: -43, y: 89, zoom: 0.3 });
	for (const [x, y] of [
		[0, 0],
		[-123, 654],
		[10000, -200]
	]) {
		const p = c.toWorld(x * c.zoom + c.x, y * c.zoom + c.y);
		expect(p.x).toBeCloseTo(x);
		expect(p.y).toBeCloseTo(y);
	}
});

it('keeps the zoom anchor fixed even when reaching a zoom limit', () => {
	const c = new Camera();
	c.panBy(34, -57);
	const anchor = c.toWorld(123, 456);
	for (const factor of [1.5, 1000, 0.00001]) {
		c.zoomAt(123, 456, factor);
		expect(c.toWorld(123, 456).x).toBeCloseTo(anchor.x);
		expect(c.toWorld(123, 456).y).toBeCloseTo(anchor.y);
	}
	expect(c.zoom).toBe(MIN_ZOOM);
	c.set({ x: 0, y: 0, zoom: 100 });
	expect(c.zoom).toBe(MAX_ZOOM);
});

it('honors custom limits when centering', () => {
	const c = new Camera({ minZoom: 0.2, maxZoom: 4 });
	c.resize(800, 600);
	c.set(c.centered(-50, 70, 20));
	expect(c.zoom).toBe(4);
	expect(c.toWorld(400, 300)).toEqual({ x: -50, y: 70 });
});

it('fits ordinary bounds inside the viewport with margin and clear of overlays', () => {
	const c = new Camera();
	c.resize(1000, 800);
	c.overlays = [{ x: 0, y: 0, width: 200, height: 800 }];
	const b = { x: -300, y: 40, width: 1200, height: 900 };
	c.set(c.fitted(b));
	expect(b.x * c.zoom + c.x).toBeGreaterThanOrEqual(224);
	expect((b.x + b.width) * c.zoom + c.x).toBeLessThanOrEqual(976);
	expect(b.y * c.zoom + c.y).toBeGreaterThanOrEqual(24);
	expect((b.y + b.height) * c.zoom + c.y).toBeLessThanOrEqual(776);
});

it('uses a readable width fit for a very tall tree', () => {
	const c = new Camera();
	c.resize(1000, 800);
	c.set(c.fitted({ x: 0, y: 0, width: 500, height: 100000 }));
	expect(c.zoom).toBe(1);
	expect(c.toWorld(500, 400).y).toBe(50000);
});
