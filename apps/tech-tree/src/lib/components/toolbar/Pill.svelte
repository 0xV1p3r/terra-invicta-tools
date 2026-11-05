<script lang="ts" module>
	/** A standalone toolbar control (button, select trigger): outlined, so it reads as a control. */
	export const pillItem =
		'h-8 rounded-md px-2.5 has-[>svg]:px-2 border border-input bg-input/30 shadow-xs hover:bg-input/60 hover:text-foreground';
	/** Plain icon button for the utility cluster (search sits beside it in its own field). */
	export const pillIcon = 'size-8 rounded-md text-muted-foreground hover:text-foreground';
	/**
	 * Track for a set of toggles (a segmented control): a recessed strip whose on segment is a raised
	 * tile. Goes on the ToggleGroup.Root.
	 */
	export const segmented =
		'h-8 items-center gap-0.5 rounded-md border border-input bg-background/80 p-0.5 shadow-inner';
	/**
	 * A segment of a `segmented` track. On: a raised, lighter tile with full-strength text; off:
	 * muted text. aria-disabled segments look disabled but stay focusable, so their tooltip explains why.
	 */
	export const pillToggle =
		'h-[1.625rem] min-w-[1.625rem] rounded-[calc(var(--radius-md)-2px)] border-0 px-2.5 has-[>svg]:px-2 text-[0.8125rem] font-normal text-muted-foreground hover:bg-accent/50 hover:text-foreground data-[state=on]:bg-accent data-[state=on]:font-medium data-[state=on]:text-foreground data-[state=on]:shadow-sm data-[state=on]:ring-1 data-[state=on]:ring-white/10 data-[state=on]:hover:bg-accent data-[state=on]:hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-0 aria-disabled:cursor-default aria-disabled:opacity-40 aria-disabled:hover:bg-transparent aria-disabled:hover:text-muted-foreground';
	/** Small uppercase caption naming the control next to it. */
	export const groupLabel =
		'pr-0.5 pl-1 text-[0.6875rem] font-medium tracking-[0.06em] text-muted-foreground/80 uppercase select-none';
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
