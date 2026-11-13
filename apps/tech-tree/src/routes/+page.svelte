<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
	import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';
	import PanelRightClose from '@lucide/svelte/icons/panel-right-close';
	import PanelRightOpen from '@lucide/svelte/icons/panel-right-open';
	import { api, mockMode, type ExtractArgs, type TreeGraph } from '$lib/api';
	import DetailPanel from '$lib/components/detail/DetailPanel.svelte';
	import Onboarding from '$lib/components/onboarding/Onboarding.svelte';
	import UpdateNotice from '$lib/components/onboarding/UpdateNotice.svelte';
	import ScopeBar from '$lib/components/scope/ScopeBar.svelte';
	import WikiDialog from '$lib/components/wiki/WikiDialog.svelte';
	import SearchPalette from '$lib/components/search/SearchPalette.svelte';
	import SettingsDialog from '$lib/components/settings/SettingsDialog.svelte';
	import Pill, { pillIcon } from '$lib/components/toolbar/Pill.svelte';
	import PillDivider from '$lib/components/toolbar/PillDivider.svelte';
	import Tip from '$lib/components/toolbar/Tip.svelte';
	import TreeCanvas from '$lib/components/tree/TreeCanvas.svelte';
	import TreeFilters from '$lib/components/tree/TreeFilters.svelte';
	import ViewBar from '$lib/components/tree/ViewBar.svelte';
	import MapPane from '$lib/components/map/MapPane.svelte';
	import MapToolbar from '$lib/components/map/MapToolbar.svelte';
	import { mapState } from '$lib/components/map/map-state.svelte';
	import SolarMap from '$lib/components/map/SolarMap.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import { extraction } from '$lib/extraction.svelte';
	import { app } from '$lib/state.svelte';
	import { wiki } from '$lib/wiki.svelte';

	// The map mounts on first use and then stays (like the tree) so switching back is instant.
	let mapMounted = $state(false);
	$effect(() => {
		if (app.page === 'map') mapMounted = true;
	});

	/** The tree on screen, for the view controls (null while a new view loads). */
	let treeGraph = $state.raw<TreeGraph | null>(null);
	/**
	 * Keeps the utility cluster at the right end of the toolbar's first row by moving it, in the
	 * DOM, to just after the groups that fit beside it. The flex row then wraps the rest below, and
	 * focus visits row 1 (utilities included) before row 2. Groups don't shrink, so their widths
	 * don't depend on where the utilities sit.
	 */
	const pinUtilities: Attachment<HTMLElement> = (toolbar) => {
		const utilities = toolbar.querySelector<HTMLElement>(':scope > [data-utilities]');
		if (!utilities) return;
		const outer = (el: Element) => {
			const style = getComputedStyle(el);
			return (
				el.getBoundingClientRect().width +
				parseFloat(style.marginLeft) +
				parseFloat(style.marginRight)
			);
		};
		const place = () => {
			const style = getComputedStyle(toolbar);
			const gap = parseFloat(style.columnGap) || 0;
			let room =
				toolbar.clientWidth -
				parseFloat(style.paddingLeft) -
				parseFloat(style.paddingRight) -
				utilities.getBoundingClientRect().width;
			let before: Element | null = null;
			for (const group of toolbar.children) {
				if (group === utilities) continue;
				room -= outer(group) + gap;
				// Half a pixel of slack for subpixel widths.
				if (room < -0.5) {
					before = group;
					break;
				}
			}
			if (utilities.nextElementSibling !== before) move(before);
		};
		// moveBefore keeps focus (and tooltip state) on the moved controls; insertBefore drops
		// focus to the body, so put it back.
		const move = (before: Element | null) => {
			const host = toolbar as HTMLElement & {
				moveBefore?: (node: Node, child: Node | null) => void;
			};
			if (host.moveBefore) {
				try {
					host.moveBefore(utilities, before);
					return;
				} catch {
					// Fall through: moveBefore refuses some moves insertBefore allows.
				}
			}
			const focused = document.activeElement;
			const refocus = focused instanceof HTMLElement && utilities.contains(focused);
			toolbar.insertBefore(utilities, before);
			if (refocus && document.activeElement !== focused) focused.focus({ preventScroll: true });
		};
		const resized = new ResizeObserver(place);
		const observe = () => {
			resized.disconnect();
			resized.observe(toolbar);
			for (const group of toolbar.children) resized.observe(group);
		};
		// Groups come and go with the view (e.g. sub-tree relationships).
		const mutated = new MutationObserver(() => {
			observe();
			place();
		});
		mutated.observe(toolbar, { childList: true });
		observe();
		place();
		return () => {
			mutated.disconnect();
			resized.disconnect();
		};
	};

	// Scenario and faction names are localized: refetch on a language change (the old metadata
	// stays on screen meanwhile).
	$effect(() => {
		const lang = app.scope.lang;
		untrack(() => void loadMeta(lang).catch(() => {}));
	});

	let metaRequest = 0;
	async function loadMeta(lang = app.scope.lang, switched = false) {
		const request = ++metaRequest;
		try {
			const meta = await api.getMeta(lang);
			if (request !== metaRequest) return;
			app.meta = meta;
			app.error = null;
		} catch (cause) {
			if (request !== metaRequest) return;
			if (!app.meta || switched) app.error = String(cause);
			throw cause;
		}
	}

	let dataRevision = $state(0);
	function resetData() {
		app.showFullTree();
		app.selected = null;
		app.focus = null;
		app.mapSystem = null;
		app.mapFocus = null;
		app.back = [];
		app.forward = [];
		app.treeKeys = new Set();
		app.clearFilters();
		wiki.reset();
		mapState.data = null;
		mapState.playing = false;
		mapState.time = NaN;
		mapState.start = NaN;
		treeGraph = null;
		mapMounted = false;
		dataRevision += 1;
	}

	async function chooseDataDir(current?: string) {
		const { open } = await import('@tauri-apps/plugin-dialog');
		const path = await open({ directory: true, defaultPath: current });
		if (typeof path !== 'string') return;
		await api.setDataDir(path);
		try {
			await loadMeta(app.scope.lang, true);
		} finally {
			resetData();
		}
	}

	async function switchBuild(buildId: string | null) {
		await api.setBuild(buildId);
		try {
			await loadMeta(app.scope.lang, true);
		} finally {
			resetData();
		}
	}

	// After a window reload, follow an extraction that is still running; then show its build.
	onMount(() => {
		void extraction.resume().then(async (finished) => {
			if (!finished) return;
			try {
				await loadMeta(app.scope.lang, true);
			} catch {
				// Shown by `app.error`.
			} finally {
				resetData();
			}
		});
	});

	/** Extract the game data; once the backend has loaded the new build, show it. */
	async function extract(args: ExtractArgs) {
		if (!(await extraction.run(args))) return;
		try {
			await loadMeta(app.scope.lang, true);
		} finally {
			resetData();
		}
	}

	/** Remounts everything below on a data switch, so no component keeps data of the old build. */
	const dataKey = $derived(dataRevision);
