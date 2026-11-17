<script lang="ts">
	import type { TechCategory } from '#lib/api.ts';
	import { CATEGORIES } from '#lib/categories.ts';

	let {
		category,
		size = 40,
		class: className = ''
	}: { category: TechCategory; size?: number; class?: string } = $props();

	const info = $derived(CATEGORIES[category]);
</script>

{#if info}
	<img
		src={info.icon}
		alt="{info.label} icon"
		width={size}
		height={size}
		class="shrink-0 {className}"
		draggable="false"
	/>
{/if}
