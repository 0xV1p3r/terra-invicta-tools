<script lang="ts">
	import { assetUrl, type EffectLine } from '$lib/api';
	import * as Card from '$lib/components/ui/card';
	import CategoryIcon from '$lib/components/tree/CategoryIcon.svelte';
	import { DOMAIN_ICONS, SENTIMENT } from '$lib/effect-style';
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
</script>

<Card.Content
	class="grid size-9 shrink-0 place-items-center rounded-md p-0 {gameIcon
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
</Card.Content>
