<script lang="ts">
	import FlaskConical from '@lucide/svelte/icons/flask-conical';
	import Wrench from '@lucide/svelte/icons/wrench';
	import type { TechCategory } from '$lib/api';
	import { CATEGORIES, CATEGORY_ORDER } from '$lib/categories';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import { app } from '$lib/state.svelte';
	import Pill from '$lib/components/toolbar/Pill.svelte';
	import CategoryIcon from './CategoryIcon.svelte';
	import PropertyFilter from './PropertyFilter.svelte';

	type Kind = 'techs' | 'projects';

	const disabled = $derived(app.mode !== 'full');
	const kinds = $derived<Kind[]>([
		...(app.showTechs ? ['techs' as const] : []),
		...(app.showProjects ? ['projects' as const] : [])
	]);

	function setKinds(value: string[]) {
		app.showTechs = value.includes('techs');
		app.showProjects = value.includes('projects');
	}
</script>

<Pill aria-label="Tree filters">
	<ToggleGroup.Root
		type="multiple"
		size="sm"
		variant="outline"
		{disabled}
		bind:value={() => kinds, setKinds}
		aria-label="Research kinds"
	>
		<ToggleGroup.Item value="techs" aria-label="Show techs">
			<FlaskConical /> Techs
		</ToggleGroup.Item>
		<ToggleGroup.Item value="projects" aria-label="Show projects">
			<Wrench /> Projects
		</ToggleGroup.Item>
	</ToggleGroup.Root>

	<ToggleGroup.Root
		type="multiple"
		size="sm"
		variant="outline"
		{disabled}
		bind:value={() => app.categories, (v) => (app.categories = v as TechCategory[])}
		aria-label="Categories (none selected = all)"
	>
		{#each CATEGORY_ORDER as category (category)}
			{@const info = CATEGORIES[category]}
			<Tooltip.Root>
				<Tooltip.Trigger>
					{#snippet child({ props })}
						<ToggleGroup.Item
							{...props}
							value={category}
							aria-label={info.label}
							class="category-toggle px-1.5"
							style="--cat: {info.cssVar}"
						>
							<CategoryIcon
								{category}
								size={20}
								class={app.categories.length && !app.categories.includes(category)
									? 'opacity-40 grayscale'
									: ''}
							/>
						</ToggleGroup.Item>
					{/snippet}
				</Tooltip.Trigger>
				<Tooltip.Content>{info.label}</Tooltip.Content>
			</Tooltip.Root>
		{/each}
	</ToggleGroup.Root>

	<PropertyFilter {disabled} />
</Pill>

<style>
	:global(.category-toggle[data-state='on']) {
		box-shadow: inset 0 -2px 0 var(--cat);
	}
</style>
