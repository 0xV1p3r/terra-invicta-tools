<script lang="ts">
	import { X, BookOpen } from '@lucide/svelte';
	import type { EntityKey, EntityRef } from '#lib/api.ts';
	import { Button } from '#lib/components/ui/button/index.ts';
	import { ScrollArea } from '#lib/components/ui/scroll-area/index.ts';
	import * as Card from '#lib/components/ui/card/index.ts';
	import { panelHeader, panelPadding, panelTitle } from '#lib/components/styles.ts';
	import { app } from '#lib/state.svelte.ts';
	import { wiki } from '#lib/wiki.svelte.ts';
	import EntityIcon from './EntityIcon.svelte';
	import EntityView from './EntityView.svelte';

	let shown = $state.raw<EntityRef | null>(null);
	let viewport = $state<HTMLElement | null>(null);
	let scrollKey: EntityKey | null = null;
	$effect(() => {
		const key = shown?.key;
		if (!key || !viewport || key === scrollKey) return;
		scrollKey = key;
		viewport.scrollTop = 0;
		titleHidden = false;
	});
	// Once the entity's own title has scrolled away, the header names it instead.
	const TITLE_HEIGHT = 64;
	let titleHidden = $state(false);
	$effect(() => {
		const el = viewport;
		if (!el) return;
		const update = () => (titleHidden = el.scrollTop > TITLE_HEIGHT);
		update();
		el.addEventListener('scroll', update, { passive: true });
		return () => el.removeEventListener('scroll', update);
	});
</script>

<Card.Root class="h-full min-h-0 gap-0 rounded-none border-0 border-l py-0">
	<div class={panelHeader}>
		{#if titleHidden && shown}
			<h2 class="flex min-w-0 items-center gap-2 {panelTitle}">
				<EntityIcon entity={shown} size={20} />
				<span class="truncate" title={shown.name}>{shown.name}</span>
			</h2>
		{:else}
			<h2 class={panelTitle}>Details</h2>
		{/if}
		<div class="flex shrink-0 items-center gap-1">
			{#if app.selected}<Button
					variant="ghost"
					size="sm"
					class="h-8 gap-2 border border-input bg-input/30 px-2.5 shadow-2xs dark:hover:bg-input/50"
					onclick={() => app.selected && wiki.openEntity(app.selected)}
					><BookOpen /> Open in wiki</Button
				>{/if}
			<Button
				variant="ghost"
				size="icon"
				class="size-8"
				aria-label="Close details"
				onclick={() => (app.panelOpen = false)}><X /></Button
			>
		</div>
	</div>
	<ScrollArea class="min-h-0 flex-1" bind:viewportRef={viewport}
		><div class="{panelPadding} py-4">
			<EntityView entityKey={app.selected} bind:shown />
		</div></ScrollArea
	>
</Card.Root>
