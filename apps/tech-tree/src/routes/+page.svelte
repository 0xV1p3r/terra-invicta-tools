<script lang="ts">
	import { onMount } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import PanelRightClose from '@lucide/svelte/icons/panel-right-close';
	import PanelRightOpen from '@lucide/svelte/icons/panel-right-open';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { api, mockMode, type TreeGraph } from '$lib/api';
	import DetailPanel from '$lib/components/detail/DetailPanel.svelte';
	import ScopeBar from '$lib/components/scope/ScopeBar.svelte';
	import WikiDialog from '$lib/components/wiki/WikiDialog.svelte';
	import SearchPalette from '$lib/components/search/SearchPalette.svelte';
	import SettingsDialog from '$lib/components/settings/SettingsDialog.svelte';
	import Pill from '$lib/components/toolbar/Pill.svelte';
	import PillDivider from '$lib/components/toolbar/PillDivider.svelte';
	import Tip from '$lib/components/toolbar/Tip.svelte';
	import TreeCanvas from '$lib/components/tree/TreeCanvas.svelte';
	import TreeFilters from '$lib/components/tree/TreeFilters.svelte';
	import ViewBar from '$lib/components/tree/ViewBar.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import { app } from '$lib/state.svelte';

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
			let room =
				toolbar.clientWidth -
				parseFloat(style.paddingLeft) -
				parseFloat(style.paddingRight) -
				utilities.getBoundingClientRect().width;
			let before: Element | null = null;
			for (const group of toolbar.children) {
				if (group === utilities) continue;
				room -= outer(group);
				// Half a pixel of slack for subpixel widths.
				if (room < -0.5) {
					before = group;
					break;
				}
			}
			if (utilities.nextElementSibling !== before) toolbar.insertBefore(utilities, before);
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

	onMount(() => {
		api.getMeta().then(
			(meta) => (app.meta = meta),
			(e) => (app.error = String(e))
		);
	});
</script>

<Tooltip.Provider delayDuration={300}>
	{#if app.error}
		<main class="grid h-screen place-items-center p-6">
			<div class="max-w-xl rounded-xl border bg-card p-6 shadow" role="alert">
				<div class="flex items-center gap-2">
					<TriangleAlert class="size-6 text-destructive" />
					<h1 class="text-lg font-semibold">Could not load the game data</h1>
				</div>
				<pre
					class="mt-4 overflow-auto rounded-lg bg-muted p-3 font-mono text-sm whitespace-pre-wrap">{app.error}</pre>
				<p class="mt-4 text-sm text-muted-foreground">
					Set <code class="text-foreground">TI_DATA_DIR</code> to the extracted data directory, or
					run <code class="text-foreground">ti-extract</code> to extract it from your Terra Invicta installation.
				</p>
			</div>
		</main>
	{:else if !app.meta}
		<main class="grid h-screen place-items-center text-muted-foreground" role="status">
			<span class="flex items-center gap-2"
				><LoaderCircle class="size-5 animate-spin" /> Loading game data…</span
			>
		</main>
	{:else}
		<div class="flex h-screen flex-col overflow-hidden">
			<h1 class="sr-only">Terra Invicta Tech Tree</h1>
			<!--
				Toolbar band across the window, so the details panel doesn't squeeze it. The groups wrap
				as wholes, and the utilities stay at the right end of the first row: `pinUtilities` moves
				them in the DOM to just after the last group that fits beside them, so Tab order follows
				the rows. Each group's -ml-px pushes its divider under the clip (no left padding, so that
				is the window edge) when it starts a row, so dividers never dangle.
			-->
			<div
				class="@container relative z-10 flex shrink-0 flex-wrap items-center overflow-hidden border-b bg-muted/60 py-1 pr-1"
				role="region"
				aria-label="Tree toolbar"
				{@attach pinUtilities}
			>
				<ViewBar graph={treeGraph} />
				<ScopeBar />
				<div role="group" aria-label="Filters" class="-ml-px flex max-w-full flex-wrap">
					<TreeFilters />
				</div>
				<Pill class="ml-auto before:hidden" aria-label="Search and settings" data-utilities>
					<SearchPalette />
					<WikiDialog />
					<PillDivider />
					<Tip
						text={app.selected
							? app.panelOpen
								? 'Hide details panel'
								: 'Show details panel'
							: 'Select a tech or project to see its details'}
					>
						<Button
							variant="ghost"
							size="icon"
							class="size-8"
							aria-label="Details panel"
							aria-pressed={app.panelOpen}
							disabled={!app.selected}
							onclick={() => (app.panelOpen = !app.panelOpen)}
						>
							{#if app.panelOpen}<PanelRightClose />{:else}<PanelRightOpen />{/if}
						</Button>
					</Tip>
					<SettingsDialog />
				</Pill>
			</div>
			<div class="flex min-h-0 flex-1">
				<main class="min-w-0 flex-1">
					<TreeCanvas bind:graph={treeGraph} />
				</main>
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
					class="pointer-events-none fixed bottom-3 left-1/2 z-20 -translate-x-1/2 bg-card/90 text-amber-400"
					>mock data</Badge
				>
			{/if}
		</div>
	{/if}
</Tooltip.Provider>