</script>

<Tooltip.Provider delayDuration={300}>
	{#if app.error}
		<Onboarding {extract} chooseDataDir={() => chooseDataDir()} loadError={app.error} />
	{:else if !app.meta}
		<main class="grid h-screen place-items-center text-muted-foreground" role="status">
			<span class="flex items-center gap-2"
				><LoaderCircle class="size-5 animate-spin" /> Loading game data…</span
			>
		</main>
	{:else}
		{#key dataKey}
			<div class="flex h-screen flex-col overflow-hidden">
				<h1 class="sr-only">Terra Invicta Tools</h1>
				<!--
				Toolbar band across the window, so the details panel doesn't squeeze it. The groups wrap
				as wholes, and the utilities stay at the right end of the first row: `pinUtilities` moves
				them in the DOM to just after the last group that fits beside them, so Tab order follows
				the rows.
			-->
				<div
					class="@container relative z-10 flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1.5 border-b bg-card px-3 py-1.5 shadow-sm"
					role="region"
					aria-label={app.page === 'map' ? 'Map toolbar' : 'Tree toolbar'}
					{@attach pinUtilities}
				>
					{#if app.page === 'map'}
						<!-- Side panel toggles sit outermost, on their panel's side. -->
						<Pill aria-label="Bodies pane">
							<Tip text={mapState.paneOpen ? 'Hide bodies pane' : 'Show bodies pane'}>
								<Button
									variant="ghost"
									size="icon"
									class={pillIcon}
									aria-label="Bodies pane"
									aria-pressed={mapState.paneOpen}
									onclick={() => (mapState.paneOpen = !mapState.paneOpen)}
								>
									{#if mapState.paneOpen}<PanelLeftClose />{:else}<PanelLeftOpen />{/if}
								</Button>
							</Tip>
						</Pill>
					{/if}
					<ViewBar graph={treeGraph} />
					<ScopeBar />
					<div
						role="group"
						aria-label="Filters"
						class="flex max-w-full flex-wrap items-center gap-x-3 gap-y-1.5"
					>
						{#if app.page === 'tree'}
							<TreeFilters />
						{:else}
							<MapToolbar />
						{/if}
					</div>
					<Pill class="ml-auto gap-0.5" aria-label="Search and settings" data-utilities>
						<SearchPalette />
						<WikiDialog />
						<PillDivider />
						<SettingsDialog {chooseDataDir} {switchBuild} {extract} />
						<Tip
							text={app.selected
								? app.panelOpen
									? 'Hide details panel'
									: 'Show details panel'
								: app.page === 'map'
									? 'Select a body to see its details'
									: 'Select a tech or project to see its details'}
						>
							<!-- aria-disabled rather than disabled, so keyboard users can reach the explanation. -->
							<Button
								variant="ghost"
								size="icon"
								class="{pillIcon} aria-disabled:cursor-default aria-disabled:opacity-50 aria-disabled:hover:bg-transparent"
								aria-label="Details panel"
								aria-pressed={app.panelOpen}
								aria-disabled={!app.selected}
								onclick={() => {
									if (app.selected) app.panelOpen = !app.panelOpen;
								}}
							>
								{#if app.panelOpen}<PanelRightClose />{:else}<PanelRightOpen />{/if}
							</Button>
						</Tip>
					</Pill>
				</div>
				<UpdateNotice {extract} />
				<div class="flex min-h-0 flex-1">
					{#if app.page === 'map' && mapState.paneOpen}
						<div class="w-80 shrink-0">
							<MapPane />
						</div>
					{/if}
					<main class="min-w-0 flex-1" class:hidden={app.page !== 'tree'}>
						<TreeCanvas bind:graph={treeGraph} />
					</main>
					{#if mapMounted}
						<main class="min-w-0 flex-1" class:hidden={app.page !== 'map'}>
							<SolarMap />
						</main>
					{/if}
					{#if app.panelOpen && app.selected}
						<div class="w-[420px] shrink-0">
							<DetailPanel />
						</div>
					{/if}
				</div>
				{#if mockMode}
					<!-- Dev-only marker; kept out of the toolbar flow so it can't push the pills around. -->
					<Badge
						variant="outline"
						class="pointer-events-none fixed left-1/2 z-20 -translate-x-1/2 bg-card/90 text-amber-400 {app.page ===
						'map'
							? 'bottom-28'
							: 'bottom-3'}">mock data</Badge
					>
				{/if}
			</div>
		{/key}
	{/if}
</Tooltip.Provider>
