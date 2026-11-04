<script lang="ts">
	import ArrowDownLeft from '@lucide/svelte/icons/arrow-down-left';
	import ArrowDownRight from '@lucide/svelte/icons/arrow-down-right';
	import ArrowUpLeft from '@lucide/svelte/icons/arrow-up-left';
	import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right';
	import Settings from '@lucide/svelte/icons/settings';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Popover from '$lib/components/ui/popover/index.js';
	import { Switch } from '$lib/components/ui/switch/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import { settings, type Corner } from '$lib/settings.svelte';

	const CORNER_OPTIONS = [
		{ value: 'top-left', label: 'Top left', icon: ArrowUpLeft },
		{ value: 'top-right', label: 'Top right', icon: ArrowUpRight },
		{ value: 'bottom-left', label: 'Bottom left', icon: ArrowDownLeft },
		{ value: 'bottom-right', label: 'Bottom right', icon: ArrowDownRight }
	] as const;
</script>

<Popover.Root>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button {...props} variant="outline" size="icon" aria-label="Settings" title="Settings">
				<Settings />
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-80" align="end">
		<div class="grid gap-4 text-sm">
			<h2 class="font-semibold">Tree display</h2>
			<div class="flex items-start justify-between gap-4">
				<Label for="setting-redundant" class="grid gap-1 font-normal">
					<span class="font-medium">Hide redundant links</span>
					<span class="text-muted-foreground">
						Links already implied by another chain of prerequisites. Still shown when a tech is
						selected.
					</span>
				</Label>
				<Switch id="setting-redundant" bind:checked={settings.hideRedundantLinks} />
			</div>
			<div class="flex items-start justify-between gap-4">
				<Label for="setting-filter-mode" class="grid gap-1 font-normal">
					<span class="font-medium">Highlight filter matches</span>
					<span class="text-muted-foreground">
						Filters dim the rest of the tree instead of hiding it, so the layout stays put.
					</span>
				</Label>
				<Switch
					id="setting-filter-mode"
					bind:checked={
						() => settings.filterMode === 'highlight',
						(on) => (settings.filterMode = on ? 'highlight' : 'hide')
					}
				/>
			</div>
			<div class="flex items-start justify-between gap-4">
				<Label for="setting-overview" class="grid gap-1 font-normal">
					<span class="font-medium">Overview when zoomed out</span>
					<span class="text-muted-foreground">
						Far out, draw plain category blocks with labels for the main techs.
					</span>
				</Label>
				<Switch id="setting-overview" bind:checked={settings.overviewLabels} />
			</div>
			<div class="flex items-center justify-between gap-4">
				<span id="setting-minimap-corner" class="font-medium">Minimap position</span>
				<ToggleGroup.Root
					type="single"
					size="sm"
					variant="outline"
					value={settings.minimapCorner}
					onValueChange={(v) => v && (settings.minimapCorner = v as Corner)}
					aria-labelledby="setting-minimap-corner"
				>
					{#each CORNER_OPTIONS as c (c.value)}
						<ToggleGroup.Item value={c.value} aria-label={c.label} title={c.label}>
							<c.icon />
						</ToggleGroup.Item>
					{/each}
				</ToggleGroup.Root>
			</div>
		</div>
	</Popover.Content>
</Popover.Root>
