<script lang="ts">
	// Summary of the hovered body or Lagrange point, next to it.
	import { Badge } from '$lib/components/ui/badge/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import { RESOURCES } from './map-state.svelte';
	import { TYPE_LABELS, type MapItem } from './scene';

	let {
		item,
		x,
		y,
		width,
		height
	}: { item: MapItem; x: number; y: number; width: number; height: number } = $props();

	const CARD_WIDTH = 248;
	const left = $derived(x + 12 + CARD_WIDTH > width ? Math.max(8, x - CARD_WIDTH - 12) : x + 12);
	let cardHeight = $state(0);
	const top = $derived(Math.max(8, Math.min(y - 12, height - cardHeight - 8)));

	const AU_KM = 149_597_870.7;
	const info = $derived(item.point ? null : (item.body?.info ?? null));
	const parent = $derived(item.body?.parent?.info ?? null);

	function distance(km: number, parentIsStar: boolean) {
		if (parentIsStar) return `${(km / AU_KM).toFixed(km < 10 * AU_KM ? 3 : 1)} AU`;
		return `${Math.round(km).toLocaleString('en-US')} km`;
	}

	function period(days: number) {
		if (days < 2) return `${(days * 24).toFixed(1)} h`;
		if (days < 700) return `${days.toFixed(days < 20 ? 2 : 0)} days`;
		return `${(days / 365.25).toFixed(days < 36_525 ? 2 : 0)} years`;
	}

	const RATING_VARIANT = {
		empty: 'outline',
		possible: 'outline',
		low: 'secondary',
		medium: 'secondary',
		high: 'default',
		max: 'default'
	} as const;
</script>

<div
	class="pointer-events-none absolute z-10"
	style="left: {left}px; top: {top}px; width: {CARD_WIDTH}px"
	bind:clientHeight={cardHeight}
	aria-hidden="true"
>
	<Card.Root class="gap-2 py-3 shadow-lg">
		<Card.Header class="flex items-center gap-2 px-3">
			<span class="size-3 shrink-0 rounded-full" style="background: {item.color}"></span>
			<div class="min-w-0">
				<Card.Title class="text-sm leading-tight">{item.name}</Card.Title>
				<Card.Description class="text-xs">
					{TYPE_LABELS[item.type]}
					{#if item.point && item.body}
						· {item.point.point} of {item.body.parent?.info.entity.name ?? '?'}–{item.body.info
							.entity.name}
					{:else if parent}
						· orbits {parent.entity.name}
					{/if}
				</Card.Description>
			</div>
		</Card.Header>
		<Card.Content class="grid gap-1.5 px-3 text-xs">
			{#if info}
				<dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5">
					<dt class="text-muted-foreground">Radius</dt>
					<dd>{Math.round(info.radiusKm).toLocaleString('en-US')} km</dd>
					{#if info.orbit}
						<dt class="text-muted-foreground">Orbit</dt>
						<dd>
							{distance(info.orbit.semiMajorAxisKm, parent?.bodyType === 'star')}, e {info.orbit.eccentricity.toFixed(
								3
							)}
						</dd>
						<dt class="text-muted-foreground">Period</dt>
						<dd>{period(info.orbit.periodDays)}</dd>
					{/if}
					<dt class="text-muted-foreground">Hab sites</dt>
					<dd>{info.habSiteCount}</dd>
					{#if info.maxHabSize > 0}
						<dt class="text-muted-foreground">Max hab size</dt>
						<dd>{info.maxHabSize}</dd>
					{/if}
				</dl>
				{#if Object.keys(info.resources).length}
					<div class="flex flex-wrap gap-1">
						{#each RESOURCES as res (res)}
							{@const rating = info.resources[res]}
							{#if rating && rating !== 'empty'}
								<Badge variant={RATING_VARIANT[rating]} class="h-5 capitalize"
									>{res}: {rating}</Badge
								>
							{/if}
						{/each}
					</div>
				{/if}
				{#if item.body && item.body.children.length}
					<p class="text-muted-foreground">
						{item.body.children.length} satellite{item.body.children.length === 1 ? '' : 's'} · double-click
						to enter
					</p>
				{/if}
			{:else if item.point}
				<p class="text-muted-foreground">Max hab size {item.point.maxHabSize}</p>
			{/if}
		</Card.Content>
	</Card.Root>
</div>
