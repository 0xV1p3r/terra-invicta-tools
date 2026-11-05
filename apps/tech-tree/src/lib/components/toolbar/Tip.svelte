<script lang="ts">
	// Tooltip for a toolbar control, with its keyboard shortcut. The trigger is a wrapper so the
	// tooltip also shows while the control is disabled (disabled buttons get no pointer events).
	// The wrapper itself is not focusable: focus on the control inside opens the tooltip, and the
	// control (not the wrapper) is described by it.
	import type { Snippet } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
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

	/** Points the wrapped control's aria-describedby at the open tooltip. */
	function describe(id: string | undefined): Attachment<HTMLElement> {
		return (wrapper) => {
			const control = wrapper.querySelector<HTMLElement>('button, input, [tabindex]');
			if (!id || !control) return;
			const previous = control.getAttribute('aria-describedby');
			control.setAttribute('aria-describedby', previous ? `${previous} ${id}` : id);
			return () => {
				if (previous) control.setAttribute('aria-describedby', previous);
				else control.removeAttribute('aria-describedby');
			};
		};
	}
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		{#snippet child({ props })}
			{@const {
				tabindex: _tabindex,
				'aria-describedby': describedBy,
				onfocus,
				onblur,
				...rest
			} = props}
			<span
				{...rest}
				class="inline-flex"
				onfocusin={onfocus as (e: FocusEvent) => void}
				onfocusout={onblur as (e: FocusEvent) => void}
				{@attach describe(describedBy as string | undefined)}>{@render children()}</span
			>
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
