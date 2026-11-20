<script lang="ts">
	// Creates a campaign (name, scenario, faction) or renames one. A campaign's scenario and faction
	// are fixed once created: they define what can be researched.
	import { untrack } from 'svelte';
	import { assetUrl, type Campaign } from '#lib/api.ts';
	import { campaign } from '#lib/campaign.svelte.ts';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import { app } from '#lib/state.svelte.ts';

	let {
		open = $bindable(false),
		renaming = null
	}: {
		open?: boolean;
		/** Campaign to rename; null = create a new one. */
		renaming?: Campaign | null;
	} = $props();

	const id = $props.id();
	let name = $state('');
	let scenario = $state('');
	let faction = $state('');
	let saving = $state(false);
	let error = $state<string | null>(null);

	const listed = $derived(app.meta?.scenarios.filter((s) => s.listed) ?? []);
	const other = $derived(app.meta?.scenarios.filter((s) => !s.listed) ?? []);
	const scenarioInfo = $derived(app.meta?.scenarios.find((s) => s.name === scenario));
	const factionInfo = $derived(app.meta?.factions.find((f) => f.name === faction));
	const valid = $derived(!!name.trim() && (!!renaming || (!!scenarioInfo && !!factionInfo)));

	// Fresh fields on every open, defaulting to the current scope (only opening resets them).
	$effect(() => {
		if (open) untrack(reset);
	});
	function reset() {
		error = null;
		saving = false;
		if (renaming) {
			name = renaming.name;
			return;
		}
		name = '';
		scenario = app.scope.scenario ?? listed[0]?.name ?? '';
		faction = app.scope.faction ?? '';
	}

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		if (!valid || saving) return;
		saving = true;
		error = null;
		if (renaming) {
			await campaign.rename(renaming.id, name.trim());
			open = false;
		} else {
			const saved = await campaign.create(name.trim(), scenario, faction);
			if (saved) open = false;
			else error = campaign.error ?? 'Could not create the campaign.';
		}
		saving = false;
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md">
		<form class="grid gap-4" onsubmit={submit}>
			<Dialog.Header>
				<Dialog.Title>{renaming ? 'Rename campaign' : 'New campaign'}</Dialog.Title>
				<Dialog.Description>
					{#if renaming}
						Scenario and faction stay as they are.
					{:else}
						Plan research for one playthrough. Mark what you have researched and pick goals; the
						scenario and faction decide what can be researched.
					{/if}
				</Dialog.Description>
			</Dialog.Header>
			<div class="grid gap-2">
				<Label for="{id}-name">Name</Label>
				<Input
					id="{id}-name"
					bind:value={name}
					placeholder="e.g. Resistance, 2026 start"
					required
				/>
			</div>
			{#if !renaming}
				<div class="grid gap-2">
					<Label id="{id}-scenario">Scenario</Label>
					<Select.Root type="single" bind:value={scenario}>
						<Select.Trigger class="w-full" aria-labelledby="{id}-scenario"
							><span class="truncate">{scenarioInfo?.label ?? 'Choose a scenario'}</span
							></Select.Trigger
						>
						<Select.Content>
							<Select.Group>
								{#each listed as s (s.name)}
									<Select.Item value={s.name} label={s.label}
										>{s.label}{#if s.dlc}<Badge variant="secondary" class="px-1 py-0 text-[10px]"
												>DLC</Badge
											>{/if}</Select.Item
									>
								{/each}
							</Select.Group>
							{#if other.length}
								<Select.Separator />
								<Select.Group>
									<Select.GroupHeading>Other</Select.GroupHeading>
									{#each other as s (s.name)}
										<Select.Item value={s.name} label={s.label}>{s.label}</Select.Item>
									{/each}
								</Select.Group>
							{/if}
						</Select.Content>
					</Select.Root>
				</div>
				<div class="grid gap-2">
					<Label id="{id}-faction">Faction</Label>
					<Select.Root type="single" bind:value={faction}>
						<Select.Trigger class="w-full" aria-labelledby="{id}-faction"
							><span class="flex min-w-0 items-center gap-1.5"
								>{#if factionInfo?.icon}<img
										src={assetUrl(factionInfo.icon) ?? undefined}
										alt=""
										class="size-4 shrink-0 object-contain"
									/>{/if}<span class="truncate">{factionInfo?.label ?? 'Choose a faction'}</span
								></span
							></Select.Trigger
						>
						<Select.Content>
							<Select.Group>
								{#each app.meta?.factions ?? [] as f (f.name)}
									<Select.Item value={f.name} label={f.label}
										>{#if f.icon}<img
												src={assetUrl(f.icon) ?? undefined}
												alt=""
												class="size-4 object-contain"
											/>{/if}{f.label}</Select.Item
									>
								{/each}
							</Select.Group>
						</Select.Content>
					</Select.Root>
				</div>
			{/if}
			{#if error}<p class="text-sm text-destructive" role="alert">{error}</p>{/if}
			<Dialog.Footer>
				<Dialog.Close>
					{#snippet child({ props })}
						<Button {...props} type="button" variant="outline">Cancel</Button>
					{/snippet}
				</Dialog.Close>
				<Button type="submit" disabled={!valid || saving}>{renaming ? 'Rename' : 'Create'}</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
