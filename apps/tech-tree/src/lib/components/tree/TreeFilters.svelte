<script lang="ts">
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

	/** Joined toggle: the group draws the outer border, the items only the divider between them. */
	const segment = `${pillToggle} h-[30px] rounded-none border-0 first:pl-3 last:border-l last:pr-3 disabled:opacity-100 focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background`;
	/** A persistent underline distinguishes shown categories from hidden ones. */
	const categoryToggle = `${pillItem} relative disabled:opacity-100 size-8 min-w-8 border-0 px-0 hover:bg-accent/60 data-[state=on]:bg-transparent data-[state=on]:hover:bg-accent/60 focus-visible:ring-offset-0`;

	const disabledReason = 'Filters apply to the full tree. Switch to Full tree to use them.';
	const componentId = $props.id();
	const descriptionId = `${componentId}-filters-description`;
	const disabled = $derived(app.mode !== 'full');
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

<span id={descriptionId} class="sr-only"
	>{disabled ? disabledReason : 'Select one or both research kinds.'}</span
>
<Pill aria-label="Research kinds" class={disabled ? 'opacity-50 grayscale' : ''}>
	<Tip
		text={disabled
			? disabledReason
			: 'Show techs, projects, or both. At least one kind stays shown.'}
	>
		<!-- One segmented control: the bordered group is as tall as the other toolbar controls. -->
		<ToggleGroup.Root
			type="multiple"
			size="sm"
			variant="outline"
			spacing={0}
			{disabled}
			bind:value={() => kinds, setKinds}
			aria-describedby={descriptionId}
			class="rounded-md border border-input data-[size=sm]:rounded-md"
			aria-label="Research kinds"
		>
			<ToggleGroup.Item
				value="techs"
				aria-label="Show techs"
				aria-describedby={descriptionId}
				class={segment}
			>
				{#if app.showTechs}<Check />{:else}<FlaskConical />{/if} Techs
			</ToggleGroup.Item>
			<ToggleGroup.Item
				value="projects"
				aria-label="Show projects"
				aria-describedby={descriptionId}
				class={segment}
			>
				{#if app.showProjects}<Check />{:else}<Wrench />{/if} Projects
			</ToggleGroup.Item>
		</ToggleGroup.Root>
	</Tip>
</Pill>

<Pill aria-label="Categories" class={disabled ? 'opacity-50 grayscale' : ''}>
	<ToggleGroup.Root
		type="multiple"
		size="sm"
		spacing={0.5}
		{disabled}
		bind:value={() => shown, setShown}
		aria-label="Categories shown"
	>
		{#each CATEGORY_ORDER as category (category)}
			{@const on = shown.includes(category)}
			<Tip
				text={disabled
					? disabledReason
					: `${CATEGORIES[category].label}${on ? '' : ' (hidden)'}. Shift+click: only this`}
			>
				<ToggleGroup.Item
					value={category}
					aria-label={CATEGORIES[category].label}
					aria-describedby={disabled ? descriptionId : undefined}
					class={categoryToggle}
					onpointerdown={(e: PointerEvent) => press(category, e)}
					onkeydown={(e: KeyboardEvent) => press(category, e)}
				>
					<CategoryIcon
						{category}
						size={22}
						class={on || disabled ? 'transition' : 'opacity-65 grayscale transition'}
					/>
					<span
						aria-hidden="true"
						class="absolute bottom-0.5 h-0.5 w-3 rounded-full bg-foreground {on ? '' : 'invisible'}"
					></span>
				</ToggleGroup.Item>
			</Tip>
		{/each}
	</ToggleGroup.Root>
	<Tip text={disabled ? disabledReason : 'Show all categories'}>
		<Button
			variant="ghost"
			size="icon"
			class="{pillItem} size-8 px-0 text-muted-foreground disabled:opacity-100 {app.categories
				.length
				? ''
				: 'invisible'}"
			{disabled}
			aria-label="Show all categories"
			aria-describedby={disabled ? descriptionId : undefined}
			onclick={() => (app.categories = [])}
		>
			<RotateCcw />
		</Button>
	</Tip>
</Pill>

<Pill aria-label="Effects filter">
	<PropertyFilter {disabled} />
</Pill>
