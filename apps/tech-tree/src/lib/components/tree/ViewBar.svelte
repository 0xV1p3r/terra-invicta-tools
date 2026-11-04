<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import ArrowRight from '@lucide/svelte/icons/arrow-right';
	import Undo2 from '@lucide/svelte/icons/undo-2';
	import type { Direction, TreeGraph } from '$lib/api';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Toggle } from '$lib/components/ui/toggle/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import { formatCost, formatCostFull } from '$lib/format';
	import { app, type FullLayout, type ViewKind } from '$lib/state.svelte';
	import CategoryIcon from './CategoryIcon.svelte';

	let { graph }: { graph: TreeGraph | null } = $props();

	const view = $derived(app.view);
	/** Root for switching to a sub-tree/focus view: the current root, else the selected research. */
	const candidate = $derived(
		app.root ?? (app.selected && app.treeKeys.has(app.selected) ? app.selected : null)
	);
	const root = $derived(app.root ? graph?.nodes.find((n) => n.key === app.root) : undefined);

	// The full tree is reached with "Back to full tree".
	const KINDS: { value: Exclude<ViewKind, 'full'>; label: string; title: string }[] = [
		{ value: 'subtree', label: 'Sub-tree', title: 'Requirements and/or dependents of one tech' },
		{ value: 'focus', label: 'Focus', title: 'One tech with its direct requirements and unlocks' }
	];

	const LAYOUTS: { value: FullLayout; label: string; title: string }[] = [
		{ value: 'layered', label: 'Layered', title: 'One drawing, columns by prerequisites and cost' },
		{ value: 'bands', label: 'By category', title: 'One horizontal band per research category' }
	];

	const DIRECTIONS: { value: Direction; label: string; title: string }[] = [
		{ value: 'up', label: 'Requirements', title: 'Everything needed to research the root' },
		{ value: 'down', label: 'Dependents', title: 'Everything the root leads to' },
		{ value: 'both', label: 'Both', title: 'Requirements and dependents' }
	];

	function setKind(kind: ViewKind) {
		if (kind === view.kind) return;
		if (candidate && kind === 'subtree') app.openSubtree(candidate, 'up');
		else if (candidate && kind === 'focus') app.openFocus(candidate);
	}
</script>

<div
	class="flex flex-wrap items-center gap-3 border-b bg-card/95 px-3 py-2 text-sm backdrop-blur"
	role="toolbar"
	aria-label="View"
>
	{#if view.kind !== 'full'}
		<Button
			variant="default"
			size="sm"
			title="Back to the full tree, centered on the selection"
			onclick={() => app.showFullTree(app.selected ?? app.root)}
		>
			<Undo2 /> Back to full tree
		</Button>
	{/if}
	<div class="flex gap-1">
		<Button
			variant="outline"
			size="icon"
			class="size-8"
			aria-label="Back"
			title="Back (Alt+←)"
			disabled={app.back.length === 0}
			onclick={() => app.goBack()}
		>
			<ArrowLeft />
		</Button>
		<Button
			variant="outline"
			size="icon"
			class="size-8"
			aria-label="Forward"
			title="Forward (Alt+→)"
			disabled={app.forward.length === 0}
			onclick={() => app.goForward()}
		>
			<ArrowRight />
		</Button>
	</div>

	<ToggleGroup.Root
		type="single"
		size="sm"
		variant="outline"
		value={view.kind === 'full' ? '' : view.kind}
		onValueChange={(v) => v && setKind(v as ViewKind)}
		aria-label="View kind"
	>
		{#each KINDS as k (k.value)}
			<ToggleGroup.Item
				value={k.value}
				title={candidate ? k.title : 'Select a tech or project first'}
				disabled={!candidate}
				class="px-3"
			>
				{k.label}
			</ToggleGroup.Item>
		{/each}
	</ToggleGroup.Root>

	{#if view.kind === 'full'}
		<ToggleGroup.Root
			type="single"
			size="sm"
			variant="outline"
			value={view.layout}
			onValueChange={(v) => v && app.update({ kind: 'full', layout: v as FullLayout })}
			aria-label="Layout"
		>
			{#each LAYOUTS as l (l.value)}
				<ToggleGroup.Item value={l.value} title={l.title} class="px-3">{l.label}</ToggleGroup.Item>
			{/each}
		</ToggleGroup.Root>
	{:else}
		<div class="flex min-w-0 items-center gap-2">
			{#if root}
				<CategoryIcon category={root.category} size={22} />
			{/if}
			<button
				class="truncate font-semibold hover:underline"
				onclick={() => app.root && app.navigate({ key: app.root, kind: root?.kind ?? 'tech' })}
			>
				{root?.name ?? app.root}
			</button>
		</div>
	{/if}

	{#if view.kind === 'subtree'}
		{@const sub = view}
		<ToggleGroup.Root
			type="single"
			size="sm"
			variant="outline"
			value={sub.direction}
			onValueChange={(v) => {
				if (v) app.update({ ...sub, direction: v as Direction });
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
			bind:pressed={() => sub.alternatives, (alternatives) => app.update({ ...sub, alternatives })}
			title="Include alternative prerequisites (altPrereq)"
		>
			Alternatives
		</Toggle>
	{/if}

	{#if graph && view.kind !== 'full'}
		<span class="ml-auto text-muted-foreground" title="{formatCostFull(graph.totalCost)} research">
			{#if view.kind === 'subtree'}{graph.nodes.length} items ·{/if}
			research needed for root
			<span class="font-semibold text-foreground">{formatCost(graph.totalCost)}</span>
		</span>
	{/if}
</div>
