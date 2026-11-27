<script lang="ts">
	// The solar system map: one system (a body and its satellites) at a time, drawn on one
	// <canvas> by `render.ts`, with DOM overlays for the breadcrumb, date and zoom controls.
	import { onDestroy, onMount, tick, untrack } from 'svelte';
	import CircleQuestionMark from '@lucide/svelte/icons/circle-question-mark';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import LogIn from '@lucide/svelte/icons/log-in';
	import Maximize from '@lucide/svelte/icons/maximize';
	import Minus from '@lucide/svelte/icons/minus';
	import Plus from '@lucide/svelte/icons/plus';
	import ZoomIn from '@lucide/svelte/icons/zoom-in';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { api, assetUrl, type EntityKey } from '#lib/api.ts';
	import { readTheme, type Theme } from '#lib/components/tree/render.ts';
	import { Camera } from '#lib/components/tree/camera.ts';
	import * as Breadcrumb from '#lib/components/ui/breadcrumb/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import { settings } from '#lib/settings.svelte.ts';
	import { app } from '#lib/state.svelte.ts';
	import { parseDate } from './kepler';
	import MapHoverCard from './MapHoverCard.svelte';
	import { mapState } from './map-state.svelte';
	import { clearTextCache, crowdedInner, render } from './render';
	import {
		buildSystem,
		pathTo,
		systemOf,
		updatePositions,
		TYPE_LABELS,
		type MapBody,
		type MapItem,
		type SystemScene
	} from './scene';
	import { canvasControl } from '#lib/components/styles.ts';
	import MapHotbar from './MapHotbar.svelte';
	import TimeBar from './TimeBar.svelte';

	// --- data ---------------------------------------------------------------------

	const data = $derived(mapState.data);
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
				mapState.data = d;
				loading = false;
			},
			(e) => {
				if (id !== latest) return;
				error = String(e);
				loading = false;
			}
		);
	});
	// A response arriving after the map is gone (data folder or build switch) is dropped.
	onDestroy(() => void latest++);

	const index = $derived(mapState.index);
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

	/**
	 * Filter outcome in this system: `failing` items (the center always passes), and among them
	 * the `gateways`, bodies whose satellites (at any depth) have matches, kept to reach those.
	 */
	const filterResult = $derived.by(() => {
		if (!scene || !mapState.filtersActive) return null;
		const center = scene.center;
		const fails = (it: MapItem) =>
			!(it.body === center && !it.point) &&
			!mapState.matches(it.type, it.point ? null : (it.body?.info ?? null));
		const hasMatch = (body: MapBody): boolean =>
			body.children.some(
				(c) =>
					mapState.matches(c.info.bodyType, c.info) ||
					(c.points.length > 0 && mapState.matches('lagrange', null)) ||
					hasMatch(c)
			);
		const failing = scene.items.filter(fails);
		const gateways = new Set(
			failing.filter((it) => !it.point && it.body && hasMatch(it.body)).map((it) => it.key)
		);
		// Matches below each gateway, for its hover card.
		const below = (body: MapBody, acc = { bodies: 0, points: 0 }) => {
			for (const c of body.children) {
				if (mapState.matches(c.info.bodyType, c.info)) acc.bodies++;
				if (mapState.matches('lagrange', null)) acc.points += c.points.length;
				below(c, acc);
			}
			return acc;
		};
		return {
			filtered: new Set(failing.filter((it) => !gateways.has(it.key)).map((it) => it.key)),
			gateways,
			gatewayMatches: new Map(
				[...gateways].map((key) => [key, below(scene.byKey.get(key)!.body!)] as const)
			),
			matching: scene.items.length - failing.length - 1,
			total: scene.items.length - 1
		};
	});
	/** Items failing the filters, other than gateways; null = no filter. */
	const filtered = $derived(filterResult?.filtered ?? null);
	const gateways = $derived(filterResult?.gateways ?? null);
	const hideFiltered = $derived(settings.mapFilterMode === 'hide');

	// A selection the filters hide is dropped.
	$effect(() => {
		const sel = app.selected;
		if (sel && hideFiltered && filtered?.has(sel)) untrack(() => (app.selected = null));
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
	/** Linear scale with the star and planets drawn on top of each other. */
	let innerCrowded = $state(false);

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
			gateways,
			hideFiltered,
			reserved: obstacles.map((r) => [r.left, r.top, r.right, r.bottom] as const),
			minorLabels: !!scene && camera.zoom >= fitZoom(scene) * 2.5,
			icons
		});
		const crowded = settings.mapScale === 'linear' && !!scene && crowdedInner(scene);
		if (crowded !== innerCrowded) innerCrowded = crowded;
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
		void [
			scene,
			app.selected,
			hovered,
			theme,
			dpr,
			icons,
			filtered,
			gateways,
			hideFiltered,
			obstacles
		];
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

	/** Camera transitions apply at once when the user asks for reduced motion. */
	let reducedMotion = $state(false);
	$effect(() => {
		const query = matchMedia('(prefers-reduced-motion: reduce)');
		reducedMotion = query.matches;
		const update = () => (reducedMotion = query.matches);
		query.addEventListener('change', update);
		return () => query.removeEventListener('change', update);
	});
	const motion = (duration: number) => (reducedMotion ? 0 : duration);

	/** Zoom showing the system's extent clear of the overlays (time bar below, breadcrumb above). */
	const fitZoom = (s: SystemScene) => (Math.min(width, height - 140) / 2 / s.extent) * 0.95;

	function fitAll(duration = 300) {
		if (scene) camera.animateTo(camera.centered(0, 0, fitZoom(scene)), motion(duration));
	}

	/** Fits the orbits of the four innermost planets (Mercury to Mars). */
	function zoomToInner() {
		if (!scene) return;
		const reach = scene.items
			.flatMap((it) => {
				const orbit = it.type === 'planet' && !it.point ? it.body?.info.orbit : null;
				return orbit ? [orbit.semiMajorAxisKm * (1 + orbit.eccentricity)] : [];
			})
			.sort((a, b) => a - b)
			.slice(0, 4)
			.at(-1);
		if (!reach) return;
		const zoom = (Math.min(width, height - 140) / 2 / scene.scale(reach)) * 0.95;
		camera.animateTo(camera.centered(0, 0, zoom), motion(400));
	}

	function centerOn(it: MapItem, zoom = camera.zoom, duration = 400) {
		camera.animateTo(camera.centered(it.wx, it.wy, zoom), motion(duration));
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

	// Fit requests from outside the map (the focus selector).
	let fitRequest = mapState.fitRequest;
	$effect(() => {
		const n = mapState.fitRequest;
		if (n === fitRequest) return;
		fitRequest = n;
		untrack(() => fitAll());
	});

	function enterSystem(key: EntityKey) {
		const body = index?.bodies.get(key);
		if (!body?.children.length) return;
		app.mapSystem = body === index?.root ? null : key;
	}

	/** Runs an overlay control's action; if that removed the focused control, the map takes focus. */
	function keepFocus(action: () => void) {
		action();
		tick().then(() => {
			const el = document.activeElement;
			if (!el || el === document.body) surface?.focus({ preventScroll: true });
		});
	}

	function goUp() {
		const parent = scene?.center.parent;
		if (parent) app.mapSystem = parent === index?.root ? null : parent.key;
	}

	// --- pointer interaction ----------------------------------------------------------

	let pointer: { id: number; x: number; y: number; moved: boolean } | null = null;
	let lastPointer: { clientX: number; clientY: number } | null = null;
	let dragging = $state(false);
	/** Pointer over an item right now (the hover card lingers, the cursor doesn't). */
	let overItem = $state(false);

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
			if (d <= Math.max(it.size + 4, 12) && d < bestDist) {
				best = it;
				bestDist = d;
			}
		}
		if (best) return best;
		// Otherwise its label.
		return (
			scene.items.find((it) => {
				const b = it.onScreen ? it.labelBox : null;
				return b && p.x >= b[0] && p.x <= b[2] && p.y >= b[1] && p.y <= b[3];
			}) ?? null
		);
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
		setHovered(hit(e)?.key ?? null);
	}

	/** Pending un-hover: the hover card stays briefly so the pointer can move onto it. */
	let unhover: ReturnType<typeof setTimeout> | undefined;
	function setHovered(key: EntityKey | null) {
		overItem = !!key;
		if (key) {
			clearTimeout(unhover);
			unhover = undefined;
			hovered = key;
		} else if (hovered && !unhover) {
			unhover = setTimeout(() => {
				unhover = undefined;
				hovered = null;
			}, 150);
		}
	}
	onDestroy(() => clearTimeout(unhover));

	// Bodies move under a resting pointer (zoom, playback): hover what is there now.
	let hoverFrame = 0;
	function requestRehover() {
		if (!hoverFrame) hoverFrame = requestAnimationFrame(rehover);
	}
	function rehover() {
		hoverFrame = 0;
		if (!lastPointer || dragging) return;
		setHovered(hit(lastPointer)?.key ?? null);
	}
	$effect(() => camera.subscribe(requestRehover));
	$effect(() => {
		void mapState.time;
		requestRehover();
	});

	onDestroy(() => cancelAnimationFrame(hoverFrame));

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
		const order = scene.items
			.filter((it) => !(hideFiltered && filtered?.has(it.key)))
			.sort((a, b) => a.priority - b.priority || Math.hypot(a.wx, a.wy) - Math.hypot(b.wx, b.wy));
		if (!order.length) return;
		const at = order.findIndex((it) => it.key === app.selected);
		const it =
			order[at < 0 ? (dir > 0 ? 0 : order.length - 1) : (at + dir + order.length) % order.length];
		select(it.key);
		if (!it.onScreen) centerOn(it);
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowRight' || e.key === 'ArrowDown') cycle(1);
		else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') cycle(-1);
		else if (e.key === 'Escape') {
			if (hoveredItem) hoverDismissed = hoveredItem.key;
			else app.selected = null;
		} else if (e.key === 'Backspace') goUp();
		else if (e.key === 'Enter' && app.selected) enterSystem(app.selected);
		else if (e.key === '+' || e.key === '=') zoomBy(1.25);
		else if (e.key === '-') zoomBy(0.8);
		else if (e.key === '0') fitAll();
		else if (e.key === ' ') mapState.playing = !mapState.playing;
		else return;
		e.preventDefault();
	}

	// --- overlays ---------------------------------------------------------------------

	type Box = [number, number, number, number];
	let hoverAnchor = $state<{ x: number; y: number; r: number; label: Box | null } | null>(null);
	/** Hover card hidden with Escape until the pointer leaves that item. */
	let hoverDismissed = $state<EntityKey | null>(null);
	$effect(() => {
		if (hovered !== hoverDismissed) untrack(() => (hoverDismissed = null));
	});
	const hoveredItem = $derived(
		hovered && !dragging && hovered !== hoverDismissed ? (scene?.byKey.get(hovered) ?? null) : null
	);

	/** Escape elsewhere in the page also hides the hover card, unless an open layer takes it. */
	function onwindowkeydown(e: KeyboardEvent) {
		// The surface's own handler prevents the default of the keys it handles.
		if (e.key !== 'Escape' || e.defaultPrevented || !hoveredItem) return;
		const layer =
			'[role=dialog], [role=alertdialog], [role=listbox], [role=menu], [data-slot=popover-content]';
		if (e.target instanceof Element && e.target.closest(layer)) return;
		if (document.querySelector(`:is(${layer})[data-state=open]`)) return;
		hoverDismissed = hoveredItem.key;
	}

	function updateHoverAnchor() {
		const it = hoveredItem;
		if (!it || !it.onScreen) {
			if (hoverAnchor) hoverAnchor = null;
			return;
		}
		const { sx: x, sy: y, size: r, labelBox } = it;
		const label = hoverAnchor?.label ?? null;
		const labelSame =
			labelBox === label || (!!labelBox && !!label && labelBox.every((v, i) => v === label[i]));
		if (hoverAnchor?.x !== x || hoverAnchor?.y !== y || hoverAnchor?.r !== r || !labelSame)
			hoverAnchor = { x, y, r, label: labelBox && [...labelBox] };
	}

	// Overlays the hover card keeps clear of, measured when it appears (container coordinates).
	let topLeft = $state<HTMLDivElement>();
	let topLeftWidth = $state(0);
	let topCenterWidth = $state(0);
	let topCenterHeight = $state(0);
	/**
	 * The breadcrumb column moves below the centered hotbar when it doesn't fit beside it. Its own
	 * width doesn't depend on where it sits, so this can't oscillate.
	 */
	const topLeftDrop = $derived(
		12 + topLeftWidth + 8 > (width - topCenterWidth) / 2 ? topCenterHeight + 8 : 0
	);
	let topCenter = $state<HTMLDivElement>();
	let bottomLeft = $state<HTMLDivElement>();
	let timeBar = $state<HTMLDivElement>();
	let obstacles = $state.raw<DOMRect[]>([]);
	// Measured whenever the overlays may have moved: the hover card and the label placer avoid them.
	$effect(() => {
		if (!container) return;
		void [
			width,
			height,
			hoveredItem,
			crumbs,
			enterable,
			innerCrowded,
			filterResult,
			data,
			loading,
			topLeftDrop,
			topCenterWidth,
			topCenterHeight
		];
		const origin = container.getBoundingClientRect();
		obstacles = [topLeft, topCenter, bottomLeft, timeBar].flatMap((el) => {
			const r = el?.getBoundingClientRect();
			return r && r.width > 0
				? [new DOMRect(r.x - origin.x, r.y - origin.y, r.width, r.height)]
				: [];
		});
	});

	/** Selected body with satellites, other than the center: offer to enter its system. */
	const enterable = $derived.by(() => {
		const it = app.selected ? scene?.byKey.get(app.selected) : undefined;
		return it && !it.point && it.body !== scene?.center && it.body?.children.length ? it : null;
	});

	// Polite announcements: the system when it changes, then the selection.
	let announcement = $state('');
	let announcedSystem: EntityKey | null = null;
	let announcedSelection: EntityKey | null = null;
	$effect(() => {
		const s = scene;
		const sel = app.selected;
		untrack(() => {
			const parts: string[] = [];
			const systemChanged = !!s && s.center.key !== announcedSystem;
			if (systemChanged) {
				announcedSystem = s.center.key;
				parts.push(systemSummary(s));
			}
			if (sel !== announcedSelection) {
				announcedSelection = sel;
				const it = sel ? s?.byKey.get(sel) : undefined;
				if (it) parts.push([it.name, TYPE_LABELS[it.type], ...filterStatus(it)].join(', '));
			} else if (!systemChanged) return;
			let text = parts.join('. ');
			// Screen readers skip a region whose text didn't change.
			if (text && text === announcement) text += '\u00a0';
			announcement = text;
		});
	});

	/** Whether the item passes the active filters, in words (none when no filter is on). */
	function filterStatus(it: MapItem): string[] {
		const f = filterResult;
		if (!f) return [];
		const below = f.gatewayMatches.get(it.key);
		if (below) {
			const what = below.bodies
				? count(below.bodies, 'matching moon', 'matching moons')
				: count(below.points, 'matching Lagrange point', 'matching Lagrange points');
			return [`doesn't match filters; ${what} in its system, press Enter to open`];
		}
		return [f.filtered.has(it.key) ? "doesn't match filters" : 'matches filters'];
	}

	const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

	function systemSummary(s: SystemScene) {
		const points = s.items.filter((it) => it.point).length;
		const bodies = s.items.length - points - 1;
		return [
			`${s.center.info.entity.name} system`,
			count(bodies, 'body', 'bodies'),
			...(points ? [count(points, 'Lagrange point', 'Lagrange points')] : [])
		].join(', ');
	}

	/** Mouse and keyboard controls, for the help popover and screen readers. */
	const HELP: [string, string][] = [
		['Click', 'Show a body’s details'],
		['Double-click', 'Enter its system of moons'],
		['Drag', 'Pan'],
		['Wheel', 'Zoom'],
		['Arrow keys', 'Step through the bodies and Lagrange points'],
		['Enter', 'Open the selected body’s system'],
		['Backspace', 'Go up one level'],
		['Space', 'Play or pause time'],
		['+ / −', 'Zoom in or out'],
		['0', 'Fit the system'],
		['Escape', 'Hide the hover card, or clear the selection']
	];
