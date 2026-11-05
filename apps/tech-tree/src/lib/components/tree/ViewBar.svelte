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
	import Pill, { pillItem, pillToggle } from '$lib/components/toolbar/Pill.svelte';
	import PillDivider from '$lib/components/toolbar/PillDivider.svelte';
	import Tip from '$lib/components/toolbar/Tip.svelte';
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
		{
			value: 'subtree',
			label: 'Sub-tree',
			title: 'Sub-tree: everything the selected tech requires and/or unlocks, all the way down'
		},
		{
			value: 'focus',
			label: 'Focus',
			title: 'Focus: the selected tech with only its direct requirements and unlocks'
		}
	];

	const LAYOUTS: { value: FullLayout; label: string; title: string }[] = [
		{
			value: 'layered',
			label: 'Layered',
			title: 'Layered: one drawing, columns by prerequisites and cost'
		},
		{
			value: 'bands',
			label: 'By category',
			title: 'By category: one horizontal band per research category'
		}
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

<Pill class="min-w-0 flex-wrap rounded-3xl" role="toolbar" aria-label="View">
	{#if view.kind !== 'full'}
		<Tip text="Back to the full tree, centered on the selection">
			<Button
				variant="default"
				size="sm"
				class={pillItem}
				onclick={() => app.showFullTree(app.selected ?? app.root)}
			>
				<Undo2 /> Full tree
			</Button>
		</Tip>
	{/if}
	<Tip text="Back" shortcut="Alt+←">
		<Button
			variant="ghost"
			size="icon"
			class="size-8 rounded-full"
			aria-label="Back"
			aria-keyshortcuts="Alt+ArrowLeft"
			disabled={app.back.length === 0}
			onclick={() => app.goBack()}
		>
			<ArrowLeft />
		</Button>
	</Tip>
	<Tip text="Forward" shortcut="Alt+→">
		<Button
			variant="ghost"
			size="icon"
			class="size-8 rounded-full"
			aria-label="Forward"
			aria-keyshortcuts="Alt+ArrowRight"
			disabled={app.forward.length === 0}
			onclick={() => app.goForward()}
		>
			<ArrowRight />
		</Button>
	</Tip>

	<PillDivider />
	<span class="group-label" aria-hidden="true">View</span>
	<ToggleGroup.Root
		type="single"
		size="sm"
		spacing={0.5}
		value={view.kind === 'full' ? '' : view.kind}
		onValueChange={(v) => v && setKind(v as ViewKind)}
		aria-label="View: what is shown"
	>
		{#each KINDS as k (k.value)}
			<Tip text={candidate ? k.title : `${k.title}. Select a tech or project first.`}>
				<ToggleGroup.Item value={k.value} disabled={!candidate} class={pillToggle}>
					{k.label}
				</ToggleGroup.Item>
			</Tip>
		{/each}
	</ToggleGroup.Root>

	{#if view.kind === 'full'}
		<PillDivider />
		<span class="group-label" aria-hidden="true">Layout</span>
		<ToggleGroup.Root
			type="single"
			size="sm"
			spacing={0.5}
			value={view.layout}
			onValueChange={(v) => v && app.update({ kind: 'full', layout: v as FullLayout })}
			aria-label="Layout: how the tree is arranged"
		>
			{#each LAYOUTS as l (l.value)}
				<Tip text={l.title}>
					<ToggleGroup.Item value={l.value} class={pillToggle}>{l.label}</ToggleGroup.Item>
				</Tip>
			{/each}
		</ToggleGroup.Root>
	{:else}
		<PillDivider />
		<div class="flex min-w-0 items-center gap-2 px-1">
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
		<PillDivider />
		<ToggleGroup.Root
			type="single"
			size="sm"
			spacing={0.5}
			value={sub.direction}
			onValueChange={(v) => {
				if (v) app.update({ ...sub, direction: v as Direction });
			}}
			aria-label="Direction"
		>
			{#each DIRECTIONS as d (d.value)}
				<Tip text={d.title}>
					<ToggleGroup.Item value={d.value} class={pillToggle}>{d.label}</ToggleGroup.Item>
				</Tip>
			{/each}
		</ToggleGroup.Root>
		<Tip text="Include alternative prerequisites (altPrereq)">
			<Toggle
				size="sm"
				class={pillToggle}
				bind:pressed={
					() => sub.alternatives, (alternatives) => app.update({ ...sub, alternatives })
				}
			>
				Alternatives
			</Toggle>
		</Tip>
	{/if}

	{#if graph && view.kind !== 'full'}
		<PillDivider />
		<span
			class="pr-2 whitespace-nowrap text-muted-foreground"
			title="{formatCostFull(graph.totalCost)} research"
		>
			{#if view.kind === 'subtree'}{graph.nodes.length} items ·{/if}
			research needed for root
			<span class="font-semibold text-foreground">{formatCost(graph.totalCost)}</span>
		</span>
	{/if}
</Pill>

<style>
	.group-label {
		padding-inline: 0.25rem 0.125rem;
		font-size: 0.6875rem;
		font-weight: 500;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		color: var(--muted-foreground);
		user-select: none;
	}
</style>
