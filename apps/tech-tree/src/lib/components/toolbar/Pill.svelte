<script lang="ts">
	// A floating group of related controls over the tree canvas.
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';
	import { cn } from '$lib/utils.js';

	let {
		class: className,
		children,
		...rest
	}: HTMLAttributes<HTMLDivElement> & { children: Snippet } = $props();
</script>

<div
	role="group"
	class={cn(
		'pointer-events-auto flex items-center gap-1.5 rounded-full border bg-card/90 p-1 text-sm shadow-md backdrop-blur',
		className
	)}
	{...rest}
>
	{@render children()}
</div>
