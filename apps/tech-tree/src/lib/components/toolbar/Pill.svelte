<script lang="ts" module>
	/**
	 * A standalone toolbar control (button, select trigger): a faint fill, a translucent outline
	 * and a hairline of light along the top, so it reads as a raised control.
	 */
	export const pillItem =
		'h-8 rounded-md px-2.5 has-[>svg]:px-2 border border-input bg-white/[0.035] font-medium shadow-[var(--highlight)] hover:bg-white/[0.07] hover:text-foreground';
	/** Plain icon button for the utility cluster (search sits beside it in its own field). */
	export const pillIcon = 'size-8 rounded-md text-muted-foreground hover:text-foreground';
	/**
	 * A set of toggles as one recessed track, the same height as the selects; the active segment
	 * is raised out of it. Goes on the ToggleGroup.Root.
	 */
	export const segmented =
		'h-8 items-center gap-px rounded-lg border border-border bg-white/[0.035] p-0.5';
	/**
	 * A segment of a `segmented` group. On: raised, full-strength text; off: muted text on the
	 * track. aria-disabled segments look disabled but stay focusable, so their tooltip explains why.
	 */
	export const pillToggle =
		'h-[1.625rem] min-w-[1.625rem] rounded-md border-0 px-2.5 has-[>svg]:px-2 text-sm font-medium text-muted-foreground hover:bg-transparent hover:text-foreground data-[state=on]:bg-raised data-[state=on]:text-foreground data-[state=on]:shadow-[inset_0_0_0_1px_rgb(255_255_255/0.08),0_1px_2px_rgb(0_0_0/0.3)] data-[state=on]:hover:bg-raised data-[state=on]:hover:text-foreground focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring focus-visible:ring-offset-0 aria-disabled:cursor-default aria-disabled:opacity-40 aria-disabled:hover:text-muted-foreground';
</script>

<script lang="ts">
	// A group of related controls in the toolbar, a flex item so the toolbar can wrap groups as
	// wholes; a group wider than the toolbar wraps its own controls rather than being clipped.
	import type { Snippet } from 'svelte';
	import type { HTMLAttributes } from 'svelte/elements';
	import { cn } from '#lib/utils.js';

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
