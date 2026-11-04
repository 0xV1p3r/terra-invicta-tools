<script lang="ts" module>
	import type { Component } from 'svelte';
	import Atom from '@lucide/svelte/icons/atom';
	import Droplets from '@lucide/svelte/icons/droplets';
	import Gem from '@lucide/svelte/icons/gem';
	import Pickaxe from '@lucide/svelte/icons/pickaxe';
	import Radiation from '@lucide/svelte/icons/radiation';
	import SatelliteDish from '@lucide/svelte/icons/satellite-dish';
	import Wind from '@lucide/svelte/icons/wind';

	/** Inline sprites of effect texts (TextMeshPro sprite names): readable names and icons
	 * for when the game image isn't available (mock mode). */
	const SPRITES: Record<string, { label: string; icon: Component }> = {
		water: { label: 'Water', icon: Droplets },
		volatiles: { label: 'Volatiles', icon: Wind },
		metal: { label: 'Metals', icon: Pickaxe },
		metal_noble: { label: 'Noble metals', icon: Gem },
		radioactive: { label: 'Fissiles', icon: Radiation },
		antimatter: { label: 'Antimatter', icon: Atom },
		exotics: { label: 'Exotics', icon: Atom },
		mission_control: { label: 'Mission Control', icon: SatelliteDish }
	};
</script>

<script lang="ts">
	import { assetUrl, type EffectLine } from '$lib/api';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import CategoryIcon from '$lib/components/tree/CategoryIcon.svelte';
	import { DOMAIN_ICONS, SENTIMENT } from '$lib/effect-style';
	import { app } from '$lib/state.svelte';
	import Sparkles from '@lucide/svelte/icons/sparkles';

	let { line }: { line: EffectLine } = $props();

	const DomainIcon = $derived(DOMAIN_ICONS[line.domain] ?? Sparkles);
	const tone = $derived(SENTIMENT[line.sentiment] ?? SENTIMENT.neutral);
	const iconUrl = $derived(assetUrl(line.icon));
	let iconFailed = $state(false);
	$effect(() => {
		void iconUrl;
		iconFailed = false;
	});
	const gameIcon = $derived(!line.category && !!iconUrl && !iconFailed);
	// Without a label the description itself is the headline.
	const title = $derived(line.label ?? (line.segments.length ? null : line.entity.name));
</script>

{#snippet description(segments: EffectLine['segments'])}
	{#each segments as seg, i (i)}
		{#if seg.kind === 'value'}
			<strong class="font-semibold text-foreground tabular-nums">{seg.text}</strong>
		{:else if seg.kind === 'sprite'}
			{@const sprite = SPRITES[seg.text]}
			{@const url = assetUrl(seg.icon)}
			<span
				class="inline-flex items-center gap-0.5 align-baseline font-medium text-foreground"
				title={sprite?.label ?? seg.text.replaceAll('_', ' ')}
			>
				{#if url}
					<img
						src={url}
						alt=""
						class="size-4 self-center rounded-sm bg-zinc-800 object-contain p-px"
						draggable="false"
					/>
				{:else if sprite}
					<sprite.icon class="size-3.5 self-center" aria-hidden="true" />
				{/if}
				{sprite?.label ?? seg.text.replaceAll('_', ' ')}
			</span>
		{:else}{seg.text}{/if}
	{/each}
{/snippet}

<div class="flex items-start gap-3 rounded-lg border bg-muted/20 p-2.5">
	<div
		class="grid size-9 shrink-0 place-items-center rounded-md {gameIcon
			? `bg-zinc-800 ring-1 ${tone.ring}`
			: tone.tile}"
		title={tone.label || undefined}
	>
		{#if line.category}
			<CategoryIcon category={line.category} size={26} />
		{:else if gameIcon}
			<img
				src={iconUrl}
				alt=""
				class="size-7 object-contain"
				draggable="false"
				onerror={() => (iconFailed = true)}
			/>
		{:else}
			<DomainIcon class="size-[18px]" aria-hidden="true" />
		{/if}
	</div>
	<div class="min-w-0 flex-1">
		<div class="flex items-start justify-between gap-2">
			<p class="leading-snug font-medium break-words {title ? '' : 'whitespace-pre-line'}">
				{#if title}{title}{:else}{@render description(line.segments)}{/if}
			</p>
			{#if line.modifier}
				<Badge
					variant="outline"
					class="shrink-0 font-mono text-sm font-semibold tabular-nums {tone.badge}"
					title={tone.label || undefined}
				>
					{line.modifier}
				</Badge>
			{/if}
		</div>
		{#if title && line.segments.length}
			<p class="mt-0.5 text-xs/relaxed whitespace-pre-line text-muted-foreground">
				{@render description(line.segments)}
			</p>
		{/if}
		<div class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1">
			{#if line.target}
				<Badge variant="secondary" class="h-5 px-1.5 text-[11px]">{line.target}</Badge>
			{/if}
			<button
				type="button"
				class="truncate font-mono text-[11px] text-muted-foreground/70 underline-offset-2 hover:text-foreground hover:underline focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none"
				title="Open effect record"
				onclick={() => app.navigate(line.entity)}
			>
				{line.entity.dataName}
			</button>
		</div>
	</div>
</div>
