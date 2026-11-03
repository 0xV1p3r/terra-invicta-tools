<script lang="ts">
	import type { EntityRef } from '$lib/api';
	import { KIND_LABELS } from '$lib/format';
	import { app } from '$lib/state.svelte';
	import EntityIcon from './EntityIcon.svelte';

	let { entity }: { entity: EntityRef } = $props();
</script>

<button
	type="button"
	class="inline-flex max-w-full items-center gap-1.5 rounded-md px-1 py-0.5 text-left align-middle font-medium text-sky-700 hover:bg-accent hover:underline focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none dark:text-sky-400"
	title="{KIND_LABELS[entity.kind] ?? entity.kind} · {entity.dataName}"
	onclick={() => app.navigate(entity)}
>
	<EntityIcon {entity} size={16} />
	<span class="truncate">{entity.name}</span>
</button>
