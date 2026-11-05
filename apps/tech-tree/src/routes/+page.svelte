<script lang="ts">
	import { onMount } from 'svelte';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import PanelRightOpen from '@lucide/svelte/icons/panel-right-open';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { api, mockMode } from '$lib/api';
	import DetailPanel from '$lib/components/detail/DetailPanel.svelte';
	import ScopeBar from '$lib/components/scope/ScopeBar.svelte';
	import WikiDialog from '$lib/components/wiki/WikiDialog.svelte';
	import SearchPalette from '$lib/components/search/SearchPalette.svelte';
	import SettingsDialog from '$lib/components/settings/SettingsDialog.svelte';
	import Pill from '$lib/components/toolbar/Pill.svelte';
	import TreeCanvas from '$lib/components/tree/TreeCanvas.svelte';
	import TreeFilters from '$lib/components/tree/TreeFilters.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { LightSwitch } from '$lib/components/ui/light-switch/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import { app } from '$lib/state.svelte';

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
		<div class="flex h-screen overflow-hidden">
			<main class="min-w-0 flex-1">
				<h1 class="sr-only">Terra Invicta Tech Tree</h1>
				<TreeCanvas>
					{#snippet leading()}
						<ScopeBar />
						<TreeFilters />
					{/snippet}
					{#snippet trailing()}
						<Pill aria-label="Search">
							<SearchPalette />
							<WikiDialog />
						</Pill>
						<Pill aria-label="System">
							{#if mockMode}
								<Badge variant="outline" class="ml-1 text-amber-600 dark:text-amber-400"
									>mock data</Badge
								>
							{/if}
							{#if !app.panelOpen}
								<Button
									variant="ghost"
									size="icon"
									class="rounded-full"
									aria-label="Show details panel"
									title="Show details panel"
									onclick={() => (app.panelOpen = true)}
								>
									<PanelRightOpen />
								</Button>
							{/if}
							<SettingsDialog />
							<LightSwitch variant="ghost" class="rounded-full" />
						</Pill>
					{/snippet}
				</TreeCanvas>
			</main>
			{#if app.panelOpen}
				<div class="w-[420px] shrink-0">
					<DetailPanel />
				</div>
			{/if}
		</div>
	{/if}
</Tooltip.Provider>
