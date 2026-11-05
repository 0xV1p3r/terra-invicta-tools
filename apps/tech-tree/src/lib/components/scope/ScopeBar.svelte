<script lang="ts">
	import { app } from '$lib/state.svelte';
	import { assetUrl } from '$lib/api';
	import * as Select from '$lib/components/ui/select';
	import { Badge } from '$lib/components/ui/badge';
	import Pill, { pillItem } from '$lib/components/toolbar/Pill.svelte';
	import PillDivider from '$lib/components/toolbar/PillDivider.svelte';
	// Borderless triggers: the pill already frames them.
	const trigger = `${pillItem} border-0 bg-transparent shadow-none hover:bg-accent dark:bg-transparent dark:hover:bg-accent/50 data-[size=sm]:h-8 data-[size=sm]:rounded-full`;
	const all = '__all__';
	let listed = $derived(app.meta?.scenarios.filter((s) => s.listed) ?? []);
	let other = $derived(app.meta?.scenarios.filter((s) => !s.listed) ?? []);
	let scenarioLabel = $derived(
		app.meta?.scenarios.find((s) => s.name === app.scope.scenario)?.label ?? 'All scenarios'
	);
	let faction = $derived(app.meta?.factions.find((f) => f.name === app.scope.faction));
</script>

<Pill aria-label="Data scope">
	<Select.Root
		type="single"
		value={app.scope.scenario ?? all}
		disabled={!app.meta}
		onValueChange={(value) => {
			app.scope = { ...app.scope, scenario: value === all ? null : value };
		}}
	>
		<Select.Trigger size="sm" class={trigger} aria-label="Scenario"
			><span class="max-w-48 truncate">{scenarioLabel}</span></Select.Trigger
		>
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
	<PillDivider />
	<Select.Root
		type="single"
		value={app.scope.faction ?? all}
		disabled={!app.meta}
		onValueChange={(value) => {
			app.scope = { ...app.scope, faction: value === all ? null : value };
		}}
	>
		<Select.Trigger size="sm" class={trigger} aria-label="Faction"
			><span class="flex max-w-48 items-center gap-2 truncate"
				>{#if faction?.icon}<img
						src={assetUrl(faction.icon) ?? undefined}
						alt=""
						class="size-4 object-contain"
					/>{/if}{faction?.label ?? 'All factions'}</span
			></Select.Trigger
		>
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
