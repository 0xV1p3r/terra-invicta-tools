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
		r,
		width,
		height,
		obstacles,
		gatewayMatches = null,
		onpointerenter,
		onpointerleave,
		onwheel
	}: {
		item: MapItem;
		/** The item's screen position and dot radius. */
		x: number;
		y: number;
		r: number;
		width: number;
		height: number;
		/** Overlay controls to keep clear of. */
		obstacles: DOMRect[];
		/** For a body kept only because satellites below it match the filters: those matches. */
		gatewayMatches?: { bodies: number; points: number } | null;
		onpointerenter?: () => void;
		onpointerleave?: () => void;
		/** Wheel over the card, forwarded to the map. */
		onwheel?: (e: WheelEvent) => void;
	} = $props();

	const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

	const CARD_WIDTH = 248;
	const GAP = 12;
	const MARGIN = 8;
	let cardHeight = $state(0);

	const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(v, Math.max(lo, hi)));

	/**
	 * Beside, then above or below the item, clear of the item and the overlays: the candidate
	 * covering the least of them, then the closest to the item.
	 */
	const position = $derived.by(() => {
		const w = CARD_WIDTH;
		const h = cardHeight;
		const clampX = (v: number) => clamp(v, MARGIN, width - w - MARGIN);
		const clampY = (v: number) => clamp(v, MARGIN, height - h - MARGIN);
		const avoid = [...obstacles, new DOMRect(x - r, y - r, 2 * r, 2 * r)];
		const sides = [
			{ left: x + r + GAP, top: y - GAP },
			{ left: x - r - GAP - w, top: y - GAP },
			{ left: x - w / 2, top: y - r - GAP - h },
			{ left: x - w / 2, top: y + r + GAP }
		];
		// Beside the item, also try sliding the card past each overlay vertically.
		const candidates = sides.flatMap((c, i) => [
			c,
			...(i < 2
				? obstacles.flatMap((o) => [
						{ left: c.left, top: o.top - h - MARGIN },
						{ left: c.left, top: o.bottom + MARGIN }
					])
				: [])
		]);
		let best = { left: 0, top: 0 };
		let bestScore = Infinity;
		for (const c of candidates) {
			const left = clampX(c.left);
			const top = clampY(c.top);
			let overlap = 0;
			for (const o of avoid) {
				const ox = Math.min(left + w, o.right) - Math.max(left, o.left);
				const oy = Math.min(top + h, o.bottom) - Math.max(top, o.top);
				if (ox > 0 && oy > 0) overlap += ox * oy;
			}
			const dx = Math.max(left - x, 0, x - left - w);
			const dy = Math.max(top - y, 0, y - top - h);
			const score = overlap * 1000 + Math.hypot(dx, dy);
			if (score < bestScore) {
				best = { left, top };
				bestScore = score;
			}
		}
		return best;
	});

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

<!-- Hovering the card keeps it open; the map's keyboard help covers what it shows. -->
<div
	class="absolute z-10"
	style="left: {position.left}px; top: {position.top}px; width: {CARD_WIDTH}px"
	bind:clientHeight={cardHeight}
	aria-hidden="true"
	{onpointerenter}
	{onpointerleave}
	{onwheel}
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
			{#if gatewayMatches}
				<p class="font-medium">
					Doesn’t match itself — {gatewayMatches.bodies
						? plural(gatewayMatches.bodies, 'matching moon', 'matching moons')
						: plural(gatewayMatches.points, 'matching Lagrange point', 'matching Lagrange points')}
					in its system
				</p>
			{/if}
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
