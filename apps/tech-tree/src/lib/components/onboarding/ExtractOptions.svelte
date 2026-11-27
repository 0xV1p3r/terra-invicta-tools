<script lang="ts">
	import ChevronRight from '@lucide/svelte/icons/chevron-right';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as RadioGroup from '#lib/components/ui/radio-group/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { settings } from '#lib/settings.svelte.ts';

	let { disabled = false }: { disabled?: boolean } = $props();

	const id = $props.id();
	let open = $state(false);
</script>

<!-- Saved in the settings: later extractions (update notice, Extract again) use them too. -->
<Collapsible.Root bind:open>
	<Collapsible.Trigger
		class="group -ml-1 flex items-center gap-1 rounded-md px-1 py-0.5 text-sm font-medium text-muted-foreground hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
	>
		<ChevronRight class="size-4 transition-transform group-data-[state=open]:rotate-90" />
		Options
	</Collapsible.Trigger>
	<Collapsible.Content class="grid gap-3 pt-3 text-sm">
		<div class="flex items-start justify-between gap-4">
			<Label for="{id}-large-art" class="grid gap-1 font-normal">
				<span class="leading-snug font-medium"
					>Include large artwork (illustrations, portraits, hab modules)</span
				>
				<span class="text-xs leading-relaxed text-muted-foreground">About 1.5 GB.</span>
			</Label>
			<Switch id="{id}-large-art" {disabled} bind:checked={settings.extractLargeArt} />
		</div>
		<div class="grid gap-2">
			<span id="{id}-format" class="font-medium">Image format</span>
			<RadioGroup.Root
				class="flex flex-wrap gap-x-6 gap-y-2"
				aria-labelledby="{id}-format"
				{disabled}
				value={settings.extractImageFormat}
				onValueChange={(v) => (settings.extractImageFormat = v === 'webp' ? 'webp' : 'png')}
			>
				<div class="flex items-center gap-2">
					<RadioGroup.Item value="webp" id="{id}-webp" />
					<Label for="{id}-webp" class="font-normal">WebP (default)</Label>
				</div>
				<div class="flex items-center gap-2">
					<RadioGroup.Item value="png" id="{id}-png" />
					<Label for="{id}-png" class="font-normal">PNG</Label>
				</div>
			</RadioGroup.Root>
			<p class="text-xs leading-relaxed text-muted-foreground">
				WebP is lossless: same quality as PNG, faster to extract and smaller.
			</p>
		</div>
		<p class="text-xs leading-relaxed text-muted-foreground">
			Icons, logos and flags are always extracted.
		</p>
	</Collapsible.Content>
</Collapsible.Root>
