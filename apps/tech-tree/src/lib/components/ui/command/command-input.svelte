<script lang="ts">
	import { Command as CommandPrimitive } from 'bits-ui';
	import SearchIcon from '@lucide/svelte/icons/search';
	import { cn } from '$lib/utils.js';
	let {
		ref = $bindable(null),
		class: className,
		value = $bindable(''),
		...restProps
	}: CommandPrimitive.InputProps = $props();
</script>

<!-- The input itself has no outline: the wrapper's bottom edge turns ring-coloured (2px, inset so it is never clipped). -->
<div
	class="flex items-center gap-2 border-b px-3 transition-[border-color,box-shadow] focus-within:border-ring focus-within:shadow-[inset_0_-1px_0_0_var(--ring)]"
	data-slot="command-input-wrapper"
>
	<SearchIcon class="size-4 shrink-0 text-muted-foreground" />
	<CommandPrimitive.Input
		bind:ref
		bind:value
		class={cn(
			'h-11 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground disabled:opacity-50',
			className
		)}
		{...restProps}
	/>
</div>
