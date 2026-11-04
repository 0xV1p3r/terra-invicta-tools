<script lang="ts">
	import ArrowDownToLine from '@lucide/svelte/icons/arrow-down-to-line';
	import ArrowUpToLine from '@lucide/svelte/icons/arrow-up-to-line';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Crosshair from '@lucide/svelte/icons/crosshair';
	import Focus from '@lucide/svelte/icons/focus';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

	import {
		api,
		assetUrl,
		type EntityKey,
		type EntityDetail,
		type UnlockItem,
		type UnlockKind
	} from '$lib/api';
	import * as Card from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Collapsible from '$lib/components/ui/collapsible/index.js';

	import { Skeleton } from '$lib/components/ui/skeleton/index.js';
	import { categoryInfo } from '$lib/categories';
	import { formatCost, formatCostFull, isResearch, KIND_LABELS } from '$lib/format';
	import { app } from '$lib/state.svelte';
	import EffectItem from './EffectItem.svelte';
	import EntityIcon from './EntityIcon.svelte';
	import EntityLink from './EntityLink.svelte';
	import StatBlock from './StatBlock.svelte';
	import UnlockCard from './UnlockCard.svelte';

	const UNLOCK_LABELS: Record<UnlockKind, string> = {
		research: 'Research',
		researchAlternative: 'Research (alternative prerequisite)',
		shipPart: 'Ship parts',
		habModule: 'Hab modules',
		org: 'Orgs',
		grantedOrg: 'Granted orgs',
		claim: 'Claims',
		trait: 'Traits',
		codex: 'Codex entries',
		mission: 'Missions',
		objective: 'Objectives'
	};

	let detail = $state.raw<EntityDetail | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let latest = 0;

	$effect(() => {
		const selectedKey = entityKey;
		const scope = $state.snapshot(app.scope);
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
	// Unlock cards with stats start expanded only in small groups (drive families have up to 6 variants).
	const OPEN_CARDS = 6;
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
	const hasAnnotations = $derived(
		!!ann &&
			(ann.scenarioSpecific ||
				ann.variesByScenario ||
				ann.dlc ||
				ann.factions.length > 0 ||
				!!ann.factionAlways ||
				!!ann.requiresNation)
	);

	function scenarioLabel(name: string) {
		return app.meta?.scenarios.find((s) => s.name === name)?.label ?? name;
	}
	function factionLabel(name: string) {
		return app.meta?.factions.find((f) => f.name === name)?.label ?? name;
	}

	import { wikiIcon } from '$lib/wiki-icons';
	import { wiki } from '$lib/wiki.svelte';
	import { settings } from '$lib/settings.svelte';
	import SpoilerChip from './SpoilerChip.svelte';
	import DataTree from './DataTree.svelte';
	import ReferenceGroups from './ReferenceGroups.svelte';
	let { entityKey, inWiki = false }: { entityKey: EntityKey | null; inWiki?: boolean } = $props();
	function treeAction(action: () => void) {
		if (inWiki) wiki.open = false;
		action();
	}
</script>

{#snippet section(title: string)}
	<Card.Content
		class="mt-5 mb-2 px-0 text-xs font-semibold tracking-wider text-muted-foreground uppercase"
	>
		{title}
	</Card.Content>
{/snippet}

{#if !entityKey}
	<Card.Content class="px-0 text-sm text-muted-foreground">
		Select a node in the tree or search (Ctrl+K) to see its details. Double-click a node to show its
		requirements tree.
	</Card.Content>
{:else if error}
	<Card.Content class="flex items-start gap-3 px-0" role="alert">
		<TriangleAlert class="mt-0.5 size-5 shrink-0 text-destructive" />
		<Card.Content class="px-0 text-sm">
			<Card.Content class="px-0 font-medium">Could not load {entityKey}</Card.Content>
			<Card.Content class="mt-1 px-0 break-words text-muted-foreground">{error}</Card.Content>
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
					<Badge>{KIND_LABELS[entity.kind] ?? entity.kind}</Badge>
					{#if inWiki}<Badge variant="outline">{wiki.label(entity.template)}</Badge>{/if}
					{#if entity.subtype}
						<Badge variant="secondary">{entity.subtype}</Badge>
					{/if}
					{#if category}
						<Badge
							variant="outline"
							style="border-color: {category.cssVar}; color: {category.cssVar}"
						>
							{category.label}
						</Badge>
					{/if}
				</Card.Content>
			</Card.Content>
		</Card.Content>

		{#if imageUrl && !imageFailed}
			<Card.Content class="mt-4 overflow-hidden rounded-lg border bg-muted/20 p-2">
				<img
					src={imageUrl}
					alt={entity.name}
					class="w-full rounded-md object-contain {inWiki ? 'max-h-60' : 'max-h-32'}"
					onerror={() => (imageFailed = true)}
				/>
			</Card.Content>
		{/if}
		{#if detail.effect}<Card.Content class="mt-4 p-0"
				><EffectItem line={detail.effect} /></Card.Content
			>{/if}
		{#if research}
			<Card.Content class="mt-4 flex gap-2 px-0">
				<Button
					variant="outline"
					size="sm"
					class="flex-1"
					onclick={() => treeAction(() => app.openSubtree(entity.key, 'up'))}
				>
					<ArrowUpToLine /> Requirements tree
				</Button>
				<Button
					variant="outline"
					size="sm"
					class="flex-1"
					onclick={() => treeAction(() => app.openSubtree(entity.key, 'down'))}
				>
					<ArrowDownToLine /> Dependents
				</Button>
				<Button
					variant="outline"
					size="icon"
					class="size-8"
					aria-label="Focus view"
					title="Focus view: direct requirements and unlocks"
					onclick={() => treeAction(() => app.openFocus(entity.key))}
				>
					<Focus />
				</Button>
				<Button
					variant="outline"
					size="icon"
					class="size-8"
					aria-label="Center in tree"
					title="Center in tree"
					onclick={() => treeAction(() => app.navigate(entity))}
				>
					<Crosshair />
				</Button>
			</Card.Content>
		{/if}

		{#if detail.researchCost != null || detail.totalCost != null}
			<Card.Content class="mt-4 grid grid-cols-2 gap-2 px-0">
				<Card.Content class="rounded-lg bg-muted/50 p-2 px-0">
					<Card.Content class="px-0 text-xs text-muted-foreground">Research cost</Card.Content>
					<Card.Content class="px-0 font-semibold" title={formatCostFull(detail.researchCost)}>
						{formatCost(detail.researchCost)}
					</Card.Content>
				</Card.Content>
				<Card.Content class="rounded-lg bg-muted/50 p-2 px-0">
					<Card.Content class="px-0 text-xs text-muted-foreground"
						>Total incl. requirements</Card.Content
					>
					<Card.Content class="px-0 font-semibold" title={formatCostFull(detail.totalCost)}>
						{formatCost(detail.totalCost)}
					</Card.Content>
				</Card.Content>
			</Card.Content>
		{/if}

		{#if detail.quote}
			<Card.Content
				class="mt-4 border-l-2 px-0 pl-3 text-sm whitespace-pre-line text-muted-foreground italic"
			>
				{detail.quote}
			</Card.Content>
		{/if}
		{#if detail.summary}
			<Card.Content class="mt-4 px-0 text-sm whitespace-pre-line">{detail.summary}</Card.Content>
		{/if}
		{#if detail.description && !detail.effect}
			<Card.Content class="mt-3 px-0 text-sm whitespace-pre-line text-muted-foreground">
				{detail.description}
			</Card.Content>
		{/if}

		{#if detail.requirements.length || detail.droppedPrereqs.length}
			{@render section('Requirements')}
			<Card.Content class="space-y-1 px-0 text-sm">
				{#each detail.requirements as group, i (i)}
					<Card.Content class="flex flex-wrap items-center gap-x-1 px-0">
						{#each group.options as option, j (j)}
							{#if j > 0}<Card.Content class="inline px-0 text-xs text-muted-foreground"
									>or</Card.Content
								>{/if}
							<EntityLink entity={option} />
							{#if group.alternative[j]}
								<Card.Content
									class="inline px-0 text-xs text-muted-foreground"
									title="Alternative prerequisite (altPrereq)"
								>
									(alternative)
								</Card.Content>
							{/if}
						{/each}
						{#if !group.options.length}
							<Card.Content class="inline px-0 text-xs font-medium text-destructive"
								>Not available in this scope:</Card.Content
							>
						{/if}
						{#each group.hidden as hidden, j (j)}
							<Card.Content
								class="inline px-0 opacity-60"
								title="Exists, but can't be researched in this scope (restricted to other factions)"
							>
								<EntityLink entity={hidden} />
							</Card.Content>
						{/each}
						{#each group.unresolved as name, j (j)}
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
										<UnlockCard
											entity={item.entity}
											stats={item.stats}
											open={items.length <= OPEN_CARDS}
										/>
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

		<ReferenceGroups title="References" groups={detail.references ?? []} />
		<ReferenceGroups title="Referenced by" groups={detail.referencedBy ?? []} />
		{#key entity.key}<Collapsible.Root class="mt-6" open={false}>
				<Collapsible.Trigger><ChevronRight class="size-4" /> Technical details</Collapsible.Trigger>
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

					{#if ann && hasAnnotations}
						<Card.Content class="mt-3 flex flex-wrap gap-1 px-0">
							{#if ann.scenarioSpecific}
								<Badge
									variant="outline"
									class="h-auto text-wrap"
									title={ann.scenarios.map(scenarioLabel).join(', ')}
								>
									Scenarios: {ann.scenarios.length
										? ann.scenarios.map(scenarioLabel).join(', ')
										: 'specific'}
								</Badge>
							{/if}
							{#if ann.variesByScenario}
								<Badge variant="outline">Varies by scenario</Badge>
							{/if}
							{#if ann.dlc}
								<Badge
									variant="outline"
									class="border-amber-500/60 text-amber-600 dark:text-amber-400"
								>
									DLC
								</Badge>
							{/if}
							{#if ann.factions.length}
								<Badge variant="outline" class="h-auto text-wrap">
									Factions: {ann.factions.map(factionLabel).join(', ')}
								</Badge>
							{/if}
							{#if ann.factionAlways}
								<Badge variant="outline">Always: {factionLabel(ann.factionAlways)}</Badge>
							{/if}
							{#if ann.requiresNation}
								<Badge variant="outline">Requires nation: {ann.requiresNation}</Badge>
							{/if}
						</Card.Content>
					{/if}
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
