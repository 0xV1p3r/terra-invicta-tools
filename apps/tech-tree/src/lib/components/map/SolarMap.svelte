<script lang="ts">
	// The solar system map: one system (a body and its satellites) at a time, drawn on one
	// <canvas> by `render.ts`, with DOM overlays for the breadcrumb, date and zoom controls.
	import { onDestroy, onMount, untrack } from 'svelte';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import LogIn from '@lucide/svelte/icons/log-in';
	import Maximize from '@lucide/svelte/icons/maximize';
	import Minus from '@lucide/svelte/icons/minus';
	import Plus from '@lucide/svelte/icons/plus';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { api, assetUrl, type EntityKey, type SolarSystem } from '$lib/api';
	import { readTheme, type Theme } from '$lib/components/tree/render';
	import { Camera } from '$lib/components/tree/camera';
	import * as Breadcrumb from '$lib/components/ui/breadcrumb/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { settings } from '$lib/settings.svelte';
	import { app } from '$lib/state.svelte';
	import { parseDate } from './kepler';
	import MapHoverCard from './MapHoverCard.svelte';
	import { mapState } from './map-state.svelte';
	import { clearTextCache, render } from './render';
	import {
		buildIndex,
		buildSystem,
		pathTo,
		systemOf,
		updatePositions,
		TYPE_LABELS,
		type MapItem,
		type SystemScene
	} from './scene';
	import TimeBar from './TimeBar.svelte';

	// --- data ---------------------------------------------------------------------

	let data = $state.raw<SolarSystem | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	const scopeKey = $derived(JSON.stringify(app.scope));
	let latest = 0;

	$effect(() => {
		const scope = JSON.parse(scopeKey);
		const id = ++latest;
		loading = true;
		error = null;
		api.getSolarSystem(scope).then(
			(d) => {
				if (id !== latest) return;
				const start = parseDate(d.startDate);
				if (start !== mapState.start) {
					mapState.start = start;
					mapState.time = start;
				}
				data = d;
				loading = false;
			},
			(e) => {
				if (id !== latest) return;
				error = String(e);
				loading = false;
			}
		);
	});

	const index = $derived(data ? buildIndex(data) : null);
	const centerKey = $derived(
		index
			? app.mapSystem && index.bodies.has(app.mapSystem)
				? app.mapSystem
				: index.root?.key
			: null
	);
	const scene = $derived.by((): SystemScene | null => {
		const center = centerKey ? index?.bodies.get(centerKey) : undefined;
		return center ? buildSystem(center, settings.mapScale) : null;
	});
	const crumbs = $derived(index && centerKey ? pathTo(index, centerKey) : []);

	/** Items failing the filters (the center always passes). */
	const filtered = $derived.by(() => {
		if (!scene || !mapState.filtersActive) return null;
		const center = scene.center;
		const fails = (it: MapItem) =>
			!(it.body === center && !it.point) &&
			!mapState.matches(it.type, it.point ? null : (it.body?.info ?? null));
		return new Set(scene.items.filter(fails).map((it) => it.key));
	});

	// Positions follow the date (and a rebuilt scene).
	$effect(() => {
		const s = scene;
		const t = mapState.time;
		if (!s || !Number.isFinite(t)) return;
		updatePositions(s, t);
		requestRender();
	});

	// --- body icons -----------------------------------------------------------------

	/** Loaded (or failed: null) icons by entity key; shared across systems. Not reactive on purpose. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const iconCache = new Map<EntityKey, ImageBitmap | null>();
	let icons = $state.raw<ReadonlyMap<EntityKey, ImageBitmap>>(new Map());

	$effect(() => {
		const s = scene;
		if (!s) return;
		untrack(() => {
			for (const it of s.items) {
				const ref = it.point ? null : it.body?.info.entity;
				const url = ref ? assetUrl(ref.icon) : null;
				if (!url || iconCache.has(it.key)) continue;
				iconCache.set(it.key, null);
				const img = new Image();
				img.onload = () =>
					createImageBitmap(img).then((bitmap) => {
						iconCache.set(it.key, bitmap);
						icons = new Map(
							[...iconCache].filter((e): e is [EntityKey, ImageBitmap] => e[1] !== null)
						);
					});
				img.src = url;
			}
		});
	});

	// --- rendering ------------------------------------------------------------------

	const camera = new Camera({ minZoom: 0.0005, maxZoom: 500 });
	let container = $state<HTMLDivElement>();
	let canvas = $state<HTMLCanvasElement>();
	let surface = $state<HTMLDivElement>();
	let width = $state(0);
	let height = $state(0);
	let dpr = $state(typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1);
	let theme = $state.raw<Theme | null>(null);
	let hovered = $state<EntityKey | null>(null);
	let fontVersion = $state(0);

	let frame = 0;
	function requestRender() {
		if (!frame) frame = requestAnimationFrame(draw);
	}

	function draw() {
		frame = 0;
		const ctx = canvas?.getContext('2d');
		if (!ctx || !canvas || !theme || width <= 0 || height <= 0) return;
		const w = Math.round(width * dpr);
		const h = Math.round(height * dpr);
		if (canvas.width !== w || canvas.height !== h) {
			canvas.width = w;
			canvas.height = h;
		}
		render(ctx, dpr, camera, scene, {
			selected: app.selected,
			hovered: dragging ? null : hovered,
			theme,
			filtered,
			hideFiltered: settings.mapFilterMode === 'hide',
			minorLabels: !!scene && camera.zoom >= fitZoom(scene) * 2.5,
			icons
		});
		updateHoverAnchor();
	}

	const unsubscribe = camera.subscribe(requestRender);
	onDestroy(() => {
		unsubscribe();
		camera.stop();
		cancelAnimationFrame(frame);
	});

	$effect(() => {
		camera.resize(width, height);
	});

	$effect(() => {
		void [scene, app.selected, hovered, theme, dpr, icons, filtered, settings.mapFilterMode];
		void fontVersion;
		requestRender();
	});

	// Theme colors follow the light/dark mode (see TreeCanvas).
	$effect(() => {
		const el = container;
		if (!el) return;
		theme = readTheme(el);
		const observer = new MutationObserver(() => (theme = readTheme(el)));
		observer.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['class', 'style']
		});
		return () => observer.disconnect();
	});

	onMount(() => {
		document.fonts?.ready.then(() => {
			clearTextCache();
			fontVersion++;
		});
	});

	$effect(() => {
		const query = matchMedia(`(resolution: ${dpr}dppx)`);
		const update = () => (dpr = window.devicePixelRatio || 1);
		query.addEventListener('change', update, { once: true });
		return () => query.removeEventListener('change', update);
	});

	// Don't keep the clock running on the tree page.
	$effect(() => {
		if (app.page !== 'map') untrack(() => (mapState.playing = false));
	});

	// --- fitting and focusing ---------------------------------------------------------

	/** Zoom showing the system's extent clear of the overlays (time bar below, breadcrumb above). */
	const fitZoom = (s: SystemScene) => (Math.min(width, height - 140) / 2 / s.extent) * 0.95;

	function fitAll(duration = 300) {
		if (scene) camera.animateTo(camera.centered(0, 0, fitZoom(scene)), duration);
	}

	function centerOn(it: MapItem, zoom = camera.zoom, duration = 400) {
		camera.animateTo(camera.centered(it.wx, it.wy, zoom), duration);
	}

	/** Scene the camera was last fitted to. */
	let fitted: SystemScene | null = null;

	// Show the focused entity's system, then center on it once that scene exists.
	$effect(() => {
		const focus = app.mapFocus;
		const idx = index;
		if (!focus || !idx) return;
		const sys = systemOf(idx, focus);
		untrack(() => {
			if (!sys) {
				app.mapFocus = null;
				return;
			}
			const next = sys === idx.root?.key ? null : sys;
			if (app.mapSystem !== next) app.mapSystem = next;
		});
	});

	$effect(() => {
		const s = scene;
		const focus = app.mapFocus;
		if (!s || width <= 0 || height <= 0 || !Number.isFinite(mapState.time)) return;
		untrack(() => {
			const target = focus ? s.byKey.get(focus) : undefined;
			if (focus && !target) return; // its system is still being opened
			if (s !== fitted) {
				fitted = s;
				updatePositions(s, mapState.time);
				if (!target) fitAll(0);
				else camera.set(camera.centered(0, 0, fitZoom(s)));
			}
			if (!target) return;
			if (filtered?.has(target.key)) mapState.clearFilters();
			centerOn(target, Math.max(camera.zoom, fitZoom(s)));
			app.mapFocus = null;
		});
	});

	function enterSystem(key: EntityKey) {
		const body = index?.bodies.get(key);
		if (!body?.children.length) return;
		app.mapSystem = body === index?.root ? null : key;
	}

	function goUp() {
		const parent = scene?.center.parent;
		if (parent) app.mapSystem = parent === index?.root ? null : parent.key;
	}

	// --- pointer interaction ----------------------------------------------------------

	let pointer: { id: number; x: number; y: number; moved: boolean } | null = null;
	let lastPointer: { clientX: number; clientY: number } | null = null;
	let dragging = $state(false);

	function local(e: { clientX: number; clientY: number }) {
		const r = canvas!.getBoundingClientRect();
		return { x: e.clientX - r.left, y: e.clientY - r.top };
	}

	/** Nearest item under the pointer (screen space, generous radius for small dots). */
	function hit(e: { clientX: number; clientY: number }): MapItem | null {
		if (!scene) return null;
		const p = local(e);
		let best: MapItem | null = null;
		let bestDist = Infinity;
		for (const it of scene.items) {
			if (!it.onScreen) continue;
			const d = Math.hypot(it.sx - p.x, it.sy - p.y);
			if (d <= Math.max(it.size + 4, 8) && d < bestDist) {
				best = it;
				bestDist = d;
			}
		}
		return best;
	}

	function onpointerdown(e: PointerEvent) {
		if (e.button !== 0 && e.button !== 1) return;
		surface?.focus({ preventScroll: true });
		camera.stop();
		pointer = { id: e.pointerId, x: e.clientX, y: e.clientY, moved: false };
		surface?.setPointerCapture(e.pointerId);
	}

	function onpointermove(e: PointerEvent) {
		if (pointer && pointer.id === e.pointerId) {
			const dx = e.clientX - pointer.x;
			const dy = e.clientY - pointer.y;
			if (!pointer.moved && Math.hypot(dx, dy) < 4) return;
			pointer.moved = true;
			dragging = true;
			pointer.x = e.clientX;
			pointer.y = e.clientY;
			camera.panBy(dx, dy);
			return;
		}
		lastPointer = { clientX: e.clientX, clientY: e.clientY };
		hovered = hit(e)?.key ?? null;
	}

	// Bodies move under a resting pointer (zoom, playback): hover what is there now.
	function rehover() {
		if (!lastPointer || dragging) return;
		const key = hit(lastPointer)?.key ?? null;
		if (key !== hovered) hovered = key;
	}
	$effect(() => camera.subscribe(() => requestAnimationFrame(rehover)));
	$effect(() => {
		void mapState.time;
		requestAnimationFrame(rehover);
	});

	function onpointerup(e: PointerEvent) {
		if (!pointer || pointer.id !== e.pointerId) return;
		const clicked = !pointer.moved && e.button === 0;
		pointer = null;
		dragging = false;
		if (!clicked) return;
		const it = hit(e);
		if (it) select(it.key);
		else app.selected = null;
	}

	function ondblclick(e: MouseEvent) {
		const it = hit(e);
		if (!it || it.point) return;
		if (it.body === scene?.center) goUp();
		else enterSystem(it.key);
	}

	function onwheel(e: WheelEvent) {
		e.preventDefault();
		const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 100 : 1;
		const speed = e.ctrlKey ? 0.01 : 0.0015;
		const factor = Math.min(2, Math.max(0.5, Math.exp(-e.deltaY * unit * speed)));
		const p = local(e);
		camera.zoomAt(p.x, p.y, factor);
	}

	function select(key: EntityKey) {
		app.selected = key;
		app.panelOpen = true;
	}

	function zoomBy(factor: number) {
		camera.zoomAt(width / 2, height / 2, factor);
	}

	/** Keyboard selection: step through the system, most important and innermost first. */
	function cycle(dir: 1 | -1) {
		if (!scene) return;
		const order = [...scene.items].sort(
			(a, b) => a.priority - b.priority || Math.hypot(a.wx, a.wy) - Math.hypot(b.wx, b.wy)
		);
		const at = order.findIndex((it) => it.key === app.selected);
		const it =
			order[at < 0 ? (dir > 0 ? 0 : order.length - 1) : (at + dir + order.length) % order.length];
		select(it.key);
		if (!it.onScreen) centerOn(it);
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowRight' || e.key === 'ArrowDown') cycle(1);
		else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') cycle(-1);
		else if (e.key === 'Escape') app.selected = null;
		else if (e.key === 'Backspace') goUp();
		else if (e.key === 'Enter' && app.selected) enterSystem(app.selected);
		else if (e.key === '+' || e.key === '=') zoomBy(1.25);
		else if (e.key === '-') zoomBy(0.8);
		else if (e.key === '0') fitAll();
		else if (e.key === ' ') mapState.playing = !mapState.playing;
		else return;
		e.preventDefault();
	}

	// --- overlays ---------------------------------------------------------------------

	let hoverAnchor = $state<{ x: number; y: number } | null>(null);
	const hoveredItem = $derived(hovered && !dragging ? (scene?.byKey.get(hovered) ?? null) : null);

	function updateHoverAnchor() {
		const it = hoveredItem;
		if (!it || !it.onScreen) {
			if (hoverAnchor) hoverAnchor = null;
			return;
		}
		const x = it.sx + it.size;
		const y = it.sy;
		if (hoverAnchor?.x !== x || hoverAnchor?.y !== y) hoverAnchor = { x, y };
	}

	/** Selected body with satellites, other than the center: offer to enter its system. */
	const enterable = $derived.by(() => {
		const it = app.selected ? scene?.byKey.get(app.selected) : undefined;
		return it && !it.point && it.body !== scene?.center && it.body?.children.length ? it : null;
	});

	const announcement = $derived.by(() => {
		const it = app.selected ? scene?.byKey.get(app.selected) : undefined;
		return it ? `${it.name}, ${TYPE_LABELS[it.type]}` : '';
	});
