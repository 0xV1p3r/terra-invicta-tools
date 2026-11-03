<script lang="ts">
	// Details of the hovered card when zoomed out (name, kind, cost, flags), next to the card.
	import { CATEGORIES } from '$lib/categories';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import { formatCostFull } from '$lib/format';
	import CategoryIcon from './CategoryIcon.svelte';
	import type { SceneNode } from './scene';

	let {
		node,
		x,
		y,
		width,
		height
	}: { node: SceneNode; x: number; y: number; width: number; height: number } = $props();

	const CARD_WIDTH = 272;
	const left = $derived(x + 8 + CARD_WIDTH > width ? Math.max(8, x - CARD_WIDTH - 8) : x + 8);
	let cardHeight = $state(0);
	const top = $derived(Math.max(8, Math.min(y, height - cardHeight - 8)));
	const category = $derived(CATEGORIES[node.item.category]);
</script>

<div
	class="pointer-events-none absolute z-10"
	style="left: {left}px; top: {top}px; width: {CARD_WIDTH}px"
	bind:clientHeight={cardHeight}
	aria-hidden="true"
>
	<Card.Root class="gap-2 py-3 shadow-lg">
		<Card.Header class="flex items-center gap-2 px-3">
			<CategoryIcon category={node.item.category} size={28} />
			<div class="min-w-0">
				<Card.Title class="text-sm leading-tight">{node.item.name}</Card.Title>
				<Card.Description class="text-xs">{category?.label} · {node.kind}</Card.Description>
			</div>
		</Card.Header>
		<Card.Content class="flex flex-col gap-1.5 px-3 text-xs">
			<div>
				<Badge variant="secondary" class="h-5">{formatCostFull(node.item.cost)} research</Badge>
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
		</Card.Content>
	</Card.Root>
</div>
