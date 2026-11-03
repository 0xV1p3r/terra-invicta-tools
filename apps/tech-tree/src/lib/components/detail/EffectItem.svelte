<script lang="ts" module>
	import type { Component } from 'svelte';
	import type { EffectDomain, EffectSentiment } from '$lib/api';
	import Atom from '@lucide/svelte/icons/atom';
	import Award from '@lucide/svelte/icons/award';
	import Bug from '@lucide/svelte/icons/bug';
	import Building2 from '@lucide/svelte/icons/building-2';
	import Coins from '@lucide/svelte/icons/coins';
	import Crosshair from '@lucide/svelte/icons/crosshair';
	import Crown from '@lucide/svelte/icons/crown';
	import Droplets from '@lucide/svelte/icons/droplets';
	import FlaskConical from '@lucide/svelte/icons/flask-conical';
	import Gem from '@lucide/svelte/icons/gem';
	import Landmark from '@lucide/svelte/icons/landmark';
	import Leaf from '@lucide/svelte/icons/leaf';
	import Megaphone from '@lucide/svelte/icons/megaphone';
	import Orbit from '@lucide/svelte/icons/orbit';
	import Pickaxe from '@lucide/svelte/icons/pickaxe';
	import Radiation from '@lucide/svelte/icons/radiation';
	import Rocket from '@lucide/svelte/icons/rocket';
	import SatelliteDish from '@lucide/svelte/icons/satellite-dish';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import Swords from '@lucide/svelte/icons/swords';
	import Telescope from '@lucide/svelte/icons/telescope';
	import UserRound from '@lucide/svelte/icons/user-round';
	import Wind from '@lucide/svelte/icons/wind';

	const DOMAIN_ICONS: Record<EffectDomain, Component> = {
		research: FlaskConical,
		priority: Landmark,
		mission: Crosshair,
		control: Crown,
		councilor: UserRound,
		army: Swords,
		ship: Rocket,
		mining: Pickaxe,
		missionControl: SatelliteDish,
		exploration: Telescope,
		transfer: Orbit,
		hab: Building2,
		alien: Bug,
		environment: Leaf,
		economy: Coins,
		politics: Megaphone,
		ability: Award,
		other: Sparkles
	};

	/** Inline sprites of effect texts (TextMeshPro sprite names), shown in place of a word. */
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

	const SENTIMENT: Record<EffectSentiment, { tile: string; badge: string; label: string }> = {
		good: {
			tile: 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-400',
			badge: 'border-emerald-500/40 bg-emerald-500/15 text-emerald-700 dark:text-emerald-300',
			label: 'Beneficial'
		},
		bad: {
			tile: 'bg-rose-500/15 text-rose-700 dark:text-rose-400',
			badge: 'border-rose-500/40 bg-rose-500/15 text-rose-700 dark:text-rose-300',
			label: 'Detrimental'
		},
		neutral: {
			tile: 'bg-muted text-muted-foreground',
			badge: 'border-border bg-muted text-foreground',
			label: ''
		}
	};
</script>

<script lang="ts">
	import type { EffectLine } from '$lib/api';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import CategoryIcon from '$lib/components/tree/CategoryIcon.svelte';
	import { app } from '$lib/state.svelte';

	let { effect }: { effect: EffectLine } = $props();

	const DomainIcon = $derived(DOMAIN_ICONS[effect.domain] ?? Sparkles);
	const tone = $derived(SENTIMENT[effect.sentiment] ?? SENTIMENT.neutral);
	// Without a label the description itself is the headline.
	const title = $derived(effect.label ?? (effect.segments.length ? null : effect.entity.name));
</script>

{#snippet description(segments: EffectLine['segments'])}
	{#each segments as seg, i (i)}
		{#if seg.kind === 'value'}
			<strong class="font-semibold text-foreground tabular-nums">{seg.text}</strong>
		{:else if seg.kind === 'sprite'}
			{@const sprite = SPRITES[seg.text]}
			<span
				class="inline-flex items-center gap-0.5 align-baseline font-medium text-foreground"
				title={sprite?.label ?? seg.text}
			>
				{#if sprite}<sprite.icon class="size-3.5 self-center" aria-hidden="true" />{/if}
				{sprite?.label ?? seg.text}
			</span>
		{:else}{seg.text}{/if}
	{/each}
{/snippet}

<div class="flex items-start gap-3 rounded-lg border bg-muted/20 p-2.5">
	<div
		class="grid size-9 shrink-0 place-items-center rounded-md {tone.tile}"
		title={tone.label || undefined}
	>
		{#if effect.category}
			<CategoryIcon category={effect.category} size={26} />
		{:else}
			<DomainIcon class="size-[18px]" aria-hidden="true" />
		{/if}
	</div>
	<div class="min-w-0 flex-1">
		<div class="flex items-start justify-between gap-2">
			<p class="leading-snug font-medium break-words {title ? '' : 'whitespace-pre-line'}">
				{#if title}{title}{:else}{@render description(effect.segments)}{/if}
			</p>
			{#if effect.modifier}
				<Badge
					variant="outline"
					class="shrink-0 font-mono text-sm font-semibold tabular-nums {tone.badge}"
					title={tone.label || undefined}
				>
					{effect.modifier}
				</Badge>
			{/if}
		</div>
		{#if title && effect.segments.length}
			<p class="mt-0.5 text-xs/relaxed whitespace-pre-line text-muted-foreground">
				{@render description(effect.segments)}
			</p>
		{/if}
		<div class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1">
			{#if effect.target}
				<Badge variant="secondary" class="h-5 px-1.5 text-[11px]">{effect.target}</Badge>
			{/if}
			<button
				type="button"
				class="truncate font-mono text-[11px] text-muted-foreground/70 underline-offset-2 hover:text-foreground hover:underline focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none"
				title="Open effect record"
				onclick={() => app.navigate(effect.entity)}
			>
				{effect.entity.dataName}
			</button>
		</div>
	</div>
</div>
