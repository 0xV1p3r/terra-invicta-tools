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

<div class="flex items-center gap-2 border-b px-3" data-slot="command-input-wrapper">
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
