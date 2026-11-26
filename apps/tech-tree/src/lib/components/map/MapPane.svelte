<script lang="ts">
	// Side pane of the solar system map: searchable list of the bodies and Lagrange points, grouped
	// by the system they're shown in, and the map filters (which narrow the list too).
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Funnel from '@lucide/svelte/icons/funnel';
	import LogIn from '@lucide/svelte/icons/log-in';
	import Search from '@lucide/svelte/icons/search';
	import X from '@lucide/svelte/icons/x';
	import { tick } from 'svelte';
	import type { EntityKey, EntityRef, ResourceRating, SpaceResource } from '#lib/api.ts';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import * as Collapsible from '#lib/components/ui/collapsible/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import { ScrollArea } from '#lib/components/ui/scroll-area/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { settings } from '#lib/settings.svelte.ts';
	import { app } from '#lib/state.svelte.ts';
	import { ITEM_TYPES, RESOURCES, mapState } from './map-state.svelte';
	import { TYPE_COLORS, TYPE_LABELS, bodyColor, type ItemType, type MapBody } from './scene';

	/** Minimum ratings offered ("any" = no constraint). */
	const MIN_RATINGS: ResourceRating[] = ['low', 'medium', 'high', 'max'];
	const ANY = 'any';

	/** Same order as the map's keyboard cycling: by type, then innermost first. */
	const ORDER: Record<ItemType, number> = {
		star: 0,
		planet: 1,
		dwarfPlanet: 2,
		planetaryMoon: 2,
		asteroidalMoon: 3,
		comet: 4,
		asteroid: 5,
		unknown: 5,
		lagrange: 6
	};

	let query = $state('');
	let filtersOpen = $state(false);

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

	interface Row {
		entity: EntityRef;
		type: ItemType;
		color: string;
	}
	interface Group {
		center: MapBody;
		rows: Row[];
	}

	/** Systems (depth first from the Sun) with their matching satellites and Lagrange points. */
	const groups = $derived.by((): Group[] => {
		const root = mapState.index?.root;
		if (!root) return [];
		const q = query.trim().toLowerCase();
		const found = (name: string) => !q || name.toLowerCase().includes(q);
		const out: Group[] = [];
		const visit = (center: MapBody) => {
			const sats = center.children
				.filter((b) => b.info.orbit)
				.sort(
					(a, b) =>
						ORDER[a.info.bodyType] - ORDER[b.info.bodyType] ||
						a.info.orbit!.semiMajorAxisKm - b.info.orbit!.semiMajorAxisKm
				);
			const rows: Row[] = [];
			// The Sun has no type filter: the map always shows its system's center.
			if (center === root && found(center.info.entity.name))
				rows.push({ entity: center.info.entity, type: 'star', color: bodyColor(center.info) });
			for (const b of sats)
				if (found(b.info.entity.name) && mapState.matches(b.info.bodyType, b.info))
					rows.push({ entity: b.info.entity, type: b.info.bodyType, color: bodyColor(b.info) });
			if (mapState.matches('lagrange', null))
				for (const b of sats)
					for (const p of b.points)
						if (found(p.entity.name))
							rows.push({ entity: p.entity, type: 'lagrange', color: TYPE_COLORS.lagrange });
			if (rows.length) out.push({ center, rows });
			for (const b of sats) if (b.children.length) visit(b);
		};
		visit(root);
		return out;
	});
	const total = $derived(groups.reduce((n, g) => n + g.rows.length, 0));

	function openSystem(center: MapBody) {
		app.mapSystem = center === mapState.index?.root ? null : center.key;
		mapState.fitRequest++;
	}

	// Bring a newly selected row into view (selections also come from the map and search).
	let list = $state<HTMLElement | null>(null);
	$effect(() => {
		const key = app.selected;
		if (!key || !list) return;
		tick().then(() =>
			list?.querySelector(`[data-key="${CSS.escape(key)}"]`)?.scrollIntoView({ block: 'nearest' })
		);
	});

	const isSelected = (key: EntityKey) => app.selected === key;
</script>