</script>

<svelte:window onkeydown={onwindowkeydown} />

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
		class:cursor-pointer={overItem && !dragging}
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
			setHovered(null);
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

	<div
		bind:this={topLeft}
		bind:clientWidth={topLeftWidth}
		class="absolute left-3 flex flex-col items-start gap-2"
		style:top="{12 + topLeftDrop}px"
	>
		{#if crumbs.length}
			<div class="rounded-xl border bg-card px-3 py-1 shadow">
				<Breadcrumb.Root class="flex h-8 items-center">
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
											keepFocus(() => (app.mapSystem = i === 0 ? null : body.key));
										}}>{body.info.entity.name}</Breadcrumb.Link
									>
								{/if}
							</Breadcrumb.Item>
						{/each}
					</Breadcrumb.List>
				</Breadcrumb.Root>
			</div>
		{/if}
		{#if filterResult}
			<div
				class="flex max-w-md items-center gap-2 rounded-full border bg-card py-0.5 pr-0.5 pl-3 text-xs shadow"
				role="status"
			>
				<span>
					{#if filterResult.matching}
						{filterResult.matching} of {filterResult.total} match in this system
					{:else}
						No bodies in this system match the filters{#if filterResult.gateways.size}
							· some of their moons do{/if}
					{/if}
				</span>
				{#if filterResult.gateways.size}
					<!-- Key to the canvas's gateway marker (see render.ts). -->
					<span class="flex shrink-0 items-center gap-1 text-muted-foreground">
						<svg viewBox="0 0 16 16" class="size-4" aria-hidden="true">
							<circle cx="8" cy="8" r="2.5" fill="currentColor" />
							<circle
								cx="8"
								cy="8"
								r="6.5"
								fill="none"
								stroke="currentColor"
								stroke-dasharray="2 2"
							/>
						</svg>
						contains matches
					</span>
				{/if}
				<Button
					variant="ghost"
					size="sm"
					class="h-6 rounded-full px-2 text-xs"
					onclick={() => keepFocus(() => mapState.clearFilters())}>Clear</Button
				>
			</div>
		{/if}
	</div>

	<div
		bind:this={topCenter}
		bind:clientWidth={topCenterWidth}
		bind:clientHeight={topCenterHeight}
		class="absolute top-3 left-1/2 flex -translate-x-1/2 flex-col items-center gap-2"
	>
		<MapHotbar />
		{#if loading}
			<div
				class="pointer-events-none flex items-center gap-2 rounded-full border bg-card px-3 py-1 text-sm text-muted-foreground shadow"
				role="status"
			>
				<LoaderCircle class="size-4 animate-spin" /> Loading solar system…
			</div>
		{/if}
		{#if enterable}
			<Button
				variant="outline"
				size="sm"
				class="rounded-full shadow"
				onclick={() => keepFocus(() => enterSystem(enterable.key))}
			>
				<LogIn /> Enter {enterable.name} system
			</Button>
		{/if}
		{#if innerCrowded}
			<Button
				variant="outline"
				size="sm"
				class="rounded-full shadow"
				onclick={() => keepFocus(zoomToInner)}
			>
				<ZoomIn /> Zoom to inner planets
			</Button>
		{/if}
	</div>

	{#if hoveredItem && hoverAnchor}
		<MapHoverCard
			item={hoveredItem}
			x={hoverAnchor.x}
			y={hoverAnchor.y}
			r={hoverAnchor.r}
			labelBox={hoverAnchor.label}
			{width}
			{height}
			{obstacles}
			gatewayMatches={filterResult?.gatewayMatches.get(hoveredItem.key) ?? null}
			onpointerenter={() => hoveredItem && setHovered(hoveredItem.key)}
			onpointerleave={() => setHovered(null)}
			{onwheel}
		/>
	{/if}

	<!-- Same order as the tree canvas: zoom in, zoom out, fit, then help at the bottom. -->
	<div bind:this={bottomLeft} class="absolute bottom-3 left-3 flex flex-col gap-1">
		<div class="flex flex-col gap-1" role="group" aria-label="Zoom">
			<Button
				variant="outline"
				size="icon"
				class={canvasControl}
				aria-label="Zoom in"
				onclick={() => zoomBy(1.25)}
			>
				<Plus />
			</Button>
			<Button
				variant="outline"
				size="icon"
				class={canvasControl}
				aria-label="Zoom out"
				onclick={() => zoomBy(0.8)}
			>
				<Minus />
			</Button>
			<Button
				variant="outline"
				size="icon"
				class={canvasControl}
				aria-label="Fit system"
				onclick={() => fitAll()}
			>
				<Maximize />
			</Button>
		</div>
		<Popover.Root>
			<Popover.Trigger>
				{#snippet child({ props })}
					<Button
						{...props}
						variant="outline"
						size="icon"
						class={canvasControl}
						aria-label="Map controls help"
					>
						<CircleQuestionMark />
					</Button>
				{/snippet}
			</Popover.Trigger>
			<Popover.Content
				class="w-80 text-sm"
				side="right"
				align="end"
				aria-labelledby="map-help-title"
			>
				<Popover.Header>
					<!-- Popover.Title is a div: a real heading, styled the same. -->
					<h2 id="map-help-title" data-slot="popover-title" class="font-medium">Map controls</h2>
				</Popover.Header>
				<dl class="mt-3 grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-1.5">
					{#each HELP as [keys, action] (keys)}
						<dt>
							<kbd
								class="rounded border bg-muted px-1.5 py-0.5 font-sans text-xs whitespace-nowrap text-muted-foreground"
								>{keys}</kbd
							>
						</dt>
						<dd>{action}</dd>
					{/each}
				</dl>
			</Popover.Content>
		</Popover.Root>
	</div>

	{#if data}
		<div
			bind:this={timeBar}
			class="absolute bottom-3 left-1/2 max-w-[calc(100%-6rem)] -translate-x-1/2"
		>
			<TimeBar />
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
