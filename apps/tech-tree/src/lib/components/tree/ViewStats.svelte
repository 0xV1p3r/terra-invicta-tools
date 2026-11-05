<script lang="ts">
	import type { TreeGraph } from '$lib/api';
	import { formatCost, formatCostFull } from '$lib/format';
	import { app } from '$lib/state.svelte';

	let { graph }: { graph: TreeGraph } = $props();

	const rootName = $derived(
		(graph.nodes.find((n) => n.key === app.root)?.name ?? app.root ?? '')
			.replace(/^TI[^:]*Template:/, '')
			.replace(/([a-z])([A-Z])/g, '$1 $2')
	);
</script>

<div
	class="pointer-events-auto flex max-w-[calc(100vw-1.5rem)] items-center gap-x-4 rounded-full border bg-card/90 px-3 py-1 text-xs whitespace-nowrap text-muted-foreground shadow backdrop-blur-sm"
	role="status"
>
	<span>{graph.nodes.length} items</span>
	<span
		class="truncate"
		title="{rootName} + requirements: {formatCostFull(graph.totalCost)} research"
	>
		{rootName} + requirements: {formatCost(graph.totalCost)} research
	</span>
</div>