<aside class="flex h-full min-h-0 flex-col border-r bg-card" aria-label="Bodies">
	<div class="flex items-center justify-between gap-2 border-b px-3 py-2">
		<h2 class="font-semibold">Bodies</h2>
		<Button
			variant="ghost"
			size="icon"
			class="size-8"
			aria-label="Close bodies pane"
			onclick={() => (mapState.paneOpen = false)}><X /></Button
		>
	</div>

	<div class="border-b p-3">
		<div class="relative">
			<Search
				class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
			/>
			<Input
				type="search"
				class="pl-8"
				placeholder="Search bodies…"
				aria-label="Search bodies"
				bind:value={query}
			/>
		</div>
	</div>

	<!-- Filters and list scroll together, so open filters don't squeeze the list. -->
	<ScrollArea class="min-h-0 flex-1">
		<Collapsible.Root bind:open={filtersOpen} class="border-b px-3 py-2">
			<div class="flex items-center gap-1">
				<Collapsible.Trigger>
					{#snippet child({ props })}
						<Button {...props} variant="ghost" size="sm" class="-ml-2 flex-1 justify-start">
							<Funnel /> Filters
							{#if activeCount}<Badge class="h-5 min-w-5 px-1">{activeCount}</Badge>{/if}
							<ChevronDown class="ml-auto transition-transform {filtersOpen ? 'rotate-180' : ''}" />
						</Button>
					{/snippet}
				</Collapsible.Trigger>
				{#if mapState.filtersActive}
					<Button variant="ghost" size="sm" onclick={() => mapState.clearFilters()}>Clear</Button>
				{/if}
			</div>
			<Collapsible.Content>
				<div class="grid gap-4 pt-2 pb-2 text-sm">
					<div class="grid gap-2">
						<h3 class="text-xs font-medium text-muted-foreground">Types</h3>
						<div class="grid grid-cols-2 gap-x-3 gap-y-2">
							{#each ITEM_TYPES as type (type)}
								<Label class="flex items-center gap-2 font-normal">
									<Checkbox
										aria-label={TYPE_LABELS[type]}
										checked={mapState.types.includes(type)}
										onCheckedChange={(on) => setType(type, on)}
									/>
									<!-- Planets each have their own color. -->
									{@render marker(type, type === 'planet' ? '' : TYPE_COLORS[type])}
									{TYPE_LABELS[type]}
								</Label>
							{/each}
						</div>
					</div>
					<div class="flex items-center justify-between gap-4">
						<Label for="map-hab-sites" class="font-medium">Has hab sites</Label>
						<Switch id="map-hab-sites" bind:checked={mapState.habSitesOnly} />
					</div>
					<div class="grid gap-2">
						<h3 class="text-xs font-medium text-muted-foreground">Minimum resources</h3>
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
							<span class="font-medium">Hide non-matching on the map</span>
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
					<p class="text-xs text-muted-foreground">
						Major bodies use their own colors; others are colored by type. Marker sizes are
						illustrative, not to scale.
					</p>
				</div>
			</Collapsible.Content>
		</Collapsible.Root>

		<p class="px-3 pt-2 text-xs text-muted-foreground" role="status">
			{total === 1 ? '1 result' : `${total} results`}
		</p>
		<div bind:this={list} class="pb-3">
			{#each groups as group (group.center.key)}
				<section class="pt-2" aria-label="{group.center.info.entity.name} system">
					<div class="flex items-center justify-between gap-2 px-3 py-1">
						<h3 class="truncate text-xs font-medium text-muted-foreground">
							{group.center.info.entity.name} system
						</h3>
						{#if group.center.key !== (app.mapSystem ?? mapState.index?.root?.key)}
							<Button
								variant="ghost"
								size="sm"
								class="h-6 px-2 text-xs"
								aria-label="Open the {group.center.info.entity.name} system on the map"
								onclick={() => openSystem(group.center)}><LogIn /> Open</Button
							>
						{/if}
					</div>
					<ul>
						{#each group.rows as row (row.entity.key)}
							<li>
								<button
									type="button"
									data-key={row.entity.key}
									aria-current={isSelected(row.entity.key) ? 'true' : undefined}
									class="flex w-full items-center gap-2 px-3 py-1 text-left text-sm outline-none hover:bg-accent hover:text-accent-foreground focus-visible:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset aria-[current]:bg-primary/10 aria-[current]:font-medium"
									onclick={() => app.navigate(row.entity)}
								>
									{@render marker(row.type, row.color)}
									<span class="min-w-0 flex-1 truncate">{row.entity.name}</span>
									<span class="shrink-0 text-xs text-muted-foreground">{TYPE_LABELS[row.type]}</span
									>
								</button>
							</li>
						{/each}
					</ul>
				</section>
			{:else}
				<p class="px-3 py-6 text-center text-sm text-muted-foreground">
					{mapState.index ? 'No bodies match.' : 'Loading…'}
				</p>
			{/each}
		</div>
	</ScrollArea>
</aside>

{#snippet marker(type: ItemType, color: string)}
	{#if type === 'lagrange'}
		<span aria-hidden="true" class="size-2 shrink-0 rotate-45 border" style="border-color: {color}"
		></span>
	{:else if !color}
		<span
			aria-hidden="true"
			class="size-2.5 shrink-0 rounded-full border border-muted-foreground bg-muted"
		></span>
	{:else}
		<span aria-hidden="true" class="size-2.5 shrink-0 rounded-full" style="background: {color}"
		></span>
	{/if}
{/snippet}
