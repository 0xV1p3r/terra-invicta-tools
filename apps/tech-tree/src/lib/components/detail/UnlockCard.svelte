<script lang="ts">
	import { getSpoilerPolicy } from './navigation';
	import SpoilerChip from './SpoilerChip.svelte';
	const spoilersAllowed = getSpoilerPolicy();
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import type { EntityStats, EntityRef } from '$lib/api';
	import * as Collapsible from '$lib/components/ui/collapsible/index.js';
	import { getEntityNavigator } from './navigation';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button';
	const navigate = getEntityNavigator();
	import { untrack } from 'svelte';
	import EntityIcon from './EntityIcon.svelte';
	import StatBlock from './StatBlock.svelte';

	let {
		entity,
		stats,
		open = false
	}: { entity: EntityRef; stats: EntityStats; open?: boolean } = $props();
	// `open` is only the initial state; the card toggles on its own afterwards.
	let expanded = $state(untrack(() => open));
</script>

{#if entity.spoiler && !spoilersAllowed()}<SpoilerChip />{:else}
	<Collapsible.Root bind:open={expanded} class="rounded-lg border bg-muted/20 p-2">
		<div class="flex items-center gap-3">
			<div class="grid size-9 shrink-0 place-items-center rounded-md bg-icon-bg ring-1 ring-border">
				<EntityIcon {entity} size={28} />
			</div>
			<div class="min-w-0 flex-1">
				<Button
					variant="link"
					type="button"
					class="h-auto px-0 py-0.5 text-left leading-snug font-medium break-words text-link underline-offset-2 hover:underline"
					title="Open {entity.name}"
					onclick={() => navigate(entity)}
				>
					{entity.name}
				</Button>
				{#if stats.tags.length}
					<div class="mt-1 flex flex-wrap gap-1">
						{#each stats.tags as tag (tag)}
							<Badge variant="secondary" class="h-5 px-1.5">{tag}</Badge>
						{/each}
					</div>
				{/if}
			</div>
			<Collapsible.Trigger
				class="group grid size-7 shrink-0 place-items-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background focus-visible:outline-none"
				aria-label="Toggle stats of {entity.name}"
			>
				<ChevronRight class="size-4 transition-transform group-data-[state=open]:rotate-90" />
			</Collapsible.Trigger>
		</div>
		<Collapsible.Content class="mt-2.5">
			<StatBlock {stats} tags={false} />
		</Collapsible.Content>
	</Collapsible.Root>
{/if}
