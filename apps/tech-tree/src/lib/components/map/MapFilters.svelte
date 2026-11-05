<script lang="ts">
	// Header controls of the solar system map: filters popover and distance scale.
	import Funnel from '@lucide/svelte/icons/funnel';
	import type { ResourceRating, SpaceResource } from '$lib/api';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Checkbox } from '$lib/components/ui/checkbox/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Popover from '$lib/components/ui/popover/index.js';
	import * as Select from '$lib/components/ui/select/index.js';
	import { Switch } from '$lib/components/ui/switch/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import { settings, type MapScale } from '$lib/settings.svelte';
	import { ITEM_TYPES, RESOURCES, mapState } from './map-state.svelte';
	import { TYPE_COLORS, TYPE_LABELS, type ItemType } from './scene';

	/** Minimum ratings offered ("any" = no constraint). */
	const MIN_RATINGS: ResourceRating[] = ['low', 'medium', 'high', 'max'];
	const ANY = 'any';

	const activeCount = $derived(
		(mapState.types.length < ITEM_TYPES.length ? 1 : 0) +
			(mapState.habSitesOnly ? 1 : 0) +
			Object.keys(mapState.minRatings).length
	);

	function setType(type: ItemType, on: boolean) {
		mapState.types = on
			? ITEM_TYPES.filter((t) => t === type || mapState.types.includes(t))
			: mapState.types.filter((t) => t !== type);
	}

	function setRating(res: SpaceResource, value: string) {
		const next = { ...mapState.minRatings };
		if (value === ANY) delete next[res];
		else next[res] = value as ResourceRating;
		mapState.minRatings = next;
	}
</script>

<div class="flex items-center gap-2" role="group" aria-label="Map controls">
	<Popover.Root>
		<Popover.Trigger>
			{#snippet child({ props })}
				<Button {...props} variant="outline" size="sm">
					<Funnel /> Filters
					{#if activeCount}<Badge class="h-5 min-w-5 px-1">{activeCount}</Badge>{/if}
				</Button>
			{/snippet}
		</Popover.Trigger>
		<Popover.Content class="w-80" align="start">
			<div class="grid gap-4 text-sm">
				<div class="grid gap-2">
					<h2 class="font-semibold">Body types</h2>
					<div class="grid grid-cols-2 gap-x-4 gap-y-2">
						{#each ITEM_TYPES as type (type)}
							<Label class="flex items-center gap-2 font-normal">
								<Checkbox
									aria-label={TYPE_LABELS[type]}
									checked={mapState.types.includes(type)}
									onCheckedChange={(on) => setType(type, on)}
								/>
								{#if type === 'lagrange'}
									<span
										aria-hidden="true"
										class="size-2 rotate-45 border"
										style="border-color: {TYPE_COLORS[type]}"
									></span>
								{:else if type === 'planet'}
									<span
										aria-hidden="true"
										class="size-2.5 rounded-full border border-muted-foreground bg-muted"
									></span>
								{:else}
									<span
										aria-hidden="true"
										class="size-2.5 rounded-full"
										style="background: {TYPE_COLORS[type]}"
									></span>
								{/if}
								{TYPE_LABELS[type]}
							</Label>
						{/each}
					</div>
				</div>
				<p class="text-xs text-muted-foreground">
					Planets: individual colors. Marker sizes are illustrative, not to scale.
				</p>
				<div class="flex items-center justify-between gap-4">
					<Label for="map-hab-sites" class="font-medium">Has hab sites</Label>
					<Switch id="map-hab-sites" bind:checked={mapState.habSitesOnly} />
				</div>
				<div class="grid gap-2">
					<h2 class="font-semibold">Minimum resources</h2>
					{#each RESOURCES as res (res)}
						<div class="flex items-center justify-between gap-4">
							<span class="capitalize">{res}</span>
							<Select.Root
								type="single"
								value={mapState.minRatings[res] ?? ANY}
								onValueChange={(v) => setRating(res, v)}
							>
								<Select.Trigger size="sm" class="w-28 capitalize" aria-label="Minimum {res}">
									{mapState.minRatings[res] ? `≥ ${mapState.minRatings[res]}` : 'Any'}
								</Select.Trigger>
								<Select.Content>
									<Select.Item value={ANY} label="Any">Any</Select.Item>
									{#each MIN_RATINGS as r (r)}
										<Select.Item value={r} label="≥ {r}" class="capitalize">≥ {r}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
					{/each}
				</div>
				<div class="flex items-start justify-between gap-4">
					<Label for="map-filter-mode" class="grid gap-1 font-normal">
						<span class="font-medium">Hide non-matching</span>
						<span class="text-muted-foreground">Off, they are only dimmed.</span>
					</Label>
					<Switch
						id="map-filter-mode"
						bind:checked={
							() => settings.mapFilterMode === 'hide',
							(on) => (settings.mapFilterMode = on ? 'hide' : 'highlight')
						}
					/>
				</div>
				<Button
					variant="outline"
					size="sm"
					disabled={!mapState.filtersActive}
					onclick={() => mapState.clearFilters()}>Clear filters</Button
				>
			</div>
		</Popover.Content>
	</Popover.Root>

	<ToggleGroup.Root
		type="single"
		size="sm"
		variant="outline"
		value={settings.mapScale}
		onValueChange={(v) => v && (settings.mapScale = v as MapScale)}
		aria-label="Distance scale"
	>
		<ToggleGroup.Item
			value="log"
			title="Logarithmic distances: inner and outer orbits both readable">Log</ToggleGroup.Item
		>
		<ToggleGroup.Item value="linear" title="True distance proportions">Linear</ToggleGroup.Item>
	</ToggleGroup.Root>
</div>
