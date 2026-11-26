<script lang="ts">
	import ArrowDownToLine from '@lucide/svelte/icons/arrow-down-to-line';
	import ArrowUpToLine from '@lucide/svelte/icons/arrow-up-to-line';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Crosshair from '@lucide/svelte/icons/crosshair';
	import Focus from '@lucide/svelte/icons/focus';
	import Globe from '@lucide/svelte/icons/globe';
	import Info from '@lucide/svelte/icons/info';
	import RotateCw from '@lucide/svelte/icons/rotate-cw';
	import Shuffle from '@lucide/svelte/icons/shuffle';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

	import {
		api,
		assetUrl,
		type EntityKey,
		type EntityDetail,
		type EntityRef,
		type UnlockItem,
		type UnlockKind
	} from '#lib/api.ts';
	import * as Card from '#lib/components/ui/card/index.ts';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import * as Tooltip from '#lib/components/ui/tooltip/index.js';

	import { Skeleton } from '#lib/components/ui/skeleton/index.js';
	import { categoryInfo } from '#lib/categories.ts';
	import {
		formatCost,
		formatCostFull,
		isResearch,
		KIND_LABELS,
		readableKey,
		REQUIREMENT_LABELS,
		requirementChoices,
		UNLOCK_LABELS
	} from '#lib/format.ts';
	import { app } from '#lib/state.svelte.ts';
	import EffectItem from './EffectItem.svelte';
	import EntityIcon from './EntityIcon.svelte';
	import EntityLink from './EntityLink.svelte';
	import StatBlock from './StatBlock.svelte';
	import UnlockCard from './UnlockCard.svelte';

	let detail = $state.raw<EntityDetail | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let latest = 0;
	// Bumped by Retry to refetch the same entity.
	let attempt = $state(0);
	// Names of loaded entities, so a failed reload (e.g. after a scope change) still names it.
	const names: Record<EntityKey, string> = {};

	$effect(() => {
		const selectedKey = entityKey;
		const scope = $state.snapshot(app.scope);
		void attempt;
		const id = ++latest;
		error = null;
		if (!selectedKey) {
			detail = null;
			loading = false;
			return;
		}
		detail = null;
		loading = true;
		api.getEntity(scope, selectedKey).then(
			(d) => {
				if (id !== latest) return;
				if (inWiki) wiki.remember(d.entity.key, !!d.entity.spoiler);
				names[d.entity.key] = d.entity.name;
				detail = d;
				loading = false;
			},
			(e) => {
				if (id !== latest) return;
				detail = null;
				error = String(e);
				loading = false;
			}
		);
	});

	const imageUrl = $derived(assetUrl(detail?.image ?? null));
	let imageFailed = $state(false);
	$effect(() => {
		void imageUrl;
		imageFailed = false;
	});
	const entity = $derived(detail?.entity ?? null);
	const research = $derived(entity ? isResearch(entity.kind) : false);
	const category = $derived(categoryInfo(entity?.category));

	// Some projects unlock well over a thousand items (e.g. territorial claims).
	const UNLOCK_PREVIEW = 25;
	let expanded = $state<Partial<Record<UnlockKind, boolean>>>({});
	$effect(() => {
		void entityKey;
		expanded = {};
	});

	const unlockGroups = $derived.by(() => {
		const groups: Partial<Record<UnlockKind, UnlockItem[]>> = {};
		for (const u of detail?.unlocks ?? []) (groups[u.kind] ??= []).push(u);
		return Object.entries(groups) as [UnlockKind, UnlockItem[]][];
	});

	const ann = $derived(detail?.annotations);

	// Shown in Requirements, Unlocks, Unlocked by or Effects: References leave these out.
	const shownElsewhere = $derived(
		new Set(
			[
				...(detail?.requirements ?? []).flatMap((g) => [...g.options, ...g.hidden]),
				...(detail?.unlocks ?? []).map((u) => u.entity),
				...(detail?.unlockedBy ?? []),
				...(detail?.effects ?? []).map((e) => e.entity)
			].map((e) => e.key)
		)
	);

	function scenarioLabel(name: string) {
		return app.meta?.scenarios.find((s) => s.name === name)?.label ?? name;
	}
	function factionLabel(name: string) {
		return app.meta?.factions.find((f) => f.name === name)?.label ?? name;
	}

	import { wikiIcon } from '#lib/wiki-icons.ts';
	import { wiki } from '#lib/wiki.svelte.ts';
	import { settings } from '#lib/settings.svelte.ts';
	import SpoilerChip from './SpoilerChip.svelte';
	import DataTree from './DataTree.svelte';
	import ReferenceGroups from './ReferenceGroups.svelte';
	let {
		entityKey,
		inWiki = false,
		shown = $bindable(null)
	}: { entityKey: EntityKey | null; inWiki?: boolean; shown?: EntityRef | null } = $props();
	// Lets the host name the loaded entity (e.g. in a sticky header).
	$effect(() => {
		shown = entity;
	});
	function treeAction(action: () => void) {
		if (inWiki) wiki.open = false;
		action();
	}
