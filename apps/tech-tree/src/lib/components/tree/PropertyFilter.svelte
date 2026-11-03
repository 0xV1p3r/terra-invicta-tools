<script lang="ts">
	import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
	import { api, type PropertyGroup, type PropertyInfo } from '$lib/api';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Command from '$lib/components/ui/command/index.js';
	import * as Popover from '$lib/components/ui/popover/index.js';
	import { app } from '$lib/state.svelte';

	let { disabled = false }: { disabled?: boolean } = $props();

	const GROUPS: { group: PropertyGroup; heading: string }[] = [
		{ group: 'effect', heading: 'Effects' },
		{ group: 'ability', heading: 'Abilities' },
		{ group: 'unlock', heading: 'Unlocks' }
	];

	let open = $state(false);
	let catalog = $state.raw<PropertyInfo[]>([]);
	let error = $state<string | null>(null);

	const selected = $derived(new Set(app.properties));
	const byGroup = $derived(
		GROUPS.map((g) => ({ ...g, items: catalog.filter((p) => p.group === g.group) })).filter(
			(g) => g.items.length > 0
		)
	);

	// The catalog (labels, counts) depends on the scope; drop selections that no longer exist.
	$effect(() => {
		const scope = $state.snapshot(app.scope);
		let stale = false;
		error = null;
		api.getProperties(scope).then(
			(list) => {
				if (stale) return;
				catalog = list;
				const ids = new Set(list.map((p) => p.id));
				if (app.properties.some((p) => !ids.has(p))) {
					app.properties = app.properties.filter((p) => ids.has(p));
				}
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
				variant="outline"
				size="sm"
				aria-label="Filter by effect or unlock{app.properties.length
					? ` (${app.properties.length} selected)`
					: ''}"
			>
				<SlidersHorizontal /> Effects
				{#if app.properties.length}
					<Badge class="h-4 min-w-4 px-1 tabular-nums">{app.properties.length}</Badge>
				{/if}
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-[28rem] p-0" align="start">
		<Command.Root class="rounded-lg!" {filter}>
			<Command.Input placeholder="Filter by effect, e.g. control point, mining…" />
			<div
				class="flex items-center justify-between border-b px-3 py-1.5 text-xs text-muted-foreground"
			>
				<span>Shows research with <em>any</em> selected property</span>
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
			<Command.List class="max-h-96">
				{#if error}
					<p class="p-4 text-sm text-destructive">{error}</p>
				{:else}
					<Command.Empty>No matching properties.</Command.Empty>
				{/if}
				{#each byGroup as g (g.group)}
					<Command.Group heading={g.heading}>
						{#each g.items as p (p.id)}
							<Command.Item
								value={p.id}
								keywords={[p.label, p.example ?? '']}
								data-checked={selected.has(p.id)}
								aria-checked={selected.has(p.id)}
								onSelect={() => toggle(p.id)}
							>
								<div class="flex min-w-0 flex-1 flex-col">
									<span class="truncate">{p.label}</span>
									{#if p.example}
										<span class="truncate text-xs text-muted-foreground" title={p.example}
											>{p.example}</span
										>
									{/if}
								</div>
								<span class="text-xs text-muted-foreground tabular-nums">{p.count}</span>
							</Command.Item>
						{/each}
					</Command.Group>
				{/each}
			</Command.List>
		</Command.Root>
	</Popover.Content>
</Popover.Root>
