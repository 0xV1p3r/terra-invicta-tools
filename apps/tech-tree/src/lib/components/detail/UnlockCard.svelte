<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import type { EntityStats, EntityRef } from '$lib/api';
	import * as Collapsible from '$lib/components/ui/collapsible/index.js';
	import { app } from '$lib/state.svelte';
	import { untrack } from 'svelte';
	import EntityIcon from './EntityIcon.svelte';
	import StatBlock from './StatBlock.svelte';

	let {
		entity,
		stats,
		open = true
	}: { entity: EntityRef; stats: EntityStats; open?: boolean } = $props();
	// `open` is only the initial state; the card toggles on its own afterwards.
	let expanded = $state(untrack(() => open));
</script>

<Collapsible.Root bind:open={expanded} class="rounded-lg border bg-muted/20 p-2.5">
	<div class="flex items-start gap-3">
		<div class="grid size-9 shrink-0 place-items-center rounded-md bg-zinc-800 ring-1 ring-border">
			<EntityIcon {entity} size={28} />
		</div>
		<div class="min-w-0 flex-1">
			<button
				type="button"
				class="text-left leading-snug font-medium break-words text-sky-700 underline-offset-2 hover:underline focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none dark:text-sky-400"
				title="Open {entity.name}"
				onclick={() => app.navigate(entity)}
			>
				{entity.name}
			</button>
			<p class="truncate font-mono text-[11px] text-muted-foreground/70">{entity.dataName}</p>
		</div>
		<Collapsible.Trigger
			class="group grid size-7 shrink-0 place-items-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
			aria-label="Toggle stats of {entity.name}"
		>
			<ChevronRight class="size-4 transition-transform group-data-[state=open]:rotate-90" />
		</Collapsible.Trigger>
	</div>
	<Collapsible.Content class="mt-2.5">
		<StatBlock {stats} />
	</Collapsible.Content>
</Collapsible.Root>
