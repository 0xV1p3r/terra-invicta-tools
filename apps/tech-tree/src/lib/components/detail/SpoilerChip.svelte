<script lang="ts">
	import { EyeOff } from '@lucide/svelte';
	import * as Tooltip from '$lib/components/ui/tooltip';
</script>

<Tooltip.Root
	><Tooltip.Trigger
		><span
			class="inline-flex items-center gap-1.5 rounded-md bg-muted px-2 py-1 text-xs text-muted-foreground"
			><EyeOff class="size-3.5" />Spoiler hidden<span aria-hidden="true" class="blur-sm select-none"
				>••••••</span
			></span
		></Tooltip.Trigger
	><Tooltip.Content>Enable spoilers in settings to see this</Tooltip.Content></Tooltip.Root
>
