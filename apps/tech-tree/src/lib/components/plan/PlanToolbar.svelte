<script lang="ts">
	// Toolbar groups of the plan page: the campaign menu (pick, create, import, rename, delete),
	// "Update from save" for imported campaigns, and the "Completed" filter of the full tree.
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import FileInput from '@lucide/svelte/icons/file-input';
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import CircleOff from '@lucide/svelte/icons/circle-off';
	import Pencil from '@lucide/svelte/icons/pencil';
	import Plus from '@lucide/svelte/icons/plus';
	import Route from '@lucide/svelte/icons/route';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import type { Campaign } from '#lib/api.ts';
	import { campaign } from '#lib/campaign.svelte.ts';
	import Pill, { pillItem, pillToggle, segmented } from '#lib/components/toolbar/Pill.svelte';
	import Tip from '#lib/components/toolbar/Tip.svelte';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import { settings } from '#lib/settings.svelte.ts';
	import { app } from '#lib/state.svelte.ts';
	import CampaignDialog from './CampaignDialog.svelte';
	import { planUi } from './plan-ui.svelte.ts';

	let renameOpen = $state(false);
	let renaming = $state.raw<Campaign | null>(null);
	let deleting = $state.raw<Campaign | null>(null);
	let updating = $state(false);

	const label = $derived(campaign.active?.name ?? 'No campaign');
	const scopeLabel = (c: Campaign) =>
		[
			app.meta?.scenarios.find((s) => s.name === c.scenario)?.label ?? c.scenario,
			app.meta?.factions.find((f) => f.name === c.faction)?.label ?? c.faction
		].join(' · ');
	const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

	async function updateFromSave() {
		updating = true;
		try {
			await campaign.updateFromSave();
		} finally {
			updating = false;
		}
	}

	/** Completed research of the active campaign: shown, or hidden/dimmed per the filter mode. */
	const completedTip = $derived(
		settings.showCompleted
			? `Completed research shown — click to ${settings.filterMode === 'hide' ? 'hide' : 'dim'} it`
			: `Completed research ${settings.filterMode === 'hide' ? 'hidden' : 'dimmed'} — click to show`
	);
</script>

<Pill aria-label="Campaign">
	<DropdownMenu.Root>
		<Tip
			text={campaign.active
				? `Campaign: ${campaign.active.name} (${scopeLabel(campaign.active)})`
				: 'Plan research for a campaign'}
		>
			<DropdownMenu.Trigger>
				{#snippet child({ props })}
					<Button
						{...props}
						variant="ghost"
						size="sm"
						class="{pillItem} w-64 justify-between gap-2 pr-2 font-normal {campaign.active
							? ''
							: 'text-muted-foreground'}"
						aria-label="Campaign: {label}"
					>
						<span class="flex min-w-0 items-center gap-1.5"
							><Route class="shrink-0" /><span class="truncate"
								><span class="font-medium">{label}</span>{#if campaign.active}<span
										class="text-muted-foreground"
									>
										{` · ${scopeLabel(campaign.active)}`}</span
									>{/if}</span
							></span
						>
						<ChevronDown class="shrink-0 opacity-50" />
					</Button>
				{/snippet}
			</DropdownMenu.Trigger>
		</Tip>
		<DropdownMenu.Content class="w-64">
			{#if campaign.list?.length}
				<DropdownMenu.Group>
					<DropdownMenu.GroupHeading>Campaigns</DropdownMenu.GroupHeading>
					<DropdownMenu.RadioGroup
						value={campaign.activeId ?? ''}
						onValueChange={(id) => id && campaign.activate(id)}
					>
						{#each campaign.list as c (c.id || 'new')}
							<DropdownMenu.RadioItem value={c.id} closeOnSelect disabled={!c.id}>
								<span class="grid min-w-0">
									<span class="truncate">{c.name}</span>
									<span class="truncate text-xs text-muted-foreground">{scopeLabel(c)}</span>
								</span>
							</DropdownMenu.RadioItem>
						{/each}
					</DropdownMenu.RadioGroup>
				</DropdownMenu.Group>
				<DropdownMenu.Separator />
			{/if}
			<DropdownMenu.Item onSelect={() => (planUi.creating = true)}>
				<Plus /> New campaign…
			</DropdownMenu.Item>
			<DropdownMenu.Item onSelect={() => campaign.importSaveFile()}>
				<FileInput /> Import from save…
			</DropdownMenu.Item>
			{#if campaign.active}
				{@const active = campaign.active}
				<DropdownMenu.Item
					onSelect={() => {
						renaming = active;
						renameOpen = true;
					}}
				>
					<Pencil /> Rename…
				</DropdownMenu.Item>
				<DropdownMenu.Item variant="destructive" onSelect={() => (deleting = active)}>
					<Trash2 /> Delete…
				</DropdownMenu.Item>
				<DropdownMenu.Separator />
				<DropdownMenu.Item onSelect={() => campaign.activate(null)}>
					<CircleOff /> Close campaign
				</DropdownMenu.Item>
			{/if}
		</DropdownMenu.Content>
	</DropdownMenu.Root>
	{#if campaign.active?.save}
		{@const save = campaign.active.save}
		<Tip
			text="Update from save: reload completed research from {fileName(
				save.path
			)} (saved {save.gameDate})"
		>
			<Button
				variant="ghost"
				size="sm"
				class="{pillItem} gap-1.5 font-normal"
				disabled={updating}
				onclick={updateFromSave}
			>
				<RefreshCw class={updating ? 'animate-spin' : ''} /> Update from save
			</Button>
		</Tip>
	{/if}
</Pill>

{#if campaign.active && app.planView === 'full'}
	<Pill aria-label="Campaign progress">
		<ToggleGroup.Root
			type="multiple"
			size="sm"
			class={segmented}
			bind:value={
				() => (settings.showCompleted ? ['completed'] : []),
				(value: string[]) => (settings.showCompleted = value.includes('completed'))
			}
			aria-label="Campaign progress"
		>
			<Tip text={completedTip}>
				<ToggleGroup.Item
					value="completed"
					aria-label="Show completed research"
					class="{pillToggle} focus-visible:outline-none"
				>
					<CircleCheck /> Completed
				</ToggleGroup.Item>
			</Tip>
		</ToggleGroup.Root>
	</Pill>
{/if}

<CampaignDialog bind:open={planUi.creating} />
<CampaignDialog bind:open={renameOpen} {renaming} />

<AlertDialog.Root
	open={deleting !== null}
	onOpenChange={(next) => {
		if (!next) deleting = null;
	}}
>
	<AlertDialog.Content>
		{#if deleting}
			{@const doomed = deleting}
			<AlertDialog.Header>
				<AlertDialog.Title>Delete campaign “{doomed.name}”?</AlertDialog.Title>
				<AlertDialog.Description>
					Its marked research, goals and choices are removed for good.
				</AlertDialog.Description>
			</AlertDialog.Header>
			<AlertDialog.Footer>
				<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
				<AlertDialog.Action
					variant="destructive"
					onclick={() => {
						deleting = null;
						void campaign.remove(doomed.id);
					}}>Delete</AlertDialog.Action
				>
			</AlertDialog.Footer>
		{/if}
	</AlertDialog.Content>
</AlertDialog.Root>
