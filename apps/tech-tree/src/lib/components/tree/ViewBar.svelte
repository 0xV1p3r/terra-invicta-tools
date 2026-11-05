<script lang="ts">
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import ArrowRight from '@lucide/svelte/icons/arrow-right';
	import type { Direction, TreeGraph } from '$lib/api';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Switch } from '$lib/components/ui/switch/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import { app, type FullLayout, type ViewKind } from '$lib/state.svelte';
	import Pill, { pillIcon, pillToggle, segmented } from '$lib/components/toolbar/Pill.svelte';
	import PillDivider from '$lib/components/toolbar/PillDivider.svelte';
	import Tip from '$lib/components/toolbar/Tip.svelte';
	import CategoryIcon from './CategoryIcon.svelte';

	let { graph }: { graph: TreeGraph | null } = $props();

	const view = $derived(app.view);
	/** Root for switching to a sub-tree/focus view: the selected research, else the current root. */
	const candidate = $derived(
		(app.selected && app.treeKeys.has(app.selected) ? app.selected : null) ?? app.root
	);
	const root = $derived(app.root ? graph?.nodes.find((n) => n.key === app.root) : undefined);

	const rootName = $derived(
		(root?.name ?? app.root ?? '')
			.replace(/^TI[^:]*Template:/, '')
			.replace(/([a-z])([A-Z])/g, '$1 $2')
	);

	const KINDS: { value: ViewKind; label: string; title: string }[] = [
		{ value: 'full', label: 'Full tree', title: 'Full tree: every tech and project' },
		{
			value: 'subtree',
			label: 'Sub-tree',
			title: 'Sub-tree: everything it requires/unlocks, all the way'
		},
		{
			value: 'focus',
			label: 'Focus',
			title: 'Focus: direct requirements and unlocks of the selected item'
		}
	];

	/** Tooltip for a view option; sub-tree/focus need a root, full tree recenters on leaving. */
	function kindTitle(k: (typeof KINDS)[number]) {
		if (k.value === 'full')
			return view.kind === 'full' ? k.title : 'Back to the full tree, centered on the selection';
		return candidate ? k.title : `${k.title}. Select a tech or project first.`;
	}

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
		{ value: 'down', label: 'Unlocks', title: 'Everything the root unlocks, all the way down' },
		{ value: 'both', label: 'Both', title: 'Requirements and unlocks' }
	];

	function setKind(kind: ViewKind) {
		if (kind === view.kind) return;
		if (kind === 'full') app.showFullTree(app.selected ?? app.root);
		else if (candidate && kind === 'subtree') app.openSubtree(candidate, 'up');
		else if (candidate && kind === 'focus') app.openFocus(candidate);
	}
</script>

<Pill aria-label="View">
	<Tip text="Back" shortcut="Alt+←">
		<Button
			variant="ghost"
			size="icon"
			class={pillIcon}
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
			class={pillIcon}
			aria-label="Forward"
			aria-keyshortcuts="Alt+ArrowRight"
			disabled={app.forward.length === 0}
			onclick={() => app.goForward()}
		>
			<ArrowRight />
		</Button>
	</Tip>

	<ToggleGroup.Root
		type="single"
		size="sm"
		spacing={0.5}
		class={segmented}
		bind:value={() => view.kind, (v) => v && setKind(v as ViewKind)}
		aria-label="View: what is shown"
	>
		{#each KINDS as k (k.value)}
			<Tip text={kindTitle(k)}>
				<!-- aria-disabled rather than disabled, so keyboard users can reach the explanation. -->
				<ToggleGroup.Item
					value={k.value}
					aria-disabled={k.value !== 'full' && !candidate}
					class={pillToggle}
				>
					{k.label}
				</ToggleGroup.Item>
			</Tip>
		{/each}
	</ToggleGroup.Root>

	{#if view.kind === 'full'}
		<ToggleGroup.Root
			type="single"
			size="sm"
			spacing={0.5}
			class={segmented}
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
			<Tip text="Show {rootName} details">
				<Button
					variant="ghost"
					size="sm"
					class="h-8 max-w-48 min-w-0 px-1 font-semibold"
					onclick={() => app.root && app.navigate({ key: app.root, kind: root?.kind ?? 'tech' })}
				>
					<span class="truncate">{rootName}</span>
				</Button>
			</Tip>
		</div>
	{/if}
</Pill>
{#if view.kind === 'subtree'}
	{@const sub = view}
	<Pill aria-label="Relationships">
		<ToggleGroup.Root
			type="single"
			size="sm"
			spacing={0.5}
			class={segmented}
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
		<Tip text="Include alternative prerequisites">
			<div class="flex h-8 items-center gap-2 rounded-md border border-input px-2.5">
				<Switch
					id="subtree-alternatives"
					size="sm"
					bind:checked={
						() => sub.alternatives, (alternatives) => app.update({ ...sub, alternatives })
					}
				/>
				<Label for="subtree-alternatives">Alternatives</Label>
			</div>
		</Tip>
	</Pill>
{/if}
