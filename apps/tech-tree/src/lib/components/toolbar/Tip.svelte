<script lang="ts">
	// Tooltip for a toolbar control, with its keyboard shortcut. The trigger is a wrapper so the
	// tooltip also shows while the control is disabled (disabled buttons get no pointer events).
	import type { Snippet } from 'svelte';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';

	let {
		text,
		shortcut,
		side = 'bottom',
		children
	}: {
		text: string;
		shortcut?: string;
		side?: 'top' | 'bottom' | 'left' | 'right';
		children: Snippet;
	} = $props();
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		{#snippet child({ props })}
			<span {...props} class="inline-flex">{@render children()}</span>
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Content {side} sideOffset={6} class="max-w-64">
		{text}
		{#if shortcut}
			<kbd
				class="ml-1.5 rounded border border-primary-foreground/30 px-1 font-sans text-[10px] whitespace-nowrap"
				>{shortcut}</kbd
			>
		{/if}
	</Tooltip.Content>
</Tooltip.Root>
