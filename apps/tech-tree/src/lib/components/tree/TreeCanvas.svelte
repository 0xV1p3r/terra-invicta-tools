<script lang="ts">
	// The research tree view: one <canvas> drawn by `render.ts` from a camera, plus DOM
	// overlays (controls, minimap, hover card, status). Performance-first replacement for
	// xyflow, whose DOM cards made WebKitGTK (Tauri on Linux) stutter on pan and freeze on zoom.
	import { onDestroy, onMount, tick, untrack } from 'svelte';
	import Keyboard from '@lucide/svelte/icons/keyboard';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import Maximize from '@lucide/svelte/icons/maximize';
	import ScanSearch from '@lucide/svelte/icons/scan-search';
	import Minus from '@lucide/svelte/icons/minus';
	import Plus from '@lucide/svelte/icons/plus';
	import RotateCw from '@lucide/svelte/icons/rotate-cw';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import {
		api,
		matchesFilter,
		type EntityKey,
		type FullLayout,
		type TechCategory,
		type TreeFilter,
		type TreeGraph
	} from '#lib/api.ts';
	import { campaign } from '#lib/campaign.svelte.ts';
	import { CATEGORIES, CATEGORY_ORDER } from '#lib/categories.ts';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import { settings } from '#lib/settings.svelte.ts';
	import { app } from '#lib/state.svelte.ts';
	import { Camera } from './camera';
	import { FLAGS } from './flags';
	import HoverCard from './HoverCard.svelte';
	import MiniMap from './MiniMap.svelte';
	import { CHECK_GLYPH, GOAL_GLYPH, planMarks, statusLabels } from './plan-marks';
	import PlanMenu, { type MenuTarget } from './PlanMenu.svelte';
	import { DETAIL_ZOOM, readTheme, render, type Theme } from './render';
	import { buildScene, nodeAt, type SceneNode } from './scene';
	import ViewStats from './ViewStats.svelte';

	/** The graph on screen once it matches the current view, for the toolbar (bindable). */
	let { graph: shown = $bindable(null) }: { graph?: TreeGraph | null } = $props();

	const NO_FILTER: TreeFilter = { techs: true, projects: true, categories: [], properties: [] };

	/** Everything the current view depends on; a loaded graph is only "current" for its key. */
	const request = $derived(
		app.view.kind === 'subtree'
			? {
					kind: 'subtree' as const,
					scope: $state.snapshot(app.scope),
					args: {
						root: app.view.root,
						direction: app.view.direction,
						alternatives: app.view.alternatives,
						maxDepth: null
					},
					mergeEdges: settings.mergeLinks
				}
			: app.view.kind === 'focus'
				? { kind: 'focus' as const, scope: $state.snapshot(app.scope), root: app.view.root }
				: {
						kind: 'full' as const,
						layout: app.view.layout,
						mergeEdges: settings.mergeLinks,
						scope: $state.snapshot(app.scope),
						// Highlight mode loads everything and dims non-matches (layout stays put).
						filter: settings.filterMode === 'highlight' ? NO_FILTER : app.filter
					}
	);
	const requestKey = $derived(JSON.stringify(request));

	/** The active campaign's statuses, steps and goals for the renderer (null without one). */
	const marks = $derived(
		campaign.active && campaign.plan ? planMarks(campaign.plan, campaign.active.goals) : null
	);
	/** The "Completed" filter is off: completed research is hidden or dimmed like non-matches. */
	const completedOut = $derived(marks !== null && !settings.showCompleted && app.mode === 'full');

	/** Highlight mode with active filters: keys of the matching nodes (others are dimmed). */
	const matches = $derived.by(() => {
		if (settings.filterMode !== 'highlight' || app.mode !== 'full') return null;
		if (!app.filtersActive && !completedOut) return null;
		const filter = app.filter;
		const done = completedOut ? marks!.done : null;
		return new Set(
			(graph?.nodes ?? [])
				.filter((n) => matchesFilter(n, filter) && !done?.has(n.key))
				.map((n) => n.key)
		);
	});

	/**
	 * Hide mode with "Completed" off: completed research, left out of the scene client-side
	 * (the backend filter knows no campaigns), so the layout stays put. A string so the scene
	 * is only rebuilt when the set changes, not on every campaign save.
	 */
	const hiddenKey = $derived(
		completedOut && settings.filterMode === 'hide' ? [...marks!.done].sort().join('\n') : ''
	);

	let loaded = $state.raw<{ key: string; graph: TreeGraph } | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	/** Bumped by "Retry" to run the request again. */
	let attempt = $state(0);

	const graph = $derived(loaded?.graph ?? null);
	const ready = $derived(loaded?.key === requestKey);
	$effect(() => {
		shown = ready ? graph : null;
	});
	const scene = $derived.by(() => {
		if (!graph) return null;
		const options = { hideRedundant: settings.hideRedundantLinks };
		if (!hiddenKey) return buildScene(graph, options);
		const hidden = new Set(hiddenKey.split('\n'));
		return buildScene(
			{
				...graph,
				nodes: graph.nodes.filter((n) => !hidden.has(n.key)),
				edges: graph.edges.filter((e) => !hidden.has(e.from) && !hidden.has(e.to))
			},
			options
		);
	});

	// The full tree is large; keep the last one per layout to make going back instant.
	const fullCache: Partial<Record<FullLayout, { key: string; graph: TreeGraph }>> = {};
	let latest = 0;

	function show(key: string, g: TreeGraph) {
		app.treeKeys = new Set(g.nodes.map((n) => n.key));
		loaded = { key, graph: g };
	}

	$effect(() => {
		const key = requestKey;
		const req = request;
		void attempt;
		const cached = req.kind === 'full' ? fullCache[req.layout] : undefined;
		if (cached?.key === key) {
			latest++;
			loading = false;
			error = null;
			show(key, cached.graph);
			return;
		}
		const id = ++latest;
		loading = true;
		error = null;
		const promise =
			req.kind === 'full'
				? api.getTree(req.scope, req.filter, req.layout, req.mergeEdges)
				: req.kind === 'focus'
					? api.getFocus(req.scope, req.root)
					: api.getSubtree(req.scope, req.args, req.mergeEdges);
		promise.then(
			(g) => {
				if (id !== latest) return;
				if (req.kind === 'full') fullCache[req.layout] = { key, graph: g };
				show(key, g);
				loading = false;
			},
			(e) => {
				if (id !== latest) return;
				error = String(e);
				loading = false;
			}
		);
	});

	// --- rendering ----------------------------------------------------------------

	const camera = new Camera();
	let container = $state<HTMLDivElement>();
	let canvas = $state<HTMLCanvasElement>();
	let surface = $state<HTMLDivElement>();
	let width = $state(0);
	let height = $state(0);
	let dpr = $state(typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1);
	let theme = $state.raw<Theme | null>(null);
	let hovered = $state<EntityKey | null>(null);
	/** Bumped when category icons or web fonts finish loading. */
	let assetVersion = $state(0);

	const icons: Partial<Record<TechCategory, HTMLImageElement>> = {};
	for (const category of CATEGORY_ORDER) {
		const img = new Image();
		img.onload = () => assetVersion++;
		img.src = CATEGORIES[category].icon;
		icons[category] = img;
	}
	const glyphs = new Map([
		...FLAGS.map((f) => [f.key as string, new Path2D(f.glyph)] as const),
		['plan:check', new Path2D(CHECK_GLYPH)],
		['plan:goal', new Path2D(GOAL_GLYPH)]
	]);

	let frame = 0;
	function requestRender() {
		if (!frame) frame = requestAnimationFrame(draw);
	}

	function draw() {
		frame = 0;
		const ctx = canvas?.getContext('2d');
		if (!ctx || !canvas || !theme) return;
		const w = Math.round(width * dpr);
		const h = Math.round(height * dpr);
		if (canvas.width !== w || canvas.height !== h) {
			canvas.width = w;
			canvas.height = h;
		}
		render(ctx, dpr, camera, scene, {
			selected: app.selected,
			// The card a menu is open for stays outlined.
			hovered: menu?.key ?? (dragging ? null : hovered),
			root: app.root ? (graph?.root ?? app.root) : null,
			theme,
			icons,
			glyphs,
			plan: marks,
			matches,
			overview: settings.overviewLabels,
			landmarks: settings.landmarkLabels
		});
		updateBackground();
		updateHoverAnchor();
		const out = camera.zoom < DETAIL_ZOOM;
		if (zoomedOut !== out) zoomedOut = out;
	}

	/** The selection or filter highlighting dims the tree; the dot grid recedes with it. */
	const quiet = $derived(app.selected !== null || matches !== null);

	// Dot grid as a CSS background that follows the camera (cheaper than drawing dots).
	function updateBackground() {
		if (!container) return;
		let gap = 20 * camera.zoom;
		while (gap < 14) gap *= 2;
		container.style.backgroundSize = `${gap}px ${gap}px`;
		container.style.backgroundPosition = `${camera.x}px ${camera.y}px`;
	}

	const unsubscribe = camera.subscribe(requestRender);
	onDestroy(() => {
		// A response arriving after the tree is gone (data folder or build switch) is dropped.
		latest++;
		unsubscribe();
		camera.stop();
		cancelAnimationFrame(frame);
	});

	$effect(() => {
		camera.resize(width, height);
	});

	// --- overlays ---------------------------------------------------------------------

	/**
	 * Screen rectangles of the floating overlays (`data-overlay` elements), which camera fits
	 * and landmark labels keep clear of.
	 * Measured when they resize or move, not per frame.
	 */
	function measureOverlays() {
		if (!container) return;
		const base = container.getBoundingClientRect();
		const rects: { x: number; y: number; width: number; height: number }[] = [];
		for (const el of container.querySelectorAll(':scope > [data-overlay]')) {
			const r = el.getBoundingClientRect();
			if (!r.width || !r.height) continue;
			rects.push({ x: r.left - base.left, y: r.top - base.top, width: r.width, height: r.height });
			observeOverlay(el);
		}
		camera.overlays = rects;
		requestRender();
	}

	let overlayFrame = 0;
	const observed = new WeakSet<Element>();
	const overlayObserver =
		typeof ResizeObserver === 'undefined'
			? null
			: new ResizeObserver(() => {
					if (!overlayFrame)
						overlayFrame = requestAnimationFrame(() => {
							overlayFrame = 0;
							measureOverlays();
						});
				});
	function observeOverlay(el: Element) {
		if (observed.has(el)) return;
		observed.add(el);
		overlayObserver?.observe(el);
	}
	onDestroy(() => {
		overlayObserver?.disconnect();
		cancelAnimationFrame(overlayFrame);
	});

	// Overlays move with the minimap corner, the view and the window size.
	$effect(() => {
		void [settings.minimapCorner, app.view, scene, width, height];
		tick().then(measureOverlays);
	});

	// Redraw on anything the picture depends on (camera changes subscribe directly).
	$effect(() => {
		void [
			scene,
			app.selected,
			app.view,
			hovered,
			theme,
			dpr,
			assetVersion,
			matches,
			marks,
			menu,
			settings.overviewLabels,
			settings.landmarkLabels
		];
		requestRender();
	});

	$effect(() => {
		if (container) theme = readTheme(container);
	});

	onMount(() => {
		document.fonts?.ready.then(() => {
			// Text was measured with fallback metrics until now.
			for (const n of scene?.nodes ?? []) n.labels.clear();
			assetVersion++;
		});
	});

	// Moving the window to a display with another scale keeps the CSS size: watch the DPR.
	$effect(() => {
		const query = matchMedia(`(resolution: ${dpr}dppx)`);
		const update = () => (dpr = window.devicePixelRatio || 1);
		query.addEventListener('change', update, { once: true });
		return () => query.removeEventListener('change', update);
	});

	// --- fitting and focusing ------------------------------------------------------

	/**
	 * Loaded graph the camera was last fitted to (scenes are rebuilt for display settings and
	 * hidden completed research too).
	 */
	let fitted: TreeGraph | null = null;

	$effect(() => {
		const s = scene;
		const g = graph;
		const focus = app.focus;
		if (!s || !g || !ready || width <= 0 || height <= 0) return;
		untrack(() => {
			measureOverlays();
			const target = focus ? s.byKey.get(focus) : undefined;
			if (g !== fitted && (!target || focus === s.graph.root)) {
				const first = fitted === null;
				fitted = g;
				if (s.nodes.length) camera.animateTo(camera.fitted(s.bounds), first ? 0 : 300);
				// A freshly opened subtree is shown whole rather than centered on its root.
				if (target && focus === s.graph.root) {
					app.focus = null;
					return;
				}
			}
			if (!focus) return;
			if (target) {
				fitted = g;
				centerOn(target, Math.max(camera.zoom, 0.9));
				app.focus = null;
			} else if (hiddenKey && hiddenKey.split('\n').includes(focus)) {
				// Hidden as completed: show completed research, focus once the scene has it.
				settings.showCompleted = true;
			} else if (app.mode === 'full' && app.filtersActive) {
				// Hidden by the filters: show everything, focus once the full tree is loaded.
				app.clearFilters();
			} else {
				app.focus = null;
			}
		});
	});

	function centerOn(n: SceneNode, zoom = camera.zoom, duration = 400) {
		if (!scene) return;
		camera.animateTo(
			camera.centered(n.x + scene.width / 2, n.y + scene.height / 2, zoom),
			duration
		);
	}

	function fitAll() {
		measureOverlays();
		if (scene?.nodes.length) camera.animateTo(camera.fitted(scene.bounds));
	}

	/** Fit the camera to the highlighted filter matches. */
	function fitMatches() {
		if (!scene || !matches?.size) return;
		let [x1, y1, x2, y2] = [Infinity, Infinity, -Infinity, -Infinity];
		for (const key of matches) {
			const n = scene.byKey.get(key);
			if (!n) continue;
			[x1, y1] = [Math.min(x1, n.x), Math.min(y1, n.y)];
			[x2, y2] = [Math.max(x2, n.x + scene.width), Math.max(y2, n.y + scene.height)];
		}
		measureOverlays();
		camera.animateTo(camera.fitted({ x: x1, y: y1, width: x2 - x1, height: y2 - y1 }));
	}

	function zoomBy(factor: number) {
		camera.zoomAt(width / 2, height / 2, factor);
	}

	// --- pointer interaction ----------------------------------------------------------

	let pointer: { id: number; x: number; y: number; moved: boolean } | null = null;
	/** Last pointer position over the tree (client coordinates), for hover after camera moves. */
	let lastPointer: { clientX: number; clientY: number } | null = null;
	let dragging = $state(false);

	function local(e: { clientX: number; clientY: number }) {
		const r = canvas!.getBoundingClientRect();
		return { x: e.clientX - r.left, y: e.clientY - r.top };
	}

	function hit(e: { clientX: number; clientY: number }) {
		if (!scene) return null;
		const p = local(e);
		const w = camera.toWorld(p.x, p.y);
		return nodeAt(scene, w.x, w.y);
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

	// Zooming or animating moves cards under a resting pointer: hover what is there now.
	$effect(() =>
		camera.subscribe(() => {
			if (!lastPointer || dragging) return;
			const key = hit(lastPointer)?.key ?? null;
			if (key !== hovered) hovered = key;
		})
	);

	function onpointerup(e: PointerEvent) {
		if (!pointer || pointer.id !== e.pointerId) return;
		const clicked = !pointer.moved && e.button === 0;
		pointer = null;
		dragging = false;
		if (!clicked) return;
		const n = hit(e);
		if (n && app.view.kind === 'focus' && n.key !== app.view.root) app.openFocus(n.key);
		else if (n) select(n.key);
		else app.selected = null;
	}

	function ondblclick(e: MouseEvent) {
		const n = hit(e);
		if (n && app.view.kind !== 'focus') app.openSubtree(n.key, 'up');
	}

	function onwheel(e: WheelEvent) {
		e.preventDefault();
		const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 100 : 1;
		// Pinch gestures arrive as ctrl+wheel with small deltas.
		const speed = e.ctrlKey ? 0.01 : 0.0015;
		const factor = Math.min(2, Math.max(0.5, Math.exp(-e.deltaY * unit * speed)));
		const p = local(e);
		camera.zoomAt(p.x, p.y, factor);
	}

	function select(key: EntityKey) {
		app.selected = key;
		app.panelOpen = true;
	}

	// --- keyboard ---------------------------------------------------------------------

	/** Nearest node from `from` in an arrow direction (prefers the same row/column). */
	function neighbour(from: SceneNode, dx: number, dy: number): SceneNode | null {
		if (!scene) return null;
		let best: SceneNode | null = null;
		let bestScore = Infinity;
		for (const n of scene.nodes) {
			const ax = n.x - from.x;
			const ay = n.y - from.y;
			const along = ax * dx + ay * dy;
			if (along <= 0) continue;
			const across = Math.abs(ax * dy) + Math.abs(ay * dx);
			const score = along + across * 3;
			if (score < bestScore) {
				bestScore = score;
				best = n;
			}
		}
		return best;
	}

	function nearestToCenter(): SceneNode | null {
		if (!scene) return null;
		const c = camera.toWorld(width / 2, height / 2);
		let best: SceneNode | null = null;
		let bestDist = Infinity;
		for (const n of scene.nodes) {
			const d = Math.hypot(n.x + scene.width / 2 - c.x, n.y + scene.height / 2 - c.y);
			if (d < bestDist) {
				bestDist = d;
				best = n;
			}
		}
		return best;
	}

	function ensureVisible(n: SceneNode) {
		if (!scene) return;
		const v = camera.visible();
		const margin = 40 / camera.zoom;
		const inside =
			n.x >= v.x + margin &&
			n.y >= v.y + margin &&
			n.x + scene.width <= v.x + v.width - margin &&
			n.y + scene.height <= v.y + v.height - margin;
		if (!inside) centerOn(n, Math.max(camera.zoom, 0.6), 250);
	}

	const ARROWS: Record<string, [number, number]> = {
		ArrowLeft: [-1, 0],
		ArrowRight: [1, 0],
		ArrowUp: [0, -1],
		ArrowDown: [0, 1]
	};

	function onkeydown(e: KeyboardEvent) {
		if (e.altKey) return; // Alt+arrows: history (window handler)
		const current = app.selected ? scene?.byKey.get(app.selected) : undefined;
		const arrow = ARROWS[e.key];
		if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) {
			e.preventDefault();
			keyboardMenu = performance.now();
			if (current) openMenuAt(current);
		} else if (arrow) {
			e.preventDefault();
			const next = current ? neighbour(current, ...arrow) : nearestToCenter();
			if (next) {
				app.selected = next.key;
				ensureVisible(next);
			}
		} else if ((e.key === 'Enter' || e.key === ' ') && current) {
			e.preventDefault();
			if (e.key === 'Enter' && app.view.kind === 'focus') app.openFocus(current.key);
			else if (e.key === 'Enter' && e.shiftKey) app.openSubtree(current.key, 'up');
			else select(current.key);
		} else if (e.key === 'Escape') {
			app.selected = null;
		} else if (e.key === '+' || e.key === '=') {
			zoomBy(1.25);
		} else if (e.key === '-') {
			zoomBy(0.8);
		} else if (e.key === '0') {
			fitAll();
		}
	}

	/** Alt+←/→ and the mouse back/forward buttons walk the history (on the map page too). */
	function onwindowkeydown(e: KeyboardEvent) {
		if (!e.altKey || (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight')) return;
		const t = e.target as HTMLElement | null;
		if (t?.closest('input, textarea, [contenteditable="true"]')) return;
		e.preventDefault();
		if (e.key === 'ArrowLeft') app.goBack();
		else app.goForward();
	}

	function onwindowmouseup(e: MouseEvent) {
		if (e.button === 3) app.goBack();
		else if (e.button === 4) app.goForward();
	}

	const SHORTCUTS: [keys: string, action: string][] = [
		['Arrow keys', 'Move the selection'],
		['Enter / Space', 'Show details'],
		['Shift+Enter', 'Requirements tree'],
		['Shift+F10', 'Plan menu'],
		['+ / −', 'Zoom in / out'],
		['0', 'Fit the tree'],
		['Esc', 'Clear the selection'],
		['Alt+← / →', 'Back / forward'],
		['Ctrl+K', 'Search'],
		['Ctrl+Shift+K', 'Wiki']
	];

	const announcement = $derived.by(() => {
		const n = app.selected ? scene?.byKey.get(app.selected) : undefined;
		if (!n) return '';
		const category = CATEGORIES[n.item.category]?.label ?? n.item.category;
		// The same flags and dimming the card shows, in words.
		const notes = n.flags.map((f) => f.label);
		if (matches && !matches.has(n.key)) notes.push('Outside the active filters');
		if (marks)
			notes.push(
				...statusLabels(
					marks.status[n.key],
					campaign.stepIndex.get(n.key),
					marks.goals.has(n.key),
					campaign.completions(n.key)
				)
			);
		return [`${n.item.name}, ${n.kind}, ${category}, cost ${n.cost}`, ...notes].join('. ');
	});

	// --- plan menu --------------------------------------------------------------------

	/** Card the plan menu is open for. */
	let menu = $state.raw<MenuTarget | null>(null);
	/** Time the menu key last opened the menu (its own contextmenu event is then ignored). */
	let keyboardMenu = -Infinity;

	function oncontextmenu(e: MouseEvent) {
		e.preventDefault();
		if (performance.now() - keyboardMenu < 500) return;
		const n = hit(e);
		if (!n) return;
		camera.stop();
		menu = { key: n.key, name: n.item.name, x: e.clientX, y: e.clientY };
	}

	/** Open the menu below a card's bottom-left corner (keyboard). */
	function openMenuAt(n: SceneNode) {
		if (!canvas || !scene) return;
		const r = canvas.getBoundingClientRect();
		const x = Math.min(Math.max(r.left + camera.x + n.x * camera.zoom, r.left), r.right);
		const y = Math.min(
			Math.max(r.top + camera.y + (n.y + scene.height) * camera.zoom, r.top),
			r.bottom
		);
		menu = { key: n.key, name: n.item.name, x, y };
	}

	const nameOf = (key: EntityKey) => graph?.nodes.find((n) => n.key === key)?.name ?? key;

	// --- hover card -------------------------------------------------------------------

	let hoverAnchor = $state<{ x: number; y: number } | null>(null);
	/** Zoomed out, cards show little themselves; zoomed in, the hover card expands with more. */
	let zoomedOut = $state(true);
	const hoveredNode = $derived(hovered && !dragging ? (scene?.byKey.get(hovered) ?? null) : null);

	function updateHoverAnchor() {
		const n = hoveredNode;
		if (!n || !scene) {
			if (hoverAnchor) hoverAnchor = null;
			return;
		}
		const x = camera.x + (n.x + scene.width) * camera.zoom;
		const y = camera.y + n.y * camera.zoom;
		if (hoverAnchor?.x !== x || hoverAnchor?.y !== y) hoverAnchor = { x, y };
	}
</script>

<svelte:window onkeydown={onwindowkeydown} onmouseup={onwindowmouseup} />

<div class="relative h-full min-h-0">
	<div
		bind:this={container}
		class="tree-surface relative size-full overflow-hidden bg-background"
		class:quiet
		bind:clientWidth={width}
		bind:clientHeight={height}
	>
		<!--
			Interactive surface (focus, keyboard, pointer); the canvas only paints. `application`
			is the ARIA role for a widget with its own keyboard model, which Svelte's checks don't
			treat as interactive.
		-->
		<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
		<div
			bind:this={surface}
			class="absolute inset-0 touch-none outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
			class:cursor-pointer={hovered && !dragging}
			class:cursor-grabbing={dragging}
			tabindex="0"
			role="application"
			aria-roledescription="research tree"
			aria-label="Research tree"
			aria-describedby="research-tree-keyboard-help"
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
			{oncontextmenu}
		>
			<canvas bind:this={canvas} class="block size-full" aria-hidden="true"></canvas>
		</div>
		<p id="research-tree-keyboard-help" class="sr-only">
			Arrow keys move the selection. Enter or Space shows details, Shift+Enter opens the
			requirements subtree (in the focus view, Enter makes the selection the focus). Shift+F10 or
			the menu key opens the plan menu of the selection. Alt+Left and Alt+Right go back and forward
			between views. Plus and minus zoom, 0 fits the tree.
		</p>
		<p class="sr-only" aria-live="polite">{announcement}</p>

		<PlanMenu bind:target={menu} {nameOf} onclose={() => surface?.focus({ preventScroll: true })} />

		{#if hoveredNode && hoverAnchor && !menu}
			<HoverCard
				node={hoveredNode}
				x={hoverAnchor.x}
				y={hoverAnchor.y}
				{width}
				{height}
				expanded={!zoomedOut}
			/>
		{/if}

		<!-- Zoom controls yield the bottom-left corner to the minimap. -->
		<div
			data-overlay
			class={[
				'absolute bottom-3 flex flex-col gap-1',
				settings.minimapCorner === 'bottom-left' ? 'right-3' : 'left-3'
			]}
			role="group"
			aria-label="Zoom"
		>
			<Button variant="outline" size="icon" aria-label="Zoom in" onclick={() => zoomBy(1.25)}>
				<Plus />
			</Button>
			<Button variant="outline" size="icon" aria-label="Zoom out" onclick={() => zoomBy(0.8)}>
				<Minus />
			</Button>
			<Button variant="outline" size="icon" aria-label="Fit tree" onclick={fitAll}>
				<Maximize />
			</Button>
			<Popover.Root>
				<Popover.Trigger>
					{#snippet child({ props })}
						<Button {...props} variant="outline" size="icon" aria-label="Keyboard shortcuts">
							<Keyboard />
						</Button>
					{/snippet}
				</Popover.Trigger>
				<Popover.Content
					class="w-72"
					side={settings.minimapCorner === 'bottom-left' ? 'left' : 'right'}
					align="end"
				>
					<Popover.Header>
						<Popover.Title>Keyboard shortcuts</Popover.Title>
					</Popover.Header>
					<dl class="mt-3 grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-1.5 text-sm">
						{#each SHORTCUTS as [keys, action] (keys)}
							<dt>
								<kbd
									class="rounded border bg-muted px-1.5 py-0.5 font-sans text-xs whitespace-nowrap text-muted-foreground"
									>{keys}</kbd
								>
							</dt>
							<dd>{action}</dd>
						{/each}
					</dl>
					<p class="mt-3 text-xs text-muted-foreground">
						Click the tree first so it has keyboard focus.
					</p>
				</Popover.Content>
			</Popover.Root>
		</div>

		<div
			class="pointer-events-none absolute top-3 left-1/2 flex -translate-x-1/2 flex-col items-center gap-2"
		>
			{#if ready && graph && app.view.kind !== 'full'}
				<ViewStats {graph} />
			{/if}
			{#if matches}
				<div
					class="pointer-events-auto flex items-center gap-2 rounded-full border bg-card py-1 pr-1 pl-3 text-sm shadow"
				>
					<span class="text-muted-foreground">{matches.size} matching</span>
					{#if matches.size}
						<Button variant="ghost" size="sm" class="h-7 rounded-full" onclick={fitMatches}>
							<ScanSearch /> Zoom to matches
						</Button>
					{/if}
				</div>
			{/if}
		</div>

		{#if scene?.nodes.length}
			{#if theme}
				<MiniMap {scene} {camera} {theme} {matches} done={marks?.done ?? null} />
			{/if}
		{/if}

		{#if loading}
			<div
				class="pointer-events-none absolute top-3 left-1/2 flex -translate-x-1/2 items-center gap-2 rounded-full border bg-card px-3 py-1 text-sm text-muted-foreground shadow"
				role="status"
			>
				<LoaderCircle class="size-4 animate-spin" /> Loading tree…
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
						<p class="font-medium">Could not load the tree</p>
						<p class="mt-1 break-words text-muted-foreground">{error}</p>
						<Button variant="outline" size="sm" class="mt-3" onclick={() => attempt++}>
							<RotateCw /> Retry
						</Button>
					</div>
				</div>
			</div>
		{:else if ready && graph && graph.nodes.length === 0}
			<div
				class="pointer-events-none absolute inset-0 grid place-content-center justify-items-center gap-3 text-muted-foreground"
			>
				No research matches the current filters.
				{#if app.mode === 'full' && app.filtersActive}
					<Button
						variant="outline"
						size="sm"
						class="pointer-events-auto"
						onclick={() => app.clearFilters()}
					>
						Reset filters
					</Button>
				{/if}
			</div>
		{/if}
	</div>
</div>
