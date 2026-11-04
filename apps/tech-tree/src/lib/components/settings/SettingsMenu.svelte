<script lang="ts">
	import Settings from '@lucide/svelte/icons/settings';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Popover from '$lib/components/ui/popover/index.js';
	import { Switch } from '$lib/components/ui/switch/index.js';
	import { settings } from '$lib/settings.svelte';
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
		</div>
	</Popover.Content>
</Popover.Root>
