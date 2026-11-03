<script lang="ts">
	import ArrowDownToLine from '@lucide/svelte/icons/arrow-down-to-line';
	import ArrowUpToLine from '@lucide/svelte/icons/arrow-up-to-line';
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import Crosshair from '@lucide/svelte/icons/crosshair';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import X from '@lucide/svelte/icons/x';
	import { api, type EntityDetail, type UnlockItem, type UnlockKind } from '$lib/api';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Collapsible from '$lib/components/ui/collapsible/index.js';
	import { ScrollArea } from '$lib/components/ui/scroll-area/index.js';
	import { Skeleton } from '$lib/components/ui/skeleton/index.js';
	import { categoryInfo } from '$lib/categories';
	import { formatCost, formatCostFull, isResearch, KIND_LABELS } from '$lib/format';
	import { app } from '$lib/state.svelte';
	import EffectItem from './EffectItem.svelte';
	import EntityIcon from './EntityIcon.svelte';
	import EntityLink from './EntityLink.svelte';

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
		const key = app.selected;
		const scope = $state.snapshot(app.scope);
		const id = ++latest;
		error = null;
		if (!key) {
			detail = null;
			loading = false;
			return;
		}
		loading = true;
		api.getEntity(scope, key).then(
			(d) => {
				if (id !== latest) return;
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

	const entity = $derived(detail?.entity ?? null);
	const research = $derived(entity ? isResearch(entity.kind) : false);
	const category = $derived(categoryInfo(entity?.category));

	// Some projects unlock well over a thousand items (e.g. territorial claims).
	const UNLOCK_PREVIEW = 25;
	let expanded = $state<Partial<Record<UnlockKind, boolean>>>({});
	$effect(() => {
		void app.selected;
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
</script>

{#snippet section(title: string)}
	<h3 class="mt-5 mb-2 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
		{title}
	</h3>
{/snippet}

<aside class="flex h-full min-h-0 flex-col border-l bg-card" aria-label="Details">
	<div class="flex items-center justify-between gap-2 border-b px-4 py-2">
		<span class="text-sm font-medium text-muted-foreground">Details</span>
		<Button
			variant="ghost"
			size="icon"
			class="size-7"
			aria-label="Close details"
			onclick={() => (app.panelOpen = false)}
		>
			<X />
		</Button>
	</div>

	<ScrollArea class="min-h-0 flex-1">
		<div class="p-4">
			{#if !app.selected}
				<p class="text-sm text-muted-foreground">
					Select a node in the tree or search (Ctrl+K) to see its details. Double-click a node to
					show its requirements tree.
				</p>
			{:else if error}
				<div class="flex items-start gap-3" role="alert">
					<TriangleAlert class="mt-0.5 size-5 shrink-0 text-destructive" />
					<div class="text-sm">
						<p class="font-medium">Could not load {app.selected}</p>
						<p class="mt-1 break-words text-muted-foreground">{error}</p>
					</div>
				</div>
			{:else if loading && !detail}
				<div class="space-y-3" aria-busy="true" aria-label="Loading details">
					<div class="flex items-center gap-3">
						<Skeleton class="size-14 rounded-lg" />
						<div class="flex-1 space-y-2">
							<Skeleton class="h-5 w-3/4" />
							<Skeleton class="h-4 w-1/2" />
						</div>
					</div>
					<Skeleton class="h-16 w-full" />
					<Skeleton class="h-4 w-full" />
					<Skeleton class="h-4 w-5/6" />
					<Skeleton class="h-4 w-2/3" />
				</div>
			{:else if detail && entity}
				<article class="transition-opacity" class:opacity-60={loading}>
					<header class="flex items-start gap-3">
						<div
							class="grid size-14 shrink-0 place-items-center rounded-lg border bg-muted/50"
							style:border-color={category?.cssVar}
						>
							<EntityIcon {entity} size={44} />
						</div>
						<div class="min-w-0 flex-1">
							<h2 class="text-lg leading-tight font-semibold break-words">{entity.name}</h2>
							<p class="mt-0.5 font-mono text-xs break-all text-muted-foreground">
								{entity.dataName}
							</p>
							<div class="mt-2 flex flex-wrap gap-1">
								<Badge>{KIND_LABELS[entity.kind] ?? entity.kind}</Badge>
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
							</div>
						</div>
					</header>

					{#if research}
						<div class="mt-4 flex gap-2">
							<Button
								variant="outline"
								size="sm"
								class="flex-1"
								onclick={() => app.openSubtree(entity.key, 'up')}
							>
								<ArrowUpToLine /> Requirements tree
							</Button>
							<Button
								variant="outline"
								size="sm"
								class="flex-1"
								onclick={() => app.openSubtree(entity.key, 'down')}
							>
								<ArrowDownToLine /> Dependents
							</Button>
							<Button
								variant="outline"
								size="icon"
								class="size-8"
								aria-label="Center in tree"
								title="Center in tree"
								onclick={() => app.navigate(entity)}
							>
								<Crosshair />
							</Button>
						</div>
					{/if}

					{#if detail.researchCost != null || detail.totalCost != null}
						<dl class="mt-4 grid grid-cols-2 gap-2">
							<div class="rounded-lg bg-muted/50 p-2">
								<dt class="text-xs text-muted-foreground">Research cost</dt>
								<dd class="font-semibold" title={formatCostFull(detail.researchCost)}>
									{formatCost(detail.researchCost)}
								</dd>
							</div>
							<div class="rounded-lg bg-muted/50 p-2">
								<dt class="text-xs text-muted-foreground">Total incl. requirements</dt>
								<dd class="font-semibold" title={formatCostFull(detail.totalCost)}>
									{formatCost(detail.totalCost)}
								</dd>
							</div>
						</dl>
					{/if}

					{#if ann && hasAnnotations}
						<div class="mt-3 flex flex-wrap gap-1">
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
						</div>
					{/if}

					{#if detail.quote}
						<blockquote
							class="mt-4 border-l-2 pl-3 text-sm whitespace-pre-line text-muted-foreground italic"
						>
							{detail.quote}
						</blockquote>
					{/if}
					{#if detail.summary}
						<p class="mt-4 text-sm whitespace-pre-line">{detail.summary}</p>
					{/if}
					{#if detail.description}
						<p class="mt-3 text-sm whitespace-pre-line text-muted-foreground">
							{detail.description}
						</p>
					{/if}

					{#if detail.requirements.length || detail.droppedPrereqs.length}
						{@render section('Requirements')}
						<ul class="space-y-1 text-sm">
							{#each detail.requirements as group, i (i)}
								<li class="flex flex-wrap items-center gap-x-1">
									{#each group.options as option, j (j)}
										{#if j > 0}<span class="text-xs text-muted-foreground">or</span>{/if}
										<EntityLink entity={option} />
										{#if group.alternative[j]}
											<span
												class="text-xs text-muted-foreground"
												title="Alternative prerequisite (altPrereq)"
											>
												(alternative)
											</span>
										{/if}
									{/each}
									{#if !group.options.length}
										<span class="text-xs font-medium text-destructive"
											>Not available in this scope:</span
										>
									{/if}
									{#each group.hidden as hidden, j (j)}
										<span
											class="opacity-60"
											title="Exists, but can't be researched in this scope (restricted to other factions)"
										>
											<EntityLink entity={hidden} />
										</span>
									{/each}
									{#each group.unresolved as name, j (j)}
										<span
											class="font-mono text-xs text-muted-foreground"
											title="Unresolved reference"
										>
											{name}
										</span>
									{/each}
								</li>
							{/each}
							{#each detail.droppedPrereqs as name, i (i)}
								<li
									class="font-mono text-xs text-muted-foreground line-through"
									title="Listed prerequisite that doesn't exist in this scope; the game ignores it"
								>
									{name}
								</li>
							{/each}
						</ul>
					{/if}

					{#if detail.effects.length}
						{@render section('Effects')}
						<ul class="space-y-2 text-sm">
							{#each detail.effects as effect, i (i)}
								<li><EffectItem {effect} /></li>
							{/each}
						</ul>
					{/if}

					{#if unlockGroups.length}
						{@render section('Unlocks')}
						<div class="space-y-3">
							{#each unlockGroups as [kind, items] (kind)}
								<div>
									<h4 class="mb-1 flex items-center gap-1.5 text-sm font-medium">
										{UNLOCK_LABELS[kind] ?? kind}
										<span class="text-xs text-muted-foreground">({items.length})</span>
									</h4>
									<ul class="flex flex-col text-sm">
										{#each expanded[kind] ? items : items.slice(0, UNLOCK_PREVIEW) as item, i (i)}
											<li><EntityLink entity={item.entity} /></li>
										{/each}
									</ul>
									{#if items.length > UNLOCK_PREVIEW}
										<button
											type="button"
											class="mt-1 text-xs text-muted-foreground underline-offset-2 hover:underline"
											onclick={() => (expanded[kind] = !expanded[kind])}
										>
											{expanded[kind] ? 'Show less' : `Show all ${items.length}`}
										</button>
									{/if}
								</div>
							{/each}
						</div>
					{/if}

					{#if detail.unlockedBy.length}
						{@render section('Unlocked by')}
						<ul class="flex flex-col text-sm">
							{#each detail.unlockedBy as ref, i (i)}
								<li><EntityLink entity={ref} /></li>
							{/each}
						</ul>
					{/if}

					<Collapsible.Root class="mt-6">
						<Collapsible.Trigger
							class="group flex items-center gap-1 text-xs font-semibold tracking-wider text-muted-foreground uppercase hover:text-foreground"
						>
							<ChevronRight class="size-4 transition-transform group-data-[state=open]:rotate-90" />
							Raw record
						</Collapsible.Trigger>
						<Collapsible.Content>
							<pre
								class="mt-2 max-h-96 overflow-auto rounded-lg bg-muted/50 p-3 font-mono text-xs">{JSON.stringify(
									detail.record,
									null,
									2
								)}</pre>
						</Collapsible.Content>
					</Collapsible.Root>
				</article>
			{/if}
		</div>
	</ScrollArea>
</aside>
