<script lang="ts">
	// Header controls of the solar system map: focus on a major body (any of them; the map's hotbar
	// has the planets), distance scale.
	import Crosshair from '@lucide/svelte/icons/crosshair';
	import Pill, { pillItem, pillToggle, segmented } from '#lib/components/toolbar/Pill.svelte';
	import Tip from '#lib/components/toolbar/Tip.svelte';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import { settings, type MapScale } from '#lib/settings.svelte.ts';
	import BodyIcon from './BodyIcon.svelte';
	import { majors } from './majors.svelte';
	import type { MapBody } from './scene';

	const trigger = `${pillItem} w-36 justify-between gap-2 pr-2 data-[size=sm]:h-8 data-[size=sm]:rounded-md`;
	const SCALES: { value: MapScale; label: string; title: string }[] = [
		{
			value: 'log',
			label: 'Log',
			title: 'Logarithmic distances: inner and outer orbits both readable'
		},
		{ value: 'linear', label: 'Linear', title: 'True distance proportions' }
	];

	const focusedBody = $derived(majors.all.find((b) => b.key === majors.focused));

	let choice = $state('');
</script>

{#snippet item(b: MapBody)}
	<Select.Item value={b.key} label={b.info.entity.name}>
		<BodyIcon info={b.info} size={18} />
		{b.info.entity.name}
	</Select.Item>
{/snippet}

<Pill aria-label="Map view">
	<!-- An action, not a value: choosing the body already shown focuses it again. -->
	<Select.Root
		type="single"
		bind:value={choice}
		onValueChange={(v) => {
			if (v) majors.focus(v);
			queueMicrotask(() => (choice = ''));
		}}
	>
		<!-- The value is always reset, so the trigger is always a "placeholder": show the focused
		     body at full strength and keep the muted look for the "Focus…" fallback only. -->
		<Select.Trigger
			size="sm"
			class={focusedBody ? `${trigger} data-placeholder:text-foreground` : trigger}
			aria-label="Focus on a major body"
		>
			<span class="flex min-w-0 flex-1 items-center gap-2 text-left">
				{#if focusedBody}<BodyIcon info={focusedBody.info} size={16} />{:else}<Crosshair />{/if}
				<span class="truncate">{focusedBody?.info.entity.name ?? 'Focus…'}</span>
			</span>
		</Select.Trigger>
		<Select.Content>
			<Select.Group>
				<Select.GroupHeading>Sun and planets</Select.GroupHeading>
				{#each majors.planets as b (b.key)}{@render item(b)}{/each}
			</Select.Group>
			{#if majors.dwarfs.length}
				<Select.Separator />
				<Select.Group>
					<Select.GroupHeading>Dwarf planets</Select.GroupHeading>
					{#each majors.dwarfs as b (b.key)}{@render item(b)}{/each}
				</Select.Group>
			{/if}
		</Select.Content>
	</Select.Root>

	<ToggleGroup.Root
		type="single"
		size="sm"
		spacing={0.5}
		class={segmented}
		value={settings.mapScale}
		onValueChange={(v) => v && (settings.mapScale = v as MapScale)}
		aria-label="Distance scale"
	>
		{#each SCALES as sc (sc.value)}
			<Tip text={sc.title}>
				<ToggleGroup.Item value={sc.value} class={pillToggle}>{sc.label}</ToggleGroup.Item>
			</Tip>
		{/each}
	</ToggleGroup.Root>
</Pill>
