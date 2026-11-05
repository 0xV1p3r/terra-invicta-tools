<script lang="ts" module>
	/** Borderless toolbar control: every control in the toolbar shares this height. */
	export const pillItem = 'h-8 rounded-md px-2.5 has-[>svg]:px-2';
	/**
	 * A toolbar toggle. While on it is a raised tile (like a segmented control) rather than a
	 * primary fill, which glared against the muted toolbar.
	 */
	export const pillToggle = `${pillItem} border-0 font-normal text-muted-foreground hover:bg-accent/60 data-[state=on]:bg-background data-[state=on]:font-medium data-[state=on]:text-foreground data-[state=on]:shadow-xs data-[state=on]:ring-1 data-[state=on]:ring-border data-[state=on]:hover:bg-background data-[state=on]:hover:text-foreground dark:data-[state=on]:bg-input dark:data-[state=on]:hover:bg-input`;
</script>

<script lang="ts">
	// A group of related controls in the toolbar, laid out as an inline block so the toolbar can
	// wrap groups as wholes. Each group draws a short divider at its left edge; the -ml-px moves
	// it under the toolbar's clip when the group starts a row, so dividers never dangle.
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
		'relative -ml-px inline-flex min-h-10 shrink-0 items-center align-middle gap-0.5 px-1.5 text-sm before:absolute before:top-1/2 before:left-0 before:h-6 before:w-px before:-translate-y-1/2 before:bg-border',
		className
	)}
	{...rest}
>
	{@render children()}
</div>
