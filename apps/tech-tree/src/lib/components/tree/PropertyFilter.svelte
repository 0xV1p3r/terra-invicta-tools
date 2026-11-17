<script lang="ts" module>
	import type { Component } from 'svelte';
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import type { PropertyGroup, PropertyInfo } from '#lib/api.ts';

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
	import Check from '@lucide/svelte/icons/check';
	import { Command as CommandPrimitive } from 'bits-ui';
	import Tip from '#lib/components/toolbar/Tip.svelte';
	import Funnel from '@lucide/svelte/icons/funnel';
	import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
	import X from '@lucide/svelte/icons/x';
	import { api } from '#lib/api.ts';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Command from '#lib/components/ui/command/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import { SENTIMENT } from '#lib/effect-style.ts';
	import { app } from '#lib/state.svelte.ts';
	import { pillItem } from '#lib/components/toolbar/Pill.svelte';
	import PropertyIcon from './PropertyIcon.svelte';

	let { disabled = false }: { disabled?: boolean } = $props();

	const disabledReason = 'Filters apply to the full tree. Switch to Full tree to use them.';
	const componentId = $props.id();
	const descriptionId = `${componentId}-effects-description`;
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

	const triggerTip = $derived(
		chips.length
			? `Selected: ${chips
					.slice(0, 3)
					.map((chip) => chip.info?.label ?? chip.id)
					.join(', ')}${chips.length > 3 ? ` +${chips.length - 3} more` : ''}`
			: 'Filter by effects, abilities, or unlocks'
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

<span id={descriptionId} class="sr-only"
	>{disabled ? disabledReason : 'Show research matching any selected effect.'}</span
>
<Popover.Root bind:open>
	<Tip text={disabled ? disabledReason : triggerTip}>
		<Popover.Trigger {disabled}>
			{#snippet child({ props })}
				<Button
					{...props}
					variant="outline"
					size="sm"
					class="{pillItem} {disabled ? 'grayscale' : ''}"
					aria-describedby={descriptionId}
					aria-label="Filter by effect or unlock{app.properties.length
						? ` (${app.properties.length} selected)`
						: ''}"
				>
					<SlidersHorizontal /> Effects
					<Badge
						class="h-4 w-6 justify-center px-1 tabular-nums {app.properties.length
							? ''
							: 'invisible'}"
						aria-hidden={app.properties.length === 0}
					>
						{app.properties.length}
					</Badge>
				</Button>
			{/snippet}
		</Popover.Trigger>
	</Tip>
	<Popover.Content class="w-[30rem] p-0" align="start">
		<Command.Root
			class="rounded-lg! [&_[data-slot=command-input-wrapper]]:border-border [&_[data-slot=command-input-wrapper]]:shadow-none [&_[data-slot=command-input-wrapper]]:focus-within:rounded-md [&_[data-slot=command-input-wrapper]]:focus-within:outline-2 [&_[data-slot=command-input-wrapper]]:focus-within:-outline-offset-2 [&_[data-slot=command-input-wrapper]]:focus-within:outline-ring"
			{filter}
		>
			<Command.Input
				aria-label="Search effects"
				placeholder="Filter by effect, ability or unlock…"
			/>
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
							class="h-6 max-w-full gap-1 overflow-visible pr-0.5 pl-0.5 {chip.shown
								? ''
								: 'bg-muted text-muted-foreground'}"
							title={chip.shown ? undefined : 'No research in the filtered tree has this'}
						>
							{#if chip.info}
								<PropertyIcon property={chip.info} size={18} />
							{/if}
							<span class="truncate">{chip.info?.label ?? chip.id}</span>
							{#if !chip.shown}<span class="sr-only"
									>No research in the filtered tree has this property.</span
								>{/if}
							<Button
								variant="ghost"
								size="icon"
								class="grid size-6 shrink-0 place-items-center rounded-sm focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background focus-visible:outline-none"
								aria-label="Remove {chip.info?.label ?? chip.id}"
								onclick={() => toggle(chip.id)}
							>
								<X class="size-3" />
							</Button>
						</Badge>
					{/each}
				</div>
			{/if}
			<Command.List class="max-h-96" aria-multiselectable="true">
				<!-- bits only points the input's aria-activedescendant at the highlight inside a Viewport. -->
				<CommandPrimitive.Viewport>
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
								<!-- A persistent multi-select: aria-selected reports applied options (bits sets it on the
							     keyboard highlight, which it tracks by data-selected and aria-activedescendant). -->
								<Command.Item
									value={p.id}
									keywords={[p.label, p.example ?? '']}
									data-checked={selected.has(p.id)}
									class="gap-2.5 data-[checked=true]:bg-primary/10"
									onSelect={() => toggle(p.id)}
								>
									{#snippet child({ props })}
										<div {...props} aria-selected={selected.has(p.id) ? 'true' : 'false'}>
											<Check
												class="size-4 shrink-0 {selected.has(p.id) ? '' : 'invisible'}"
												aria-hidden="true"
											/>
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
										</div>
									{/snippet}
								</Command.Item>
							{/each}
						</Command.Group>
					{/each}
				</CommandPrimitive.Viewport>
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
