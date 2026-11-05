<script lang="ts">
	import { tick } from 'svelte';
	import Funnel from '@lucide/svelte/icons/funnel';
	import Undo2 from '@lucide/svelte/icons/undo-2';
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

	const componentId = $props.id();
	const descriptionId = `${componentId}-filters-description`;
	const collapsed = $derived(app.mode !== 'full');
	// `app.categories` lists the shown categories (empty: all), so count the hidden ones.
	const activeCount = $derived(
		Number(!app.showTechs) +
			Number(!app.showProjects) +
			(app.categories.length ? CATEGORY_ORDER.length - app.categories.length : 0) +
			app.properties.length
	);
	const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
	// Filters don't apply outside the full tree, so the collapsed control calls them saved.
	const savedSummary = $derived(
		[
			...(app.showTechs ? [] : ['techs hidden']),
			...(app.showProjects ? [] : ['projects hidden']),
			...(app.categories.length
				? [`${plural(CATEGORY_ORDER.length - app.categories.length, 'category')} hidden`]
				: []),
			...(app.properties.length ? [plural(app.properties.length, 'effect')] : [])
		].join(', ')
	);
	const collapsedReason = $derived(
		activeCount
			? `Back to the full tree to filter. ${plural(activeCount, 'saved filter')} ${activeCount === 1 ? 'applies' : 'apply'} again there: ${savedSummary}.`
			: 'Back to the full tree to filter. Filters apply to the full tree only.'
	);
	/**
	 * Container widths at which the collapsed control fits its label: Focus view has no
	 * Relationships group, so it has room much sooner. Literal classes so Tailwind finds them.
	 */
	const labelled = $derived(
		app.mode === 'focus'
			? {
					button: '@min-[1100px]:w-auto @min-[1100px]:gap-1.5 @min-[1100px]:px-2',
					label: '@min-[1100px]:inline',
					undo: '@min-[1100px]:hidden',
					badge: '@min-[1100px]:h-4 @min-[1100px]:min-w-4 @min-[1100px]:px-1 @min-[1100px]:text-xs'
				}
			: {
					button: '@min-[1800px]:w-auto @min-[1800px]:gap-1.5 @min-[1800px]:px-2',
					label: '@min-[1800px]:inline',
					undo: '@min-[1800px]:hidden',
					badge: '@min-[1800px]:h-4 @min-[1800px]:min-w-4 @min-[1800px]:px-1 @min-[1800px]:text-xs'
				}
	);
	const kinds = $derived<Kind[]>([
		...(app.showTechs ? ['techs' as const] : []),
		...(app.showProjects ? ['projects' as const] : [])
	]);

	/** Tooltip for a kind toggle: its state and what a click does; the last shown kind stays on. */
	function kindTip(label: string, on: boolean) {
		if (on && kinds.length === 1) return `${label} — at least one kind stays shown`;
		return on ? `${label} shown — click to hide` : `${label} hidden — click to show`;
	}

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
				class="{pillItem} gap-1 text-xs {activeCount
					? 'w-10 justify-start pr-1 pl-1.5'
					: 'w-8 px-0'} {labelled.button}"
				aria-label={`Full tree to filter${activeCount ? `, ${plural(activeCount, 'saved filter')}` : ''}`}
				onclick={async () => {
					app.showFullTree(app.selected ?? app.root);
					// Focusing a filtered-out node makes the canvas clear every filter.
					// Keep the saved filters when returning through this control.
					if (app.filtersActive) app.focus = null;
					await tick();
					techsButton?.focus();
				}}
			>
				<!-- Icon-only while the label doesn't fit: a return arrow on the funnel, the count beside it. -->
				<span class="relative inline-flex" aria-hidden="true"
					><Funnel /><Undo2
						class="absolute -bottom-1 -left-1.5 size-2.5! rounded-sm bg-muted stroke-3 {labelled.undo}"
					/></span
				>
				<span class="hidden {labelled.label}">Full tree filters</span>
				{#if activeCount}<Badge
						aria-hidden="true"
						class="h-3.5 min-w-3.5 justify-center px-0.5 text-[0.625rem] leading-none tabular-nums {labelled.badge}"
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
			<Tip text={kindTip('Techs', app.showTechs)}>
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
			<Tip text={kindTip('Projects', app.showProjects)}>
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
				{@const last = on && shown.length === 1}
				<!-- The last shown category can't be hidden (setShown keeps it on). -->
				<Tip
					text={`${CATEGORIES[category].label} — ${last ? 'at least one category stays shown' : on ? 'shown. Click to hide' : 'hidden. Click to show'} · Shift+click: ${last ? 'show all' : 'only this'}`}
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
		<!-- Subdued but focusable while every category is shown, so the slot never reads as a gap. -->
		<Tip text={app.categories.length ? 'Show all categories' : 'All categories shown'}>
			<Button
				variant="ghost"
				size="icon"
				class="{pillItem} size-8 px-0 text-muted-foreground aria-disabled:cursor-default aria-disabled:opacity-40 aria-disabled:hover:bg-transparent aria-disabled:hover:text-muted-foreground"
				aria-label="Show all categories"
				aria-disabled={!app.categories.length}
				onclick={() => {
					if (app.categories.length) app.categories = [];
				}}
			>
				<RotateCcw />
			</Button>
		</Tip>
	</Pill>

	<Pill aria-label="Effects filter">
		<PropertyFilter />
	</Pill>
{/if}
