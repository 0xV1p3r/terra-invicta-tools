<script lang="ts">
	import { app } from '$lib/state.svelte';
	import { assetUrl } from '$lib/api';
	import * as Select from '$lib/components/ui/select';
	import { Badge } from '$lib/components/ui/badge';
	import Pill, { pillItem } from '$lib/components/toolbar/Pill.svelte';
	import Tip from '$lib/components/toolbar/Tip.svelte';
	// Outlined triggers, like the other toolbar controls. A fixed width keeps the toolbar from
	// reflowing when a long scenario or faction name is chosen; a long value truncates, and the
	// tooltip carries it in full.
	const trigger = `${pillItem} justify-between gap-2 pr-2 data-[size=sm]:h-8 data-[size=sm]:rounded-md`;
	const all = '__all__';
	let listed = $derived(app.meta?.scenarios.filter((s) => s.listed) ?? []);
	let other = $derived(app.meta?.scenarios.filter((s) => !s.listed) ?? []);
	let scenarioLabel = $derived(
		app.meta?.scenarios.find((s) => s.name === app.scope.scenario)?.label ?? 'All scenarios'
	);
	let faction = $derived(app.meta?.factions.find((f) => f.name === app.scope.faction));
</script>

<Pill aria-label="Data scope" class="gap-2">
	<Select.Root
		type="single"
		value={app.scope.scenario ?? all}
		disabled={!app.meta}
		onValueChange={(value) => {
			app.scope = { ...app.scope, scenario: value === all ? null : value };
		}}
	>
		<Tip text="Scenario: {scenarioLabel}">
			<Select.Trigger size="sm" class="{trigger} w-44" aria-label="Scenario: {scenarioLabel}"
				><span class="truncate">{scenarioLabel}</span></Select.Trigger
			>
		</Tip>
		<Select.Content>
			<Select.Group>
				<Select.Item value={all} label="All scenarios">All scenarios</Select.Item>
				{#each listed as scenario (scenario.name)}
					<Select.Item value={scenario.name} label={scenario.label}
						>{scenario.label}{#if scenario.dlc}<Badge
								variant="secondary"
								class="px-1 py-0 text-[10px]">DLC</Badge
							>{/if}</Select.Item
					>
				{/each}
			</Select.Group>
			{#if other.length}
				<Select.Separator />
				<Select.Group
					><Select.GroupHeading>Other</Select.GroupHeading>
					{#each other as scenario (scenario.name)}
						<Select.Item value={scenario.name} label={scenario.label}
							>{scenario.label}{#if scenario.dlc}<Badge
									variant="secondary"
									class="px-1 py-0 text-[10px]">DLC</Badge
								>{/if}</Select.Item
						>
					{/each}
				</Select.Group>
			{/if}
		</Select.Content>
	</Select.Root>
	<Select.Root
		type="single"
		value={app.scope.faction ?? all}
		disabled={!app.meta}
		onValueChange={(value) => {
			app.scope = { ...app.scope, faction: value === all ? null : value };
		}}
	>
		<Tip text="Faction: {faction?.label ?? 'All factions'}">
			<Select.Trigger
				size="sm"
				class="{trigger} w-44"
				aria-label="Faction: {faction?.label ?? 'All factions'}"
				><span class="flex min-w-0 items-center gap-1.5"
					>{#if faction?.icon}<img
							src={assetUrl(faction.icon) ?? undefined}
							alt=""
							class="size-4 shrink-0 object-contain"
						/>{/if}<span class="truncate">{faction?.label ?? 'All factions'}</span></span
				></Select.Trigger
			>
		</Tip>
		<Select.Content
			><Select.Group>
				<Select.Item value={all} label="All factions">All factions</Select.Item>
				{#each app.meta?.factions ?? [] as f (f.name)}
					<Select.Item value={f.name} label={f.label}
						>{#if f.icon}<img
								src={assetUrl(f.icon) ?? undefined}
								alt=""
								class="size-4 object-contain"
							/>{/if}{f.label}</Select.Item
					>
				{/each}
			</Select.Group></Select.Content
		>
	</Select.Root>
</Pill>
