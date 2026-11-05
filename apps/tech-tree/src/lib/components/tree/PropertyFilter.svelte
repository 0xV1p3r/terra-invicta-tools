<script lang="ts" module>
	import type { Component } from 'svelte';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import type { PropertyGroup, PropertyInfo } from '$lib/api';

	const GROUPS: { group: PropertyGroup; heading: string }[] = [
		{ group: 'effect', heading: 'Effects' },
		{ group: 'ability', heading: 'Abilities' },
		{ group: 'unlock', heading: 'Unlocks' }
	];

	/** Arrow for the direction of a quantity effect (`effect:<context>:up|down`). */
	function directionIcon(p: PropertyInfo): Component | null {
		if (p.group !== 'effect') return null;
		return p.id.endsWith(':down') ? ArrowDown : ArrowUp;
	}
</script>

<script lang="ts">
	import Funnel from '@lucide/svelte/icons/funnel';
	import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
	import X from '@lucide/svelte/icons/x';
	import { api } from '$lib/api';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Command from '$lib/components/ui/command/index.js';
	import * as Popover from '$lib/components/ui/popover/index.js';
	import { SENTIMENT } from '$lib/effect-style';
	import { app } from '$lib/state.svelte';
	import { pillItem } from '$lib/components/toolbar/Pill.svelte';
	import PropertyIcon from './PropertyIcon.svelte';

	let { disabled = false }: { disabled?: boolean } = $props();

	let open = $state(false);
	/** Properties of the research the other filters leave on screen. */
	let catalog = $state.raw<PropertyInfo[]>([]);
	let error = $state<string | null>(null);
	/** Every property seen so far, so selections filtered out of view keep their label. */
	let known = $state.raw<ReadonlyMap<string, PropertyInfo>>(new Map());

	const selected = $derived(new Set(app.properties));
	const visible = $derived(new Set(catalog.map((p) => p.id)));
	const narrowed = $derived(!app.showTechs || !app.showProjects || app.categories.length > 0);
	const byGroup = $derived(
		GROUPS.map((g) => ({ ...g, items: catalog.filter((p) => p.group === g.group) })).filter(
			(g) => g.items.length > 0
		)
	);
	const chips = $derived(
		app.properties.map((id) => ({ id, info: known.get(id), shown: visible.has(id) }))
	);

	// Options follow the scope and the other tree filters (kinds, categories).
	$effect(() => {
		const scope = $state.snapshot(app.scope);
		// Not `app.filter`: selecting a property must not refetch the options.
		const filter = {
			techs: app.showTechs,
			projects: app.showProjects,
			categories: $state.snapshot(app.categories),
			properties: []
		};
		let stale = false;
		error = null;
		api.getProperties(scope, filter).then(
			(list) => {
				if (stale) return;
				catalog = list;
				known = new Map([...known, ...list.map((p) => [p.id, p] as const)]);
			},
			(e) => {
				if (!stale) error = String(e);
			}
		);
		return () => {
			stale = true;
		};
	});

	/** Every word of the query must occur in the label or example (fuzzy matching is too loose). */
	function filter(_value: string, search: string, keywords?: string[]) {
		const text = (keywords ?? []).join(' ').toLowerCase();
		return search
			.toLowerCase()
			.split(/\s+/)
			.every((word) => text.includes(word))
			? 1
			: 0;
	}

	function toggle(id: string) {
		app.properties = selected.has(id)
			? app.properties.filter((p) => p !== id)
			: [...app.properties, id];
	}
</script>

<Popover.Root bind:open>
	<Popover.Trigger {disabled}>
		{#snippet child({ props })}
			<Button
				{...props}
				variant="ghost"
				size="sm"
				class={pillItem}
				aria-label="Filter by effect or unlock{app.properties.length
					? ` (${app.properties.length} selected)`
					: ''}"
			>
				<SlidersHorizontal /> Effects
				{#if app.properties.length}
					<span class="flex -space-x-1">
						{#each chips.slice(0, 3) as chip (chip.id)}
							{#if chip.info}
								<PropertyIcon property={chip.info} size={18} />
							{/if}
						{/each}
					</span>
					<Badge class="h-4 min-w-4 px-1 tabular-nums">{app.properties.length}</Badge>
				{/if}
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-[30rem] p-0" align="start">
		<Command.Root class="rounded-lg!" {filter}>
			<Command.Input placeholder="Filter by effect, e.g. control point, mining…" />
			<div
				class="flex items-center justify-between gap-2 border-b px-3 py-1.5 text-xs text-muted-foreground"
			>
				<span class="flex min-w-0 items-center gap-1.5">
					{#if narrowed}
						<Funnel class="size-3.5 shrink-0 text-primary" aria-hidden="true" />
						<span class="truncate"
							>{catalog.length} options in the filtered tree · matches <em>any</em></span
						>
					{:else}
						<span class="truncate">{catalog.length} options · matches <em>any</em> selected</span>
					{/if}
				</span>
				<Button
					variant="ghost"
					size="sm"
					class="h-6 px-2 text-xs"
					disabled={app.properties.length === 0}
					onclick={() => (app.properties = [])}
				>
					Clear
				</Button>
			</div>
			{#if chips.length}
				<div class="flex flex-wrap gap-1 border-b px-2 py-1.5" aria-label="Selected properties">
					{#each chips as chip (chip.id)}
						<Badge
							variant="secondary"
							class="h-6 max-w-full gap-1 pr-0.5 pl-0.5 {chip.shown ? '' : 'opacity-50'}"
							title={chip.shown ? undefined : 'No research in the filtered tree has this'}
						>
							{#if chip.info}
								<PropertyIcon property={chip.info} size={18} />
							{/if}
							<span class="truncate">{chip.info?.label ?? chip.id}</span>
							<button
								type="button"
								class="grid size-5 place-items-center rounded-sm hover:bg-foreground/10 focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none"
								aria-label="Remove {chip.info?.label ?? chip.id}"
								onclick={() => toggle(chip.id)}
							>
								<X class="size-3" />
							</button>
						</Badge>
					{/each}
				</div>
			{/if}
			<Command.List class="max-h-96">
				{#if error}
					<p class="p-4 text-sm text-destructive">{error}</p>
				{:else}
					<Command.Empty>No matching properties.</Command.Empty>
				{/if}
				{#each byGroup as g (g.group)}
					<Command.Group value={g.group} heading="{g.heading} · {g.items.length}">
						{#each g.items as p (p.id)}
							{@const Direction = directionIcon(p)}
							{@const tone = SENTIMENT[p.sentiment] ?? SENTIMENT.neutral}
							<Command.Item
								value={p.id}
								keywords={[p.label, p.example ?? '']}
								data-checked={selected.has(p.id)}
								aria-checked={selected.has(p.id)}
								class="gap-2.5 data-[checked=true]:bg-primary/10"
								onSelect={() => toggle(p.id)}
							>
								<PropertyIcon property={p} />
								<div class="flex min-w-0 flex-1 flex-col">
									<span class="flex items-center gap-1">
										{#if Direction}
											<Direction
												class="size-3.5! {tone.text}"
												aria-label={tone.label || undefined}
											/>
										{/if}
										<span class="truncate">{p.label}</span>
									</span>
									{#if p.example}
										<span class="truncate text-xs text-muted-foreground" title={p.example}
											>{p.example}</span
										>
									{/if}
								</div>
								<span
									class="rounded-full bg-muted px-1.5 text-[11px] text-muted-foreground tabular-nums"
									title="{p.count} techs/projects"
								>
									{p.count}
								</span>
							</Command.Item>
						{/each}
					</Command.Group>
				{/each}
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
