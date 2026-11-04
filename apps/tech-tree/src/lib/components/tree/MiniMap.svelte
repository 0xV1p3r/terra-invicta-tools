<script lang="ts">
	// Overview of the whole tree: nodes pre-drawn once per scene into an offscreen canvas;
	// each camera change only blits it and draws the viewport frame. Click or drag to pan.
	import { onDestroy } from 'svelte';
	import { mode } from 'mode-watcher';
	import * as Card from '$lib/components/ui/card/index.js';
	import { settings, type Corner } from '$lib/settings.svelte';
	import type { Camera } from './camera';
	import type { Scene } from './scene';

	let { scene, camera }: { scene: Scene; camera: Camera } = $props();

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
		const w = Math.max(1, scene.width * scale);
		const h = Math.max(1, scene.height * scale);
		for (const n of scene.nodes) {
			ctx.fillStyle = n.color;
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
		const rx = ox + v.x * scale;
		const ry = oy + v.y * scale;
		const rw = v.width * scale;
		const rh = v.height * scale;
		// Dim everything outside the viewport frame.
		const style = getComputedStyle(canvas);
		ctx.fillStyle = style.getPropertyValue('--background');
		ctx.globalAlpha = 0.65;
		ctx.beginPath();
		ctx.rect(0, 0, WIDTH, HEIGHT);
		ctx.rect(rx, ry, rw, rh);
		ctx.fill('evenodd');
		ctx.globalAlpha = 1;
		ctx.strokeStyle = style.getPropertyValue('--foreground');
		ctx.lineWidth = 1;
		ctx.strokeRect(rx, ry, rw, rh);
	}

	function requestDraw() {
		if (!frame) frame = requestAnimationFrame(draw);
	}

	$effect(() => camera.subscribe(requestDraw));
	onDestroy(() => cancelAnimationFrame(frame));
	$effect(() => {
		void [nodesLayer, mode.current];
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

<Card.Root class="absolute {CORNER_CLASS[settings.minimapCorner]} overflow-hidden p-0 shadow">
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