</script>

<div
	bind:this={container}
	class="relative h-full min-h-0 overflow-hidden bg-background"
	bind:clientWidth={width}
	bind:clientHeight={height}
>
	<!-- See TreeCanvas: `application` surface with its own keyboard model; the canvas only paints. -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
	<div
		bind:this={surface}
		class="absolute inset-0 touch-none outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
		class:cursor-pointer={hovered && !dragging}
		class:cursor-grabbing={dragging}
		tabindex="0"
		role="application"
		aria-roledescription="solar system map"
		aria-label="Solar system map"
		aria-describedby="solar-map-keyboard-help"
		{onpointerdown}
		{onpointermove}
		{onpointerup}
		onpointercancel={onpointerup}
		onpointerleave={() => {
			lastPointer = null;
			hovered = null;
		}}
		{ondblclick}
		{onwheel}
		{onkeydown}
	>
		<canvas bind:this={canvas} class="block size-full" aria-hidden="true"></canvas>
	</div>
	<p id="solar-map-keyboard-help" class="sr-only">
		Click a body to show its details, double-click it to enter its system of moons. Arrow keys step
		through the bodies and Lagrange points of the system. Enter opens the selected body's system,
		Backspace goes up one level. Space plays or pauses time. Plus and minus zoom, 0 fits the system.
	</p>
	<p class="sr-only" aria-live="polite">{announcement}</p>

	{#if crumbs.length}
		<div class="absolute top-3 left-3 rounded-lg border bg-card px-3 py-1.5 shadow">
			<Breadcrumb.Root>
				<Breadcrumb.List>
					{#each crumbs as body, i (body.key)}
						{#if i > 0}<Breadcrumb.Separator />{/if}
						<Breadcrumb.Item>
							{#if i === crumbs.length - 1}
								<Breadcrumb.Page>{body.info.entity.name}</Breadcrumb.Page>
							{:else}
								<Breadcrumb.Link
									href="#{body.info.entity.dataName}"
									onclick={(e: MouseEvent) => {
										e.preventDefault();
										app.mapSystem = i === 0 ? null : body.key;
									}}>{body.info.entity.name}</Breadcrumb.Link
								>
							{/if}
						</Breadcrumb.Item>
					{/each}
				</Breadcrumb.List>
			</Breadcrumb.Root>
		</div>
	{/if}

	{#if enterable}
		<div class="absolute top-3 left-1/2 -translate-x-1/2">
			<Button
				variant="outline"
				size="sm"
				class="rounded-full shadow"
				onclick={() => enterSystem(enterable.key)}
			>
				<LogIn /> Enter {enterable.name} system
			</Button>
		</div>
	{/if}

	{#if hoveredItem && hoverAnchor}
		<MapHoverCard item={hoveredItem} x={hoverAnchor.x} y={hoverAnchor.y} {width} {height} />
	{/if}

	<div class="absolute bottom-3 left-3 flex flex-col gap-1" role="group" aria-label="Zoom">
		<Button variant="outline" size="icon" aria-label="Zoom in" onclick={() => zoomBy(1.25)}>
			<Plus />
		</Button>
		<Button variant="outline" size="icon" aria-label="Zoom out" onclick={() => zoomBy(0.8)}>
			<Minus />
		</Button>
		<Button variant="outline" size="icon" aria-label="Fit system" onclick={() => fitAll()}>
			<Maximize />
		</Button>
	</div>

	{#if data}
		<div class="absolute bottom-3 left-1/2 max-w-[calc(100%-6rem)] -translate-x-1/2">
			<TimeBar />
		</div>
	{/if}

	{#if loading}
		<div
			class="pointer-events-none absolute top-3 left-1/2 flex -translate-x-1/2 items-center gap-2 rounded-full border bg-card px-3 py-1 text-sm text-muted-foreground shadow"
			role="status"
		>
			<LoaderCircle class="size-4 animate-spin" /> Loading solar system…
		</div>
	{/if}
	{#if error}
		<div class="absolute inset-0 grid place-items-center p-6">
			<div
				class="flex max-w-lg items-start gap-3 rounded-xl border border-destructive/50 bg-card p-4 shadow"
				role="alert"
			>
				<TriangleAlert class="mt-0.5 size-5 shrink-0 text-destructive" />
				<div class="text-sm">
					<p class="font-medium">Could not load the solar system</p>
					<p class="mt-1 break-words text-muted-foreground">{error}</p>
				</div>
			</div>
		</div>
	{/if}
</div>
