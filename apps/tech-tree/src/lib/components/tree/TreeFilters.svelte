<script lang="ts">
	import { tick } from 'svelte';
	import Funnel from '@lucide/svelte/icons/funnel';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import Check from '@lucide/svelte/icons/check';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import FlaskConical from '@lucide/svelte/icons/flask-conical';
	import Wrench from '@lucide/svelte/icons/wrench';
	import type { TechCategory } from '$lib/api';
	import { CATEGORIES, CATEGORY_ORDER } from '$lib/categories';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import { app } from '$lib/state.svelte';
	import Pill, { pillItem, pillToggle } from '$lib/components/toolbar/Pill.svelte';
	import Tip from '$lib/components/toolbar/Tip.svelte';
	import CategoryIcon from './CategoryIcon.svelte';
	import PropertyFilter from './PropertyFilter.svelte';

	type Kind = 'techs' | 'projects';

	const segment = `${pillToggle} focus-visible:outline-none focus-visible:ring-2! focus-visible:ring-ring! focus-visible:ring-offset-2 focus-visible:ring-offset-background`;
	let techsButton = $state<HTMLButtonElement | null>(null);
	/** A persistent underline distinguishes shown categories from hidden ones. */
	const categoryToggle = `${pillItem} group/category relative disabled:opacity-100 size-8 min-w-8 border-0 px-0 hover:bg-accent/60 data-[state=on]:bg-transparent data-[state=on]:hover:bg-accent/60 focus-visible:ring-offset-0`;

	const collapsedReason = 'Filters apply to the full tree. Click to return to the full tree.';
	const componentId = $props.id();
	const descriptionId = `${componentId}-filters-description`;
	const collapsed = $derived(app.mode !== 'full');
	const activeCount = $derived(
		Number(!app.showTechs) +
			Number(!app.showProjects) +
			app.categories.length +
			app.properties.length
	);
	const kinds = $derived<Kind[]>([
		...(app.showTechs ? ['techs' as const] : []),
		...(app.showProjects ? ['projects' as const] : [])
	]);

	function setKinds(value: string[]) {
		if (value.length === 0) return;
		app.showTechs = value.includes('techs');
		app.showProjects = value.includes('projects');
	}

	// `app.categories` empty means every category is shown; the toggles show what is on screen.
	const shown = $derived<TechCategory[]>(
		app.categories.length ? app.categories : [...CATEGORY_ORDER]
	);
	/** Shift held on the last press of a category toggle: show only that category. */
	let solo: TechCategory | null = null;

	function setShown(value: string[]) {
		const pressed = solo;
		solo = null;
		if (pressed) {
			// Shift on the only shown category brings the others back.
			const only = app.categories.length === 1 && app.categories[0] === pressed;
			app.categories = only ? [] : [pressed];
			return;
		}
		// Hiding every category would leave an empty tree: keep the last one on.
		if (value.length === 0) return;
		const next = CATEGORY_ORDER.filter((c) => value.includes(c));
		app.categories = next.length === CATEGORY_ORDER.length ? [] : next;
	}

	function press(category: TechCategory, e: PointerEvent | KeyboardEvent) {
		solo = e.shiftKey ? category : null;
	}
</script>

{#if collapsed}
	<Pill aria-label="Tree filters">
		<Tip text={collapsedReason}>
			<Button
				variant="ghost"
				size="sm"
				class="{pillItem} gap-1.5 px-2 text-xs @max-[1800px]:size-8 @max-[1800px]:px-0 @max-[1800px]:has-[>svg]:px-0"
				aria-label={`Full tree to filter${activeCount ? ` · ${activeCount}` : ''}`}
				onclick={async () => {
					app.showFullTree(app.selected ?? app.root);
					// Focusing a filtered-out node makes the canvas clear every filter.
					// Keep the saved filters when returning through this control.
					if (app.filtersActive) app.focus = null;
					await tick();
					techsButton?.focus();
				}}
			>
				<Funnel /> <span class="@max-[1800px]:hidden">Full tree to filter</span>
				{#if activeCount}<Badge class="h-4 min-w-4 justify-center px-1 tabular-nums @max-[1800px]:hidden"
						>{activeCount}</Badge
					>{/if}
			</Button>
		</Tip>
	</Pill>
{:else}
	<span id={descriptionId} class="sr-only"
		>Select one or both research kinds. At least one kind stays shown.</span
	>
	<Pill aria-label="Research kinds">
		<ToggleGroup.Root
			type="multiple"
			size="sm"
			spacing={0.5}
			bind:value={() => kinds, setKinds}
			aria-label="Research kinds"
		>
			<Tip text="Show techs. At least one kind stays shown.">
				<ToggleGroup.Item
					bind:ref={techsButton}
					value="techs"
					aria-label="Show techs"
					aria-describedby={descriptionId}
					class={segment}
				>
					{#if app.showTechs}<Check />{:else}<FlaskConical />{/if} Techs
				</ToggleGroup.Item>
			</Tip>
			<Tip text="Show projects. At least one kind stays shown.">
				<ToggleGroup.Item
					value="projects"
					aria-label="Show projects"
					aria-describedby={descriptionId}
					class={segment}
				>
					{#if app.showProjects}<Check />{:else}<Wrench />{/if} Projects
				</ToggleGroup.Item>
			</Tip>
		</ToggleGroup.Root>
	</Pill>

	<Pill aria-label="Categories">
		<ToggleGroup.Root
			type="multiple"
			size="sm"
			spacing={0.5}
			bind:value={() => shown, setShown}
			aria-label="Categories shown"
		>
			{#each CATEGORY_ORDER as category (category)}
				{@const on = shown.includes(category)}
				<Tip
					text={`${CATEGORIES[category].label} — ${on ? 'shown. Click to hide' : 'hidden. Click to show'} · Shift+click: ${shown.length === 1 && on ? 'show all' : 'only this'}`}
				>
					<ToggleGroup.Item
						value={category}
						aria-label={CATEGORIES[category].label}
						class={categoryToggle}
						onpointerdown={(e: PointerEvent) => press(category, e)}
						onkeydown={(e: KeyboardEvent) => press(category, e)}
					>
						<CategoryIcon
							{category}
							size={22}
							class={on
								? 'transition'
								: 'opacity-75 saturate-[.6] transition group-hover/category:opacity-100 group-hover/category:saturate-100'}
						/>
						<span
							aria-hidden="true"
							class="absolute bottom-0.5 h-0.5 w-3 rounded-full bg-foreground {on
								? ''
								: 'invisible'}"
						></span>
					</ToggleGroup.Item>
				</Tip>
			{/each}
		</ToggleGroup.Root>
		<Tip text="Show all categories">
			<Button
				variant="ghost"
				size="icon"
				class="{pillItem} size-8 px-0 text-muted-foreground disabled:opacity-100 {app.categories
					.length
					? ''
					: 'invisible'}"
				aria-label="Show all categories"
				onclick={() => (app.categories = [])}
			>
				<RotateCcw />
			</Button>
		</Tip>
	</Pill>

	<Pill aria-label="Effects filter">
		<PropertyFilter />
	</Pill>
{/if}
