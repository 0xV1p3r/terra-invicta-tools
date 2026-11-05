<script lang="ts" module>
	/** Borderless toolbar control: every control in the toolbar shares this height. */
	export const pillItem = 'h-8 rounded-md px-2.5 has-[>svg]:px-2';
	/**
	 * A toolbar toggle. While on it is a raised tile (like a segmented control) rather than a
	 * primary fill, which glared against the muted toolbar. The tile alone is low contrast, so an on
	 * toggle also gets a bright underline, like the category toggles; focus on an on toggle swaps
	 * the tile's hairline for the toolbar's usual focus ring. aria-disabled toggles look disabled
	 * but stay focusable, so their tooltip explains why.
	 */
	export const pillToggle = `${pillItem} relative border-0 after:absolute after:inset-x-2.5 after:bottom-0.5 after:h-0.5 after:rounded-full after:bg-foreground after:opacity-0 data-[state=on]:after:opacity-100 font-normal text-muted-foreground hover:bg-accent/60 data-[state=on]:bg-background data-[state=on]:font-medium data-[state=on]:text-foreground data-[state=on]:shadow-xs data-[state=on]:ring-1 data-[state=on]:ring-border data-[state=on]:hover:bg-background data-[state=on]:hover:text-foreground dark:data-[state=on]:bg-input dark:data-[state=on]:hover:bg-input data-[state=on]:focus-visible:ring-2 data-[state=on]:focus-visible:ring-ring aria-disabled:cursor-default aria-disabled:opacity-50 aria-disabled:hover:bg-transparent aria-disabled:hover:text-muted-foreground`;
</script>

<script lang="ts">
	// A group of related controls in the toolbar, a flex item so the toolbar can wrap groups as
	// wholes; a group wider than the toolbar wraps its own controls rather than being clipped. Each group draws a short divider at its left edge; the -ml-px moves
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
		'relative -ml-px flex min-h-10 max-w-full shrink-0 flex-wrap items-center gap-0.5 px-1.5 text-sm before:absolute before:top-1/2 before:left-0 before:h-6 before:w-px before:-translate-y-1/2 before:bg-border',
		className
	)}
	{...rest}
>
	{@render children()}
</div>
