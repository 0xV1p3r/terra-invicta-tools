<script lang="ts">
	// Details of the hovered card, next to it. Zoomed out: name, kind, cost and flags, which the
	// canvas leaves out. Zoomed in, the canvas card shows those, so the hover card expands with
	// what it can't: flag labels, total cost, summary, requirements, effects and unlocks
	// (fetched lazily). A pointer-only preview: the detail panel is the accessible path.
	import type { EntityDetail, UnlockItem, UnlockKind } from '$lib/api';
	import { CATEGORIES } from '$lib/categories';
	import EntityIcon from '$lib/components/detail/EntityIcon.svelte';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import { Skeleton } from '$lib/components/ui/skeleton/index.js';
	import { SENTIMENT } from '$lib/effect-style';
	import {
		formatCostFull,
		REQUIREMENT_LABELS,
		requirementChoices,
		UNLOCK_LABELS
	} from '$lib/format';
	import { app } from '$lib/state.svelte';
	import CategoryIcon from './CategoryIcon.svelte';
	import { cachedDetail, loadDetail } from './hover-detail';
	import type { SceneNode } from './scene';

	let {
		node,
		x,
		y,
		width,
		height,
		expanded = false
	}: {
		node: SceneNode;
		x: number;
		y: number;
		width: number;
		height: number;
		expanded?: boolean;
	} = $props();

	const CARD_WIDTH = 272;
	const EXPANDED_WIDTH = 340;
	// Sweeping the mouse over the tree shouldn't fetch every card it crosses.
	const FETCH_DELAY = 150;
	const MAX_REQUIREMENTS = 5;
	const MAX_EFFECTS = 6;
	const MAX_UNLOCK_NAMES = 3;

	const cardWidth = $derived(expanded ? EXPANDED_WIDTH : CARD_WIDTH);
	const left = $derived(x + 8 + cardWidth > width ? Math.max(8, x - cardWidth - 8) : x + 8);
	let cardHeight = $state(0);
	const top = $derived(Math.max(8, Math.min(y, height - cardHeight - 8)));
	// Taller than the tree view: cut off with a fade (the card takes no pointer, so no scrolling).
	const maxHeight = $derived(Math.max(0, height - 16));
	const clipped = $derived(cardHeight >= maxHeight);
	const category = $derived(CATEGORIES[node.item.category]);

	let detail = $state.raw<EntityDetail | null>(null);
	let failed = $state(false);
	let latest = 0;

	$effect(() => {
		const key = node.key;
		const scope = $state.snapshot(app.scope);
		const id = ++latest;
		failed = false;
		if (!expanded) {
			detail = null;
			return;
		}
		const hit = cachedDetail(scope, key);
		detail = hit ?? null;
		if (hit) return;
		const timer = setTimeout(() => {
			loadDetail(scope, key).then(
				(d) => {
					if (id === latest) detail = d;
				},
				() => {
					if (id === latest) failed = true;
				}
			);
		}, FETCH_DELAY);
		return () => clearTimeout(timer);
	});

	const summary = $derived(detail?.summary ?? detail?.description ?? null);
	const showTotal = $derived(
		detail?.totalCost != null && detail.totalCost !== (detail.researchCost ?? node.item.cost)
	);
	const unlockGroups = $derived.by(() => {
		const groups: Partial<Record<UnlockKind, UnlockItem[]>> = {};
		for (const u of detail?.unlocks ?? []) (groups[u.kind] ??= []).push(u);
		return Object.entries(groups) as [UnlockKind, UnlockItem[]][];
	});
</script>

