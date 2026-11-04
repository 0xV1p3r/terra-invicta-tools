<script lang="ts">
	import { X, BookOpen } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { ScrollArea } from '$lib/components/ui/scroll-area';
	import * as Card from '$lib/components/ui/card';
	import { app } from '$lib/state.svelte';
	import { wiki } from '$lib/wiki.svelte';
	import EntityView from './EntityView.svelte';
</script>

<Card.Root class="h-full min-h-0 gap-0 rounded-none border-0 border-l py-0">
	<Card.Header class="flex flex-row items-center justify-between border-b px-4 py-2">
		<Card.Title>Details</Card.Title>
		{#if app.selected}<Button
				variant="ghost"
				size="sm"
				onclick={() => app.selected && wiki.openEntity(app.selected)}
				><BookOpen /> Open in wiki</Button
			>{/if}
		<Button
			variant="ghost"
			size="icon"
			aria-label="Close details"
			onclick={() => (app.panelOpen = false)}><X /></Button
		>
	</Card.Header>
	<ScrollArea class="min-h-0 flex-1"
		><Card.Content class="p-4"><EntityView entityKey={app.selected} /></Card.Content></ScrollArea
	>
</Card.Root>