</script>

{#snippet section(title: string, hint?: string)}
	<Card.Content
		class="mt-5 mb-1.5 flex items-baseline justify-between gap-2 px-0 text-xs font-medium text-muted-foreground"
	>
		{title}
		{#if hint}<span class="font-normal text-faint">{hint}</span>{/if}
	</Card.Content>
{/snippet}

{#snippet or(show: boolean)}
	{#if show}<Card.Content class="inline px-0 text-xs text-muted-foreground">or</Card.Content>{/if}
{/snippet}

{#snippet iconAction(label: string, tip: string, onclick: () => void, Icon: typeof Focus)}
	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					variant="outline"
					size="sm"
					class="h-8 w-full"
					aria-label={label}
					{onclick}
				>
					<Icon />
					{label}
				</Button>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content>{tip}</Tooltip.Content>
	</Tooltip.Root>
{/snippet}

{#if !entityKey}
	<Card.Content class="px-0 text-sm text-muted-foreground">
		{#if app.page === 'map'}
			Click a body or Lagrange point to see its stats. Double-click a body, or use “Enter system”,
			to open its system of moons; the breadcrumb or Backspace goes back up. Search with Ctrl+K.
		{:else}
			Select a node in the tree or search (Ctrl+K) to see its details. Double-click a node to show
			its requirements tree.
		{/if}
	</Card.Content>
{:else if error}
	<Card.Content class="flex items-start gap-3 px-0" role="alert">
		<TriangleAlert class="mt-0.5 size-5 shrink-0 text-destructive" />
		<Card.Content class="min-w-0 px-0 text-sm">
			<Card.Content class="px-0 font-medium"
				>Could not load {names[entityKey] ?? readableKey(entityKey)}</Card.Content
			>
			<Card.Content class="mt-1 px-0 break-words text-muted-foreground">{error}</Card.Content>
			<Button variant="outline" size="sm" class="mt-3" onclick={() => attempt++}>
				<RotateCw /> Retry
			</Button>
		</Card.Content>
	</Card.Content>
{:else if loading && !detail}
	<Card.Content class="space-y-3 px-0" aria-busy="true" aria-label="Loading details">
		<Card.Content class="flex items-center gap-3 px-0">
			<Skeleton class="size-14 rounded-lg" />
			<Card.Content class="flex-1 space-y-2 px-0">
				<Skeleton class="h-5 w-3/4" />
				<Skeleton class="h-4 w-1/2" />
			</Card.Content>
		</Card.Content>
		<Skeleton class="h-16 w-full" />
		<Skeleton class="h-4 w-full" />
		<Skeleton class="h-4 w-5/6" />
		<Skeleton class="h-4 w-2/3" />
	</Card.Content>
{:else if detail && entity && inWiki && entity.spoiler && !settings.wikiSpoilers}
	<SpoilerChip /><Button class="mt-4" onclick={() => (settings.wikiSpoilers = true)}
		>Enable spoilers</Button
	>
{:else if detail && entity}
	<Card.Content class={loading ? 'px-0 opacity-60 transition-opacity' : 'px-0 transition-opacity'}>
		<Card.Content class="flex items-start gap-3 px-0">
			<Card.Content
				class="grid size-14 shrink-0 place-items-center rounded-lg border bg-muted/50 px-0"
				style={category ? `border-color: ${category.cssVar}` : undefined}
			>
				<EntityIcon
					{entity}
					size={inWiki ? 48 : 44}
					fallbackIcon={inWiki
						? wikiIcon(
								entity.template,
								wiki.sections.find((s) => s.template === entity.template)?.group
							)
						: undefined}
				/>
			</Card.Content>
			<Card.Content class="min-w-0 flex-1 px-0">
				<Card.Content
					class="px-0 leading-tight font-semibold break-words {inWiki ? 'text-2xl' : 'text-lg'}"
					>{entity.name}</Card.Content
				>
				<Card.Content class="mt-2 flex flex-wrap gap-1 px-0">
					<Badge variant="outline" class="text-muted-foreground"
						>{KIND_LABELS[entity.kind] ?? entity.kind}</Badge
					>
					{#if inWiki}<Badge variant="outline">{wiki.label(entity.template)}</Badge>{/if}
					{#if entity.subtype}
						<Badge variant="secondary">{entity.subtype}</Badge>
					{/if}
					{#if category}
						<Badge variant="outline" class="gap-1.5 text-muted-foreground">
							<span class="size-1.5 rounded-full" style="background: {category.cssVar}"></span>
							{category.label}
						</Badge>
					{/if}
					{#if ann?.scenarioSpecific}
						<Badge variant="outline" class="h-auto text-wrap">
							<Globe />
							Only in: {ann.scenarios.length
								? ann.scenarios.map(scenarioLabel).join(', ')
								: 'some scenarios'}
						</Badge>
					{/if}
					{#if ann?.dlc}
						<Badge variant="outline" class="border-warning-border">DLC</Badge>
					{/if}
					{#if ann?.factions.length}
						<Badge variant="outline" class="h-auto text-wrap">
							Factions: {ann.factions.map(factionLabel).join(', ')}
						</Badge>
					{/if}
					{#if ann?.factionAlways}
						<Badge variant="outline">Always: {factionLabel(ann.factionAlways)}</Badge>
					{/if}
					{#if ann?.requiresNation}
						<Badge variant="outline">Requires nation: {ann.requiresNation}</Badge>
					{/if}
				</Card.Content>
			</Card.Content>
		</Card.Content>

		{#if ann?.variesByScenario && !app.scope.scenario}
			<Card.Content
				class="mt-3 flex w-fit items-center gap-1.5 rounded-md border py-0.5 pr-1 pl-2 text-xs font-medium text-muted-foreground"
			>
				<Shuffle class="size-3.5 shrink-0" />
				Varies by scenario
				<Tooltip.Root>
					<Tooltip.Trigger
						class="grid size-6 place-items-center rounded-sm hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background focus-visible:outline-none"
						aria-label="Why this varies"
					>
						<Info class="size-3.5" />
					</Tooltip.Trigger>
					<Tooltip.Content class="max-w-64">
						Prerequisites or cost differ between scenarios; the values below are the all-scenarios
						overview. Pick a scenario in the toolbar for exact values.
					</Tooltip.Content>
				</Tooltip.Root>
			</Card.Content>
		{/if}
		{#if inWiki && imageUrl && !imageFailed}
			<Card.Content class="mt-4 overflow-hidden rounded-lg border bg-muted/20 p-2">
				<img
					src={imageUrl}
					alt={entity.name}
					class="max-h-60 w-full rounded-md object-contain"
					onerror={() => (imageFailed = true)}
				/>
			</Card.Content>
		{/if}
		{#if detail.effect}<Card.Content class="mt-4 p-0"
				><EffectItem line={detail.effect} /></Card.Content
			>{/if}
		{#if detail.summary}
			<Card.Content class="mt-4 px-0 text-sm/relaxed whitespace-pre-line text-muted-foreground"
				>{detail.summary}</Card.Content
			>
		{/if}
		{#if research}
			<Card.Content class="mt-4 grid grid-cols-2 gap-2 px-0">
				<Button
					variant="outline"
					size="sm"
					class="w-full"
					onclick={() => treeAction(() => app.openSubtree(entity.key, 'up'))}
				>
					<ArrowUpToLine /> Requirements tree
				</Button>
				<Tooltip.Root>
					<Tooltip.Trigger>
						{#snippet child({ props })}
							<Button
								{...props}
								variant="outline"
								size="sm"
								class="flex-1"
								aria-label="Unlocks tree: Everything this unlocks, all the way down"
								onclick={() => treeAction(() => app.openSubtree(entity.key, 'down'))}
							>
								<ArrowDownToLine /> Unlocks tree
							</Button>
						{/snippet}
					</Tooltip.Trigger>
					<Tooltip.Content>Everything this unlocks, all the way down</Tooltip.Content>
				</Tooltip.Root>
				{@render iconAction(
					'Focus',
					'Focus: direct requirements and unlocks',
					() => treeAction(() => app.openFocus(entity.key)),
					Focus
				)}
				{@render iconAction(
					'Center',
					'Center in tree',
					() => treeAction(() => app.navigate(entity)),
					Crosshair
				)}
			</Card.Content>
		{/if}
		{#if detail.researchCost != null || detail.totalCost != null}
			<Card.Content class="mt-4 grid grid-cols-2 divide-x overflow-hidden rounded-lg border px-0">
				<Card.Content class="px-3 py-2">
					<Card.Content class="px-0 text-xs text-muted-foreground">Research cost</Card.Content>
					<Card.Content
						class="px-0 text-base font-semibold tracking-tight"
						title={formatCostFull(detail.researchCost)}
					>
						{formatCost(detail.researchCost)}
					</Card.Content>
				</Card.Content>
				<Card.Content class="px-3 py-2">
					<Card.Content class="px-0 text-xs text-muted-foreground"
						>Total incl. requirements</Card.Content
					>
					<Card.Content
						class="px-0 text-base font-semibold tracking-tight"
						title={formatCostFull(detail.totalCost)}
					>
						{formatCost(detail.totalCost)}
					</Card.Content>
				</Card.Content>
			</Card.Content>
		{/if}
		{#if detail.requirements.length || detail.droppedPrereqs.length}
			{@render section(
				'Requirements',
				detail.requirements.length > 1 ? REQUIREMENT_LABELS.all : undefined
			)}
			<Card.Content class="space-y-1 px-0 text-sm">
				{#each detail.requirements as group, i (i)}
					{@const choice = requirementChoices(group) > 1}
					<Card.Content
						class="flex flex-wrap items-center gap-x-1 px-0 {choice
							? 'rounded-md border border-dashed px-2 py-1'
							: ''}"
					>
						{#if choice}<Card.Content class="w-full px-0 text-xs text-muted-foreground"
								>{REQUIREMENT_LABELS.any}</Card.Content
							>{/if}
						{#each group.options as option, j (j)}
							{@render or(j > 0)}
							<EntityLink entity={option} />
						{/each}
						{#if !group.options.length}
							<Card.Content class="inline px-0 text-xs font-medium text-destructive"
								>Not available in this scope:</Card.Content
							>
						{/if}
						{#each group.hidden as hidden, j (j)}
							{@render or(j > 0 || group.options.length > 0)}
							<Card.Content
								class="inline px-0"
								title="Exists, but can't be researched in this scope (restricted to other factions)"
							>
								<EntityLink entity={hidden} />
								<Card.Content class="inline px-0 text-xs text-muted-foreground"
									>(not in this scope)</Card.Content
								>
							</Card.Content>
						{/each}
						{#each group.unresolved as name, j (j)}
							{@render or(j > 0 || group.options.length + group.hidden.length > 0)}
							<Card.Content
								class="inline px-0 font-mono text-xs text-muted-foreground"
								title="Unresolved reference"
							>
								{name}
							</Card.Content>
						{/each}
					</Card.Content>
				{/each}
				{#each detail.droppedPrereqs as name, i (i)}
					<Card.Content
						class="px-0 font-mono text-xs text-muted-foreground line-through"
						title="Listed prerequisite that doesn't exist in this scope; the game ignores it"
					>
						{name}
					</Card.Content>
				{/each}
			</Card.Content>
		{/if}
		{#if unlockGroups.length}
			{@render section('Unlocks')}
			<Card.Content class="space-y-3 px-0">
				{#each unlockGroups as [kind, items] (kind)}
					<Card.Content class="px-0">
						<Card.Content class="mb-1 flex items-center gap-1.5 px-0 text-sm font-medium">
							{UNLOCK_LABELS[kind] ?? kind}
							<Card.Content class="inline px-0 text-xs text-muted-foreground"
								>({items.length})</Card.Content
							>
						</Card.Content>
						<Card.Content
							class="flex flex-col px-0 text-sm {items.some((u) => u.stats) ? 'gap-2' : ''}"
						>
							{#each expanded[kind] ? items : items.slice(0, UNLOCK_PREVIEW) as item, i (`${i}:${item.entity.key}`)}
								<Card.Content class="px-0">
									{#if item.stats}
										<UnlockCard entity={item.entity} stats={item.stats} />
									{:else}
										<EntityLink entity={item.entity} />
									{/if}
								</Card.Content>
							{/each}
						</Card.Content>
						{#if items.length > UNLOCK_PREVIEW}
							<Button
								variant="link"
								type="button"
								class="mt-1 text-xs text-muted-foreground underline-offset-2 hover:underline"
								onclick={() => (expanded[kind] = !expanded[kind])}
							>
								{expanded[kind] ? 'Show less' : `Show all ${items.length}`}
							</Button>
						{/if}
					</Card.Content>
				{/each}
			</Card.Content>
		{/if}
		{#if detail.unlockedBy.length}
			{@render section('Unlocked by')}
			<Card.Content class="flex flex-col px-0 text-sm">
				{#each detail.unlockedBy as ref, i (i)}
					<Card.Content class="px-0"><EntityLink entity={ref} /></Card.Content>
				{/each}
			</Card.Content>
		{/if}
		{#if detail.stats}
			{@render section('Stats')}
			<StatBlock stats={detail.stats} wide={inWiki} />
		{/if}
		{#if detail.effects.length}
			{@render section('Effects')}
			<Card.Content class="space-y-2 px-0 text-sm">
				{#each detail.effects as effect, i (i)}
					<Card.Content class="px-0"><EffectItem line={effect} /></Card.Content>
				{/each}
			</Card.Content>
		{/if}
		{#if detail.quote || (detail.description && !detail.effect) || (!inWiki && imageUrl && !imageFailed)}
			{#key entity.key}<Collapsible.Root class="mt-6" open={false}>
					<Collapsible.Trigger
						class="group flex items-center gap-1 rounded-sm py-1 text-xs font-medium text-muted-foreground hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background focus-visible:outline-none"
						><ChevronRight class="size-4 transition-transform group-data-[state=open]:rotate-90" /> Lore</Collapsible.Trigger
					>
					<Collapsible.Content class="flex flex-col gap-3 pt-3">
						{#if !inWiki && imageUrl && !imageFailed}
							<Card.Content class="overflow-hidden rounded-lg border bg-muted/20 p-2">
								<img
									src={imageUrl}
									alt={entity.name}
									class="max-h-32 w-full rounded-md object-contain"
									onerror={() => (imageFailed = true)}
								/>
							</Card.Content>
						{/if}
						{#if detail.quote}
							<Card.Content
								class="border-l-2 px-0 pl-3 text-sm whitespace-pre-line text-muted-foreground italic"
							>
								{detail.quote}
							</Card.Content>
						{/if}
						{#if detail.description && !detail.effect}
							<Card.Content class="px-0 text-sm whitespace-pre-line text-muted-foreground">
								{detail.description}
							</Card.Content>
						{/if}
					</Collapsible.Content>
				</Collapsible.Root>{/key}
		{/if}

		<ReferenceGroups title="References" groups={detail.references ?? []} exclude={shownElsewhere} />
		<ReferenceGroups
			title="Referenced by"
			groups={detail.referencedBy ?? []}
			exclude={shownElsewhere}
		/>
		{#key entity.key}<Collapsible.Root class="mt-6" open={false}>
				<Collapsible.Trigger
					class="group flex items-center gap-1 rounded-sm py-1 text-xs font-medium text-muted-foreground hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background focus-visible:outline-none"
					><ChevronRight class="size-4 transition-transform group-data-[state=open]:rotate-90" /> Technical
					details</Collapsible.Trigger
				>
				<Collapsible.Content class="flex flex-col gap-3 pt-3 text-sm">
					<p>Data name: {entity.dataName}</p>
					<p>Template: {entity.template}</p>
					<div class="flex items-center gap-2">
						<code class="break-all">{entity.key}</code><Button
							variant="outline"
							size="sm"
							onclick={() => navigator.clipboard.writeText(entity.key)}>Copy key</Button
						>
					</div>

					<ReferenceGroups title="Reference fields" groups={detail.references ?? []} technical />
					<ReferenceGroups
						title="Referenced by fields"
						groups={detail.referencedBy ?? []}
						technical
					/>
					<DataTree value={detail.record} /></Collapsible.Content
				>
			</Collapsible.Root>{/key}
	</Card.Content>
{/if}
