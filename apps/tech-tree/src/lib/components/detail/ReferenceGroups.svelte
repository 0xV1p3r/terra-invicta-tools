<script lang="ts">
	import { SvelteMap } from 'svelte/reactivity';
	import type { EntityKey, RefGroup } from '#lib/api.ts';
	import { templateLabel } from '#lib/format.ts';
	import { wiki } from '#lib/wiki.svelte.ts';
	import { sectionHeading, subHeading } from '#lib/components/styles.ts';
	import { Button } from '#lib/components/ui/button/index.ts';
	import EntityLink from './EntityLink.svelte';
	let {
		title,
		groups,
		technical = false,
		exclude
	}: {
		title: string;
		groups: RefGroup[];
		technical?: boolean;
		/** Entities the panel already shows elsewhere (requirements, unlocks…): left out unless technical. */
		exclude?: ReadonlySet<EntityKey>;
	} = $props();
	const displayGroups = $derived.by(() => {
		if (technical) return groups;
		const merged = new SvelteMap<string, RefGroup>();
		for (const group of groups) {
			const target = merged.get(group.template) ?? { ...group, field: '', entities: [] };
			const entities = group.entities.filter((e) => !exclude?.has(e.key));
			target.entities = [
				...new SvelteMap([...target.entities, ...entities].map((e) => [e.key, e])).values()
			];
			merged.set(group.template, target);
		}
		return [...merged.values()].filter((g) => g.entities.length);
	});
	let expanded = $state<Record<string, boolean>>({});
	$effect(() => {
		void groups;
		expanded = {};
	});
</script>

{#if displayGroups.length}
	<section class="mt-5 grid gap-2">
		<h3 class={sectionHeading}>{title}</h3>
		{#each displayGroups as group, i (`${i}:${group.template}:${group.field}`)}
			<div>
				<h4 class={subHeading}>
					{templateLabel(
						group.template,
						wiki.sections.find((s) => s.template === group.template)?.label
					)}{#if technical}
						· {group.field} ({group.entities.length}){/if}
				</h4>
				{#each expanded[i] ? group.entities : group.entities.slice(0, 25) as entity, j (`${j}:${entity.key}`)}<EntityLink
						{entity}
					/>{/each}
				{#if group.entities.length > 25}<Button
						variant="link"
						size="sm"
						onclick={() => (expanded[i] = !expanded[i])}
						>{expanded[i] ? 'Show less' : `Show all ${group.entities.length}`}</Button
					>{/if}
			</div>{/each}
	</section>
{/if}
