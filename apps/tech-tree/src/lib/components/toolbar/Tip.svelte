<script lang="ts">
	// Tooltip for a toolbar control, with its keyboard shortcut. The trigger is a wrapper so the
	// tooltip also shows while the control is disabled (disabled buttons get no pointer events).
	// The wrapper itself is not focusable: focus on a control inside opens the tooltip, and that
	// control (not the wrapper) is described by it.
	import type { Snippet } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import * as Tooltip from '#lib/components/ui/tooltip/index.js';

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

	const controls = 'button, input, [role="combobox"], [tabindex]';
	/** The control last focused or hovered inside the wrapper; it gets the description. */
	let active = $state<HTMLElement | null>(null);

	function track(e: Event) {
		const wrapper = e.currentTarget as HTMLElement;
		const control = (e.target as HTMLElement).closest<HTMLElement>(controls);
		if (control && control !== wrapper && wrapper.contains(control)) active = control;
		else {
			// Pointer over a disabled control lands on the wrapper: use the only control, if any.
			const all = wrapper.querySelectorAll<HTMLElement>(controls);
			if (all.length === 1) active = all[0];
		}
	}

	/** Points the active control's aria-describedby at the open tooltip. */
	function describe(id: string | undefined, control: HTMLElement | null): Attachment<HTMLElement> {
		return (wrapper) => {
			if (!id || !control || !wrapper.contains(control)) return;
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
				onpointerover={track}
				onfocusin={(e) => {
					track(e);
					(onfocus as (e: FocusEvent) => void)?.(e);
				}}
				onfocusout={onblur as (e: FocusEvent) => void}
				{@attach describe(describedBy as string | undefined, active)}>{@render children()}</span
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
