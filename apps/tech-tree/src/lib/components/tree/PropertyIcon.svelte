<script lang="ts" module>
	import type { Component } from 'svelte';
	import type { EntityKind } from '#lib/api.ts';

	/** Entity kind of an `unlock:<kind>` property (for its icon). */
	function unlockKind(id: string): EntityKind {
		const kind = id.split(':')[1];
		if (kind === 'grantedOrg') return 'org';
		if (kind === 'claim') return 'faction';
		return kind in KIND_ICONS ? (kind as EntityKind) : 'other';
	}

	function fallbackIcon(p: PropertyInfo): Component {
		if (p.group === 'unlock') return KIND_ICONS[unlockKind(p.id)];
		return DOMAIN_ICONS[p.domain ?? 'other'] ?? Sparkles;
	}
</script>

<script lang="ts">
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import { assetUrl, type PropertyInfo } from '#lib/api.ts';
	import { KIND_ICONS } from '#lib/components/detail/EntityIcon.svelte';
	import { DOMAIN_ICONS, SENTIMENT } from '#lib/effect-style.ts';
	import CategoryIcon from './CategoryIcon.svelte';

	let { property, size = 28 }: { property: PropertyInfo; size?: number } = $props();

	const tone = $derived(SENTIMENT[property.sentiment] ?? SENTIMENT.neutral);
	const url = $derived(assetUrl(property.icon));
	let failed = $state(false);
	$effect(() => {
		void url;
		failed = false;
	});
	const gameIcon = $derived(!property.category && !!url && !failed);
	const Fallback = $derived(fallbackIcon(property));
</script>

<span
	class="grid shrink-0 place-items-center rounded-md {gameIcon
		? `bg-icon-bg ring-1 ${tone.ring}`
		: property.category
			? ''
			: tone.tile}"
	style:width="{size}px"
	style:height="{size}px"
	aria-hidden="true"
>
	{#if property.category}
		<CategoryIcon category={property.category} {size} />
	{:else if gameIcon}
		<img
			src={url}
			alt=""
			class="object-contain"
			style:width="{size * 0.78}px"
			style:height="{size * 0.78}px"
			draggable="false"
			onerror={() => (failed = true)}
		/>
	{:else}
		<Fallback style="width: {size * 0.58}px; height: {size * 0.58}px" />
	{/if}
</span>
