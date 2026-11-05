<script lang="ts">
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
	const segment = `${pillToggle} h-[30px] rounded-none border-0 first:pl-3 last:border-l last:pr-3 focus-visible:ring-inset focus-visible:ring-offset-0`;
	/** Category toggles: the icon itself shows the state (in color while shown), not a fill. */
	const categoryToggle = `${pillItem} size-8 min-w-8 border-0 px-0 hover:bg-accent/60 data-[state=on]:bg-transparent data-[state=on]:hover:bg-accent/60 focus-visible:ring-offset-0`;

	const disabled = $derived(app.mode !== 'full');
	const kinds = $derived<Kind[]>([
		...(app.showTechs ? ['techs' as const] : []),
		...(app.showProjects ? ['projects' as const] : [])
	]);

	function setKinds(value: string[]) {
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

<Pill aria-label="Research kinds">
	<!-- One segmented control: the bordered group is as tall as the other toolbar controls. -->
	<ToggleGroup.Root
		type="multiple"
		size="sm"
		variant="outline"
		spacing={0}
		{disabled}
		bind:value={() => kinds, setKinds}
		class="overflow-hidden rounded-md border border-input data-[size=sm]:rounded-md"
		aria-label="Research kinds"
	>
		<ToggleGroup.Item value="techs" aria-label="Show techs" class={segment}>
			<FlaskConical /> Techs
		</ToggleGroup.Item>
		<ToggleGroup.Item value="projects" aria-label="Show projects" class={segment}>
			<Wrench /> Projects
		</ToggleGroup.Item>
	</ToggleGroup.Root>
</Pill>

<Pill aria-label="Categories">
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
			<Tip text="{CATEGORIES[category].label}{on ? '' : ' (hidden)'}. Shift+click: only this">
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
						class={['transition', on ? '' : 'opacity-35 grayscale'].join(' ')}
					/>
				</ToggleGroup.Item>
			</Tip>
		{/each}
	</ToggleGroup.Root>
	{#if app.categories.length}
		<Button
			variant="ghost"
			size="sm"
			class="{pillItem} text-muted-foreground"
			{disabled}
			onclick={() => (app.categories = [])}
		>
			Show all
		</Button>
	{/if}
</Pill>

<Pill aria-label="Effects filter">
	<PropertyFilter {disabled} />
</Pill>
