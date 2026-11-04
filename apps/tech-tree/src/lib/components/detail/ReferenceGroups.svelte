<script lang="ts">
	import { SvelteMap } from 'svelte/reactivity';
	import type { RefGroup } from '$lib/api';
	import { wiki } from '$lib/wiki.svelte';
	import * as Card from '$lib/components/ui/card';
	import { Button } from '$lib/components/ui/button';
	import EntityLink from './EntityLink.svelte';
	let {
		title,
		groups,
		technical = false
	}: { title: string; groups: RefGroup[]; technical?: boolean } = $props();
	const displayGroups = $derived.by(() => {
		if (technical) return groups;
		const merged = new SvelteMap<string, RefGroup>();
		for (const group of groups) {
			const target = merged.get(group.template) ?? { ...group, field: '', entities: [] };
			target.entities = [
				...new SvelteMap([...target.entities, ...group.entities].map((e) => [e.key, e])).values()
			];
			merged.set(group.template, target);
		}
		return [...merged.values()];
	});
	let expanded = $state<Record<string, boolean>>({});
	$effect(() => {
		void groups;
		expanded = {};
	});
</script>

{#if groups.length}
	<Card.Root class="mt-5 gap-2 border-0 bg-transparent py-0 shadow-none ring-0">
		<Card.Header class="px-0"
			><Card.Title class="text-xs text-muted-foreground uppercase">{title}</Card.Title></Card.Header
		>
		{#each displayGroups as group, i (`${i}:${group.template}:${group.field}`)}
			<Card.Content class="px-0"
				><Card.Description
					>{wiki.label(group.template)}{#if technical}
						· {group.field} ({group.entities.length}){/if}</Card.Description
				>
				{#each expanded[i] ? group.entities : group.entities.slice(0, 25) as entity, j (`${j}:${entity.key}`)}<EntityLink
						{entity}
					/>{/each}
				{#if group.entities.length > 25}<Button
						variant="link"
						size="sm"
						onclick={() => (expanded[i] = !expanded[i])}
						>{expanded[i] ? 'Show less' : `Show all ${group.entities.length}`}</Button
					>{/if}
			</Card.Content>{/each}</Card.Root
	>
{/if}
