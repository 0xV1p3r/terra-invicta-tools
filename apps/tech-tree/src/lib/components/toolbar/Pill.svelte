<script lang="ts" module>
	/** Borderless control inside a pill: every control in the toolbar shares this height. */
	export const pillItem = 'h-8 rounded-full px-2.5 has-[>svg]:px-2';
	/** A toggle inside a pill: filled with the primary color while on, in both themes. */
	export const pillToggle = `${pillItem} border-0 font-normal data-[state=on]:bg-primary data-[state=on]:font-medium data-[state=on]:text-primary-foreground data-[state=on]:hover:bg-primary/90 data-[state=on]:hover:text-primary-foreground`;
</script>

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
		'pointer-events-auto flex min-h-10 items-center gap-0.5 rounded-full border bg-card/90 p-1 text-sm shadow-md backdrop-blur',
		className
	)}
	{...rest}
>
	{@render children()}
</div>
