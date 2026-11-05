<script lang="ts">
	import FlaskConical from '@lucide/svelte/icons/flask-conical';
	import LayoutGrid from '@lucide/svelte/icons/layout-grid';
	import Wrench from '@lucide/svelte/icons/wrench';
	import type { TechCategory } from '$lib/api';
	import { CATEGORIES, CATEGORY_ORDER } from '$lib/categories';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Checkbox } from '$lib/components/ui/checkbox/index.js';
	import * as Popover from '$lib/components/ui/popover/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import { app } from '$lib/state.svelte';
	import Pill, { pillItem, pillToggle } from '$lib/components/toolbar/Pill.svelte';
	import PillDivider from '$lib/components/toolbar/PillDivider.svelte';
	import CategoryIcon from './CategoryIcon.svelte';
	import PropertyFilter from './PropertyFilter.svelte';

	type Kind = 'techs' | 'projects';

	/** Joined toggle: the group draws the outer border, the items only the divider between them. */
	const segment = `${pillToggle} h-[30px] rounded-none border-0 first:pl-3 last:border-l last:pr-3 focus-visible:ring-inset focus-visible:ring-offset-0`;

	const disabled = $derived(app.mode !== 'full');
	const kinds = $derived<Kind[]>([
		...(app.showTechs ? ['techs' as const] : []),
		...(app.showProjects ? ['projects' as const] : [])
	]);

	function setKinds(value: string[]) {
		app.showTechs = value.includes('techs');
		app.showProjects = value.includes('projects');
	}

	// `app.categories` empty means every category is shown; the checkboxes show what is on screen.
	const shown = (category: TechCategory) =>
		app.categories.length === 0 || app.categories.includes(category);

	function setShown(category: TechCategory, on: boolean) {
		const next = CATEGORY_ORDER.filter((c) => (c === category ? on : shown(c)));
		app.categories = next.length === CATEGORY_ORDER.length ? [] : next;
	}
</script>

<Pill aria-label="Tree filters">
	<!-- One segmented control: the bordered group is as tall as the other pill controls. -->
	<ToggleGroup.Root
		type="multiple"
		size="sm"
		variant="outline"
		spacing={0}
		{disabled}
		bind:value={() => kinds, setKinds}
		class="overflow-hidden rounded-full border border-input data-[size=sm]:rounded-full"
		aria-label="Research kinds"
	>
		<ToggleGroup.Item value="techs" aria-label="Show techs" class={segment}>
			<FlaskConical /> Techs
		</ToggleGroup.Item>
		<ToggleGroup.Item value="projects" aria-label="Show projects" class={segment}>
			<Wrench /> Projects
		</ToggleGroup.Item>
	</ToggleGroup.Root>

	<PillDivider />

	<Popover.Root>
		<Popover.Trigger {disabled}>
			{#snippet child({ props })}
				<Button
					{...props}
					variant="ghost"
					size="sm"
					class={pillItem}
					aria-label="Filter by category ({app.categories.length
						? `${app.categories.length} selected`
						: 'all shown'})"
				>
					<LayoutGrid /> Categories
					{#if app.categories.length}
						<span class="flex -space-x-1">
							{#each app.categories.slice(0, 3) as category (category)}
								<CategoryIcon {category} size={18} />
							{/each}
						</span>
						<Badge class="h-4 min-w-4 px-1 tabular-nums">{app.categories.length}</Badge>
					{:else}
						<span class="text-muted-foreground">All</span>
					{/if}
				</Button>
			{/snippet}
		</Popover.Trigger>
		<Popover.Content class="w-72 p-0" align="start">
			<div
				class="flex items-center justify-between gap-2 border-b px-3 py-1.5 text-xs text-muted-foreground"
			>
				<span>
					{app.categories.length
						? `${app.categories.length} of ${CATEGORY_ORDER.length} shown`
						: 'All categories shown'}
				</span>
				<Button
					variant="ghost"
					size="sm"
					class="h-6 px-2 text-xs"
					disabled={app.categories.length === 0}
					onclick={() => (app.categories = [])}
				>
					Show all
				</Button>
			</div>
			<ul class="p-1" aria-label="Categories">
				{#each CATEGORY_ORDER as category (category)}
					{@const info = CATEGORIES[category]}
					{@const id = `category-filter-${category}`}
					{@const on = shown(category)}
					<li class="group flex items-center gap-1 rounded-md pr-1 hover:bg-accent">
						<label
							for={id}
							class="flex min-w-0 flex-1 cursor-pointer items-center gap-2.5 py-1 pl-2"
						>
							<Checkbox
								{id}
								checked={on}
								disabled={on && app.categories.length === 1}
								onCheckedChange={(v) => setShown(category, v)}
							/>
							<CategoryIcon {category} size={22} class={on ? '' : 'opacity-40 grayscale'} />
							<span class="truncate">{info.label}</span>
						</label>
						<Button
							variant="ghost"
							size="sm"
							class="h-6 px-2 text-xs opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
							aria-label="Show only {info.label}"
							onclick={() => (app.categories = [category])}
						>
							Only
						</Button>
					</li>
				{/each}
			</ul>
		</Popover.Content>
	</Popover.Root>

	<PillDivider />

	<PropertyFilter {disabled} />
</Pill>