{#snippet section(title: string, hint?: string)}
	<div
		class="flex items-baseline justify-between gap-2 text-[11px] font-semibold tracking-wider text-muted-foreground uppercase"
	>
		{title}
		{#if hint}<span class="font-normal tracking-normal normal-case">{hint}</span>{/if}
	</div>
{/snippet}

{#snippet more(count: number)}
	{#if count > 0}<div class="text-muted-foreground">+{count} more</div>{/if}
{/snippet}

<div
	class="pointer-events-none absolute z-10"
	style="left: {left}px; top: {top}px; width: {cardWidth}px"
	bind:clientHeight={cardHeight}
	aria-hidden="true"
>
	<Card.Root
		class="gap-2 overflow-hidden py-3 shadow-lg {clipped
			? 'mask-b-from-[calc(100%-2.5rem)] mask-b-to-100%'
			: ''}"
		style="max-height: {maxHeight}px"
	>
		<Card.Header class="flex items-center gap-2 px-3">
			<CategoryIcon category={node.item.category} size={28} />
			<div class="min-w-0">
				<Card.Title class="text-sm leading-tight">{node.item.name}</Card.Title>
				<Card.Description class="text-xs">{category?.label} · {node.kind}</Card.Description>
			</div>
		</Card.Header>
		<Card.Content class="flex flex-col gap-1.5 px-3 text-xs">
			<div class="flex flex-wrap gap-1">
				<Badge variant="secondary" class="h-5">{formatCostFull(node.item.cost)} research</Badge>
				{#if showTotal}
					<Badge variant="outline" class="h-5">{formatCostFull(detail?.totalCost)} total</Badge>
				{/if}
			</div>
			{#each node.flags as flag (flag.key)}
				<div
					class="flex items-center gap-1.5"
					class:text-destructive={flag.inverted}
					class:text-muted-foreground={!flag.inverted}
				>
					<flag.icon class="size-3.5 shrink-0" />
					{flag.label}
				</div>
			{/each}

			{#if expanded && detail}
				{#if summary}
					<p class="line-clamp-4 whitespace-pre-line text-muted-foreground">{summary}</p>
				{/if}

				{#if detail.requirements.length}
					<Separator class="my-0.5" />
					{@render section(
						'Requirements',
						detail.requirements.length > 1 ? REQUIREMENT_LABELS.all : undefined
					)}
					{#each detail.requirements.slice(0, MAX_REQUIREMENTS) as group, i (i)}
						{@const choice = requirementChoices(group) > 1}
						<div
							class="flex flex-wrap items-center gap-x-1 gap-y-0.5 {choice
								? 'rounded-md border border-dashed px-1.5 py-1'
								: ''}"
						>
							{#if choice}
								<span class="w-full text-muted-foreground">{REQUIREMENT_LABELS.any}</span>
							{/if}
							{#each group.options as option, j (j)}
								{#if j > 0}<span class="text-muted-foreground">or</span>{/if}
								<span class="inline-flex min-w-0 items-center gap-1 font-medium">
									<EntityIcon entity={option} size={14} />
									<span class="truncate">{option.name}</span>
								</span>
							{/each}
							{#each group.hidden as hidden, j (j)}
								{#if j > 0 || group.options.length}<span class="text-muted-foreground">or</span
									>{/if}
								<span class="inline-flex min-w-0 items-center gap-1 text-muted-foreground">
									<EntityIcon entity={hidden} size={14} />
									<span class="truncate">{hidden.name}</span>
								</span>
							{/each}
						</div>
					{/each}
					{@render more(detail.requirements.length - MAX_REQUIREMENTS)}
				{/if}

				{#if detail.effects.length}
					<Separator class="my-0.5" />
					{@render section('Effects')}
					{#each detail.effects.slice(0, MAX_EFFECTS) as effect, i (i)}
						{@const tone = SENTIMENT[effect.sentiment] ?? SENTIMENT.neutral}
						<div class="flex items-start justify-between gap-2">
							<span class="line-clamp-2 min-w-0">
								{effect.label ?? effect.text ?? effect.entity.name}
							</span>
							{#if effect.modifier}
								<Badge variant="outline" class="h-5 shrink-0 font-mono tabular-nums {tone.badge}">
									{effect.modifier}
								</Badge>
							{/if}
						</div>
					{/each}
					{@render more(detail.effects.length - MAX_EFFECTS)}
				{/if}

				{#if unlockGroups.length}
					<Separator class="my-0.5" />
					{@render section('Unlocks')}
					{#each unlockGroups as [kind, items] (kind)}
						<div class="min-w-0">
							<span class="font-medium">{UNLOCK_LABELS[kind] ?? kind}</span>
							<span class="text-muted-foreground">({items.length})</span>
							<div class="truncate text-muted-foreground">
								{items
									.slice(0, MAX_UNLOCK_NAMES)
									.map((u) => u.entity.name)
									.join(', ')}{items.length > MAX_UNLOCK_NAMES
									? `, +${items.length - MAX_UNLOCK_NAMES} more`
									: ''}
							</div>
						</div>
					{/each}
				{/if}
			{:else if expanded && !failed}
				<div class="flex flex-col gap-1.5 pt-1">
					<Skeleton class="h-3 w-full" />
					<Skeleton class="h-3 w-5/6" />
					<Skeleton class="h-3 w-2/3" />
				</div>
			{/if}
		</Card.Content>
	</Card.Root>
</div>
