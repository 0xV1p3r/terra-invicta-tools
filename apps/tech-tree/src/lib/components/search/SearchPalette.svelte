<script lang="ts">
	import { api, assetUrl, type SearchHit, type EntityKind, type TechCategory } from '$lib/api';
	import { app } from '$lib/state.svelte';
	import { KIND_LABELS } from '$lib/format';
	import * as ToggleGroup from '$lib/components/ui/toggle-group';
	import * as Command from '$lib/components/ui/command';
	import { Button } from '$lib/components/ui/button';
	import {
		Search,
		FlaskConical,
		Wrench,
		Sparkles,
		Building,
		Rocket,
		Briefcase,
		User,
		Flag,
		Target,
		Crosshair,
		BookOpen,
		File
	} from '@lucide/svelte';
	let open = $state(false);
	let query = $state('');
	let hits = $state<SearchHit[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let retry = $state(0);
	const searchLimit = 200;
	let typeFilter = $state('all');
	const filterTypes = ['All', 'Research', 'Equipment', 'Other'];
	const filteredHits = $derived(
		hits.filter(({ entity }) => {
			const group =
				entity.kind === 'tech' || entity.kind === 'project'
					? 'Research'
					: entity.kind === 'shipPart' || entity.kind === 'habModule'
						? 'Equipment'
						: 'Other';
			return typeFilter === 'all' || typeFilter === group;
		})
	);
	const icons = {
		tech: FlaskConical,
		project: Wrench,
		effect: Sparkles,
		habModule: Building,
		shipPart: Rocket,
		org: Briefcase,
		trait: User,
		faction: Flag,
		objective: Target,
		mission: Crosshair,
		codex: BookOpen,
		other: File
	} satisfies Record<EntityKind, typeof File>;
	const categories: Record<TechCategory, string> = {
		Energy: 'energy',
		InformationScience: 'information',
		LifeScience: 'life',
		Materials: 'materials',
		MilitaryScience: 'military',
		SocialScience: 'social',
		SpaceScience: 'space',
		Xenology: 'xenology'
	};
	$effect(() => {
		void retry;
		const text = query.trim();
		const scope = { ...app.scope };
		const enabled = open && !!app.meta;
		let stale = false;
		hits = [];
		error = null;
		loading = enabled && !!text;
		if (!enabled || !text) return;
		const timer = setTimeout(async () => {
			try {
				const results = await api.search(scope, text, searchLimit);
				if (!stale) hits = results;
			} catch (cause) {
				if (!stale) error = cause instanceof Error ? cause.message : String(cause);
			} finally {
				if (!stale) loading = false;
			}
		}, 150);
		return () => {
			stale = true;
			clearTimeout(timer);
		};
	});
	function readableLabel(value: string) {
		const words = value.replace(/([a-z])([A-Z])/g, '$1 $2').replace(/[_-]+/g, ' ');
		return words.charAt(0).toUpperCase() + words.slice(1);
	}
	function shortcut(event: KeyboardEvent) {
		if (
			(event.ctrlKey || event.metaKey) &&
			event.key.toLowerCase() === 'k' &&
			!event.shiftKey &&
			!event.repeat
		) {
			event.preventDefault();
			if (app.meta) open = !open;
		}
	}
	function select(hit: SearchHit) {
		open = false;
		app.navigate(hit.entity);
	}
</script>

<svelte:window onkeydown={shortcut} />
<Button
	variant="outline"
	size="sm"
	disabled={!app.meta}
	onclick={() => {
		open = true;
	}}
	aria-label="Search entities"
	aria-keyshortcuts="Control+k Meta+k"
	class="gap-2 rounded-full"
>
	<Search /> <span>Search…</span><kbd
		class="ml-4 rounded border px-1 text-[10px] text-muted-foreground">Ctrl K</kbd
	>
</Button>
<Command.Dialog
	bind:open
	shouldFilter={false}
	title="Search entities"
	description="Search technologies, projects, and game entities."
	class="sm:max-w-xl"
>
	<Command.Input bind:value={query} placeholder="Search entities…" aria-label="Search entities" />
	<ToggleGroup.Root
		type="single"
		size="sm"
		value={typeFilter}
		onValueChange={(value) => {
			if (value) typeFilter = value;
		}}
		aria-label="Filter search results by type"
		class="mx-3 my-2 flex-wrap"
		spacing={1}
	>
		{#each filterTypes as type (type)}
			<ToggleGroup.Item value={type === 'All' ? 'all' : type}>{type}</ToggleGroup.Item>
		{/each}
	</ToggleGroup.Root>
	<Command.List aria-busy={loading}>
		{#if error}
			<div class="flex flex-col items-start gap-3 px-4 py-6">
				<p role="alert" class="text-sm text-destructive">
					Could not search for “{query.trim()}”. Try again.
				</p>
				<Button variant="outline" size="sm" onclick={() => retry++}>Retry</Button>
			</div>
		{:else if loading}
			<div role="status" class="px-4 py-6 text-sm text-muted-foreground">Searching…</div>
		{:else if !filteredHits.length}
			<div role="status" class="px-4 py-6 text-center text-sm text-muted-foreground">
				{#if !query.trim()}
					Type to search
				{:else if hits.length >= searchLimit && typeFilter !== 'all'}
					No {typeFilter.toLowerCase()} matches among the first {searchLimit} results. Refine your search.
				{:else}
					{typeFilter === 'all' ? 'No results' : 'No results in this type'}
				{/if}
			</div>
		{:else}
			<Command.Group heading="Results">
				{#each filteredHits as hit (hit.entity.key)}
					{@const entity = hit.entity}
					{@const Icon = icons[entity.kind]}
					<Command.Item value={entity.key} onSelect={() => select(hit)}>
						{#if entity.icon}<img
								src={assetUrl(entity.icon) ?? undefined}
								alt=""
								class="size-5 shrink-0 object-contain"
							/>{:else}<Icon class="size-5 text-muted-foreground" />{/if}
						<span class="flex min-w-0 flex-1 flex-col"
							><span class="truncate">{entity.name}</span><span
								class="truncate text-xs text-muted-foreground"
								>{KIND_LABELS[entity.kind]}{entity.category
									? ` · ${readableLabel(entity.category)}`
									: entity.subtype
										? ` · ${readableLabel(entity.subtype)}`
										: ''}</span
							></span
						>
						{#if (entity.kind === 'tech' || entity.kind === 'project') && entity.category}
							<span
								class="size-2 shrink-0 rounded-full"
								style:background-color={`var(--category-${categories[entity.category]})`}
								title={entity.category}
								aria-label={entity.category}
							></span>
						{/if}
					</Command.Item>
				{/each}
			</Command.Group>
		{/if}
	</Command.List>
	{#if !loading && !error && hits.length >= searchLimit}
		<p role="status" class="border-t px-4 py-2 text-xs text-muted-foreground">
			Type filters apply to the first {searchLimit} matches. Refine your search to narrow results.
		</p>
	{/if}
</Command.Dialog>
