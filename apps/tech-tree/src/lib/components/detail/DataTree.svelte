<script lang="ts">
	import * as Collapsible from '#lib/components/ui/collapsible/index.ts';
	import * as Card from '#lib/components/ui/card/index.ts';
	import { Button } from '#lib/components/ui/button/index.ts';
	import DataTree from './DataTree.svelte';
	let { value, label = 'Record' }: { value: unknown; label?: string } = $props();
	let limit = $state(25);
	const entries = $derived(
		value !== null && typeof value === 'object' ? Object.entries(value) : null
	);
</script>

{#if entries?.length}
	<Collapsible.Root open={label === 'Record'} class="ml-2 border-l pl-2 text-xs">
		<Collapsible.Trigger class="py-1 text-left font-mono break-all"
			>{label} · {entries.length} {Array.isArray(value) ? 'items' : 'fields'}</Collapsible.Trigger
		>
		<Collapsible.Content>
			{#each entries.slice(0, limit) as [key, item] (key)}<DataTree
					value={item}
					label={key}
				/>{/each}
			{#if entries.length > limit}<Button variant="link" size="sm" onclick={() => (limit += 100)}
					>Show more ({entries.length - limit} remaining)</Button
				>{/if}
		</Collapsible.Content></Collapsible.Root
	>
{:else}
	<Card.Content class="flex gap-2 px-2 py-1 text-xs break-all"
		><Card.Title class="font-mono text-xs">{label}</Card.Title><Card.Description
			class="text-xs whitespace-pre-wrap"
			>{value == null
				? 'null'
				: entries
					? Array.isArray(value)
						? '[]'
						: '{}'
					: value === ''
						? '(empty)'
						: String(value)}</Card.Description
		></Card.Content
	>
{/if}
