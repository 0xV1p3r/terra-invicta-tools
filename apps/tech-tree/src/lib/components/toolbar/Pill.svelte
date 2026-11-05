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
	// A group of related controls in the toolbar; the toolbar draws the dividers between groups.
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
	class={cn('flex min-h-10 items-center gap-0.5 px-2 text-sm', className)}
	{...rest}
>
	{@render children()}
</div>
