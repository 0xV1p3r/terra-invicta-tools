<script lang="ts">
	import { getSpoilerPolicy } from './navigation';
	import SpoilerChip from './SpoilerChip.svelte';
	const spoilersAllowed = getSpoilerPolicy();
	import type { EntityRef } from '$lib/api';
	import { KIND_LABELS } from '$lib/format';
	import { getEntityNavigator } from './navigation';
	import { Button } from '$lib/components/ui/button';
	const navigate = getEntityNavigator();
	import EntityIcon from './EntityIcon.svelte';

	let { entity }: { entity: EntityRef } = $props();
</script>

{#if entity.spoiler && !spoilersAllowed()}<SpoilerChip />{:else}
	<Button
		variant="link"
		type="button"
		class="inline-flex max-w-full items-center gap-1.5 rounded-md px-1 py-0.5 text-left align-middle font-medium text-link hover:bg-accent hover:underline"
		title="{KIND_LABELS[entity.kind] ?? entity.kind} · {entity.dataName}"
		onclick={() => navigate(entity)}
	>
		<EntityIcon {entity} size={16} />
		<span class="truncate">{entity.name}</span>
	</Button>
{/if}
