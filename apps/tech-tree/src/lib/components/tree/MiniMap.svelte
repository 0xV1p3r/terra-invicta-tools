<script lang="ts">
	// Overview of the whole tree: nodes pre-drawn once per scene into an offscreen canvas;
	// each camera change only blits it and draws the viewport frame. Click or drag to pan.
	import { onDestroy } from 'svelte';
	import * as Card from '#lib/components/ui/card/index.js';
	import { settings, type Corner } from '#lib/settings.svelte.ts';
	import type { Camera } from './camera';
	import type { Theme } from './render';
	import type { EntityKey } from '#lib/api.ts';
	import type { Scene } from './scene';

	let {
		scene,
		camera,
		theme,
		matches = null
	}: {
		scene: Scene;
		camera: Camera;
		theme: Theme;
		/** Filter matches in highlight mode; the rest are faded. */
		matches?: ReadonlySet<EntityKey> | null;
	} = $props();

	const WIDTH = 200;
	const HEIGHT = 150;
	const PAD = 6;
	const CORNER_CLASS: Record<Corner, string> = {
		'top-left': 'top-3 left-3',
		'top-right': 'top-3 right-3',
		'bottom-left': 'bottom-3 left-3',
		'bottom-right': 'right-3 bottom-3'
	};

	let canvas = $state<HTMLCanvasElement>();
	const dpr = typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1;

	/** World → minimap transform for the current scene. */
	const fit = $derived.by(() => {
		const b = scene.bounds;
		const scale = Math.min((WIDTH - 2 * PAD) / b.width, (HEIGHT - 2 * PAD) / b.height);
		return {
			scale,
			ox: PAD + (WIDTH - 2 * PAD - b.width * scale) / 2 - b.x * scale,
			oy: PAD + (HEIGHT - 2 * PAD - b.height * scale) / 2 - b.y * scale
		};
	});

	const nodesLayer = $derived.by(() => {
		const layer = document.createElement('canvas');
		layer.width = WIDTH * dpr;
		layer.height = HEIGHT * dpr;
		const ctx = layer.getContext('2d')!;
		ctx.scale(dpr, dpr);
		const { scale, ox, oy } = fit;
		const w = Math.max(1.5, scene.width * scale);
		const h = Math.max(1.5, scene.height * scale);
		for (const n of scene.nodes) {
			ctx.globalAlpha = matches && !matches.has(n.key) ? 0.15 : 1;
			ctx.fillStyle = theme.categories[n.item.category] ?? n.color;
			ctx.fillRect(ox + n.x * scale, oy + n.y * scale, w, h);
		}
		return layer;
	});

	let frame = 0;
	function draw() {
		frame = 0;
		const ctx = canvas?.getContext('2d');
		if (!ctx || !canvas) return;
		canvas.width = WIDTH * dpr;
		canvas.height = HEIGHT * dpr;
		ctx.drawImage(nodesLayer, 0, 0);
		ctx.scale(dpr, dpr);
		const { scale, ox, oy } = fit;
		const v = camera.visible();
		// The viewport frame, kept inside the minimap's border (it may be larger than the tree).
		const inset = 2;
		const x1 = Math.max(inset, ox + v.x * scale);
		const y1 = Math.max(inset, oy + v.y * scale);
		const x2 = Math.min(WIDTH - inset, ox + (v.x + v.width) * scale);
		const y2 = Math.min(HEIGHT - inset, oy + (v.y + v.height) * scale);
		if (x2 <= x1 || y2 <= y1) return;
		const [rx, ry, rw, rh] = [x1, y1, x2 - x1, y2 - y1];
		// The whole tree in view: nothing to point out, so no frame.
		const b = scene.bounds;
		const all =
			v.x <= b.x &&
			v.y <= b.y &&
			v.x + v.width >= b.x + b.width &&
			v.y + v.height >= b.y + b.height;
		if (all) return;
		// Recede everything outside the viewport frame (marks stay readable), then frame it
		// with a halo so it stands out on both dense and empty areas.
		ctx.fillStyle = theme.background;
		ctx.globalAlpha = 0.45;
		ctx.beginPath();
		ctx.rect(0, 0, WIDTH, HEIGHT);
		ctx.rect(rx, ry, rw, rh);
		ctx.fill('evenodd');
		ctx.globalAlpha = 0.08;
		ctx.fillStyle = theme.foreground;
		ctx.fillRect(rx, ry, rw, rh);
		ctx.globalAlpha = 0.8;
		ctx.strokeStyle = theme.background;
		ctx.lineWidth = 3;
		ctx.strokeRect(rx, ry, rw, rh);
		ctx.globalAlpha = 1;
		ctx.strokeStyle = theme.foreground;
		ctx.lineWidth = 1.5;
		ctx.strokeRect(rx, ry, rw, rh);
	}

	function requestDraw() {
		if (!frame) frame = requestAnimationFrame(draw);
	}

	$effect(() => camera.subscribe(requestDraw));
	onDestroy(() => cancelAnimationFrame(frame));
	$effect(() => {
		void [nodesLayer, theme];
		requestDraw();
	});

	let dragging = false;
	function panTo(e: PointerEvent) {
		const r = canvas!.getBoundingClientRect();
		const { scale, ox, oy } = fit;
		const wx = (e.clientX - r.left - ox) / scale;
		const wy = (e.clientY - r.top - oy) / scale;
		camera.set(camera.centered(wx, wy));
	}
</script>

<Card.Root
	data-overlay
	class="absolute {CORNER_CLASS[settings.minimapCorner]} overflow-hidden p-0 shadow"
>
	<canvas
		bind:this={canvas}
		class="block cursor-pointer"
		style="width: {WIDTH}px; height: {HEIGHT}px"
		aria-hidden="true"
		title="Tree overview: click or drag to move the view"
		onpointerdown={(e) => {
			dragging = true;
			canvas?.setPointerCapture(e.pointerId);
			panTo(e);
		}}
		onpointermove={(e) => dragging && panTo(e)}
		onpointerup={() => (dragging = false)}
		onpointercancel={() => (dragging = false)}
	></canvas>
</Card.Root>
