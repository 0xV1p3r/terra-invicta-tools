<script lang="ts">
	// Switches between the research tree and the solar system map (icon toggles in the view pill).
	import type { Component } from 'svelte';
	import Network from '@lucide/svelte/icons/network';
	import Orbit from '@lucide/svelte/icons/orbit';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import { app, type Page } from '#lib/state.svelte.ts';
	import { pillToggle, segmented } from './Pill.svelte';
	import Tip from './Tip.svelte';

	const PAGES: { value: Page; label: string; title: string; icon: Component }[] = [
		{ value: 'tree', label: 'Tech tree', title: 'Tech tree: techs and projects', icon: Network },
		{
			value: 'map',
			label: 'Solar system',
			title: 'Solar system: space bodies and their stats',
			icon: Orbit
		}
	];
	const toggle = `${pillToggle} w-[1.625rem] px-0 has-[>svg]:px-0`;
</script>

<ToggleGroup.Root
	type="single"
	size="sm"
	spacing={0.5}
	class={segmented}
	value={app.page}
	onValueChange={(v) => v && (app.page = v as Page)}
	aria-label="Page"
>
	{#each PAGES as p (p.value)}
		<Tip text={p.title}>
			<ToggleGroup.Item value={p.value} class={toggle} aria-label={p.label}>
				<p.icon />
			</ToggleGroup.Item>
		</Tip>
	{/each}
</ToggleGroup.Root>
