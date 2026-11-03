<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import type { Direction, TreeGraph } from '$lib/api';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Toggle } from '$lib/components/ui/toggle/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import { formatCost, formatCostFull } from '$lib/format';
	import { app } from '$lib/state.svelte';
	import CategoryIcon from './CategoryIcon.svelte';

	let { graph }: { graph: TreeGraph | null } = $props();

	const root = $derived(graph?.nodes.find((n) => n.key === (graph?.root ?? app.subtreeRoot)));

	const DIRECTIONS: { value: Direction; label: string; title: string }[] = [
		{ value: 'up', label: 'Requirements', title: 'Everything needed to research the root' },
		{ value: 'down', label: 'Dependents', title: 'Everything the root leads to' },
		{ value: 'both', label: 'Both', title: 'Requirements and dependents' }
	];
</script>

<div
	class="flex flex-wrap items-center gap-3 border-b bg-card/95 px-3 py-2 text-sm backdrop-blur"
	role="toolbar"
	aria-label="Sub-tree options"
>
	<Button
		variant="outline"
		size="sm"
		onclick={() => app.showFullTree(app.selected ?? app.subtreeRoot)}
	>
		<ArrowLeft />
		Back to full tree
	</Button>
	<div class="flex min-w-0 items-center gap-2">
		{#if root}
			<CategoryIcon category={root.category} size={22} />
		{/if}
		<button
			class="truncate font-semibold hover:underline"
			onclick={() =>
				app.subtreeRoot && app.navigate({ key: app.subtreeRoot, kind: root?.kind ?? 'tech' })}
		>
			{root?.name ?? app.subtreeRoot}
		</button>
	</div>
	<ToggleGroup.Root
		type="single"
		size="sm"
		variant="outline"
		value={app.subtreeDirection}
		onValueChange={(v) => {
			if (v) app.subtreeDirection = v as Direction;
		}}
		aria-label="Direction"
	>
		{#each DIRECTIONS as d (d.value)}
			<ToggleGroup.Item value={d.value} title={d.title} class="px-3">{d.label}</ToggleGroup.Item>
		{/each}
	</ToggleGroup.Root>
	<Toggle
		size="sm"
		variant="outline"
		bind:pressed={app.subtreeAlternatives}
		title="Include alternative prerequisites (altPrereq)"
	>
		Alternatives
	</Toggle>
	{#if graph}
		<span class="ml-auto text-muted-foreground" title="{formatCostFull(graph.totalCost)} research">
			{graph.nodes.length} items · research needed for root
			<span class="font-semibold text-foreground">{formatCost(graph.totalCost)}</span>
		</span>
	{/if}
</div>
