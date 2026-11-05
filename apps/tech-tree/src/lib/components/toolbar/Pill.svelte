<script lang="ts" module>
	/** A standalone toolbar control (button, select trigger): outlined, so it reads as a control. */
	export const pillItem =
		'h-8 rounded-md px-2.5 has-[>svg]:px-2 border border-input bg-input/30 shadow-xs hover:bg-input/60 hover:text-foreground';
	/** Plain icon button for the utility cluster (search sits beside it in its own field). */
	export const pillIcon = 'size-8 rounded-md text-muted-foreground hover:text-foreground';
	/**
	 * A set of toggles as one joined button group, the same height and outline as the selects. The
	 * group clips its own corners and draws the dividers: each segment sits in a tooltip wrapper,
	 * so per-item first/last rounding would round every segment. Goes on the ToggleGroup.Root.
	 */
	export const segmented =
		'h-8 gap-0 overflow-hidden rounded-md border border-white/20 bg-background/40 shadow-xs divide-x divide-white/20';
	/**
	 * A segment of a `segmented` group. On: a lighter fill with full-strength text; off: muted text.
	 * aria-disabled segments look disabled but stay focusable, so their tooltip explains why.
	 */
	export const pillToggle =
		'h-[1.875rem] min-w-[1.875rem] rounded-none border-0 px-3 has-[>svg]:px-2.5 text-sm font-normal text-muted-foreground hover:bg-input/40 hover:text-foreground data-[state=on]:bg-white/10 data-[state=on]:font-medium data-[state=on]:text-foreground data-[state=on]:hover:bg-white/10 data-[state=on]:hover:text-foreground focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring focus-visible:ring-offset-0 aria-disabled:cursor-default aria-disabled:opacity-40 aria-disabled:hover:bg-transparent aria-disabled:hover:text-muted-foreground';
</script>

<script lang="ts">
	// A group of related controls in the toolbar, a flex item so the toolbar can wrap groups as
	// wholes; a group wider than the toolbar wraps its own controls rather than being clipped.
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
	class={cn('flex min-h-9 max-w-full shrink-0 flex-wrap items-center gap-1.5 text-sm', className)}
	{...rest}
>
	{@render children()}
</div>
