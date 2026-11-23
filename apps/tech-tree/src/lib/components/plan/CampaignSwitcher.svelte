<script lang="ts">
	// Toolbar group of the research planner: the plan panel toggle and a menu to pick, create,
	// rename and delete campaigns. Sits outermost left, on the plan panel's side.
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import FileInput from '@lucide/svelte/icons/file-input';
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import CircleOff from '@lucide/svelte/icons/circle-off';
	import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
	import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';
	import Pencil from '@lucide/svelte/icons/pencil';
	import Plus from '@lucide/svelte/icons/plus';
	import Route from '@lucide/svelte/icons/route';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { api, type Campaign } from '#lib/api.ts';
	import { campaign } from '#lib/campaign.svelte.ts';
	import Pill, { pillIcon, pillItem } from '#lib/components/toolbar/Pill.svelte';
	import Tip from '#lib/components/toolbar/Tip.svelte';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import { app } from '#lib/state.svelte.ts';
	import CampaignDialog from './CampaignDialog.svelte';
	import { planUi } from './plan-ui.svelte.ts';

	let renameOpen = $state(false);
	let renaming = $state.raw<Campaign | null>(null);
	let deleting = $state.raw<Campaign | null>(null);

	const label = $derived(campaign.active?.name ?? 'No campaign');

	/** Pick a save file (starting in the game's save folder) and import it into a new campaign. */
	async function importSave() {
		const dir = await api.defaultSaveDir().catch(() => null);
		const path = await api.pickSaveFile('Import a Terra Invicta save', dir ?? undefined);
		if (path) await campaign.importSave(path);
	}
	const scopeLabel = (c: Campaign) =>
		[
			app.meta?.scenarios.find((s) => s.name === c.scenario)?.label ?? c.scenario,
			app.meta?.factions.find((f) => f.name === c.faction)?.label ?? c.faction
		].join(' · ');
</script>

<Pill aria-label="Research plan" class="gap-1">
	<Tip text={planUi.panelOpen ? 'Hide plan panel' : 'Show plan panel'}>
		<Button
			variant="ghost"
			size="icon"
			class={pillIcon}
			aria-label="Plan panel"
			aria-pressed={planUi.panelOpen}
			onclick={() => planUi.togglePanel()}
		>
			{#if planUi.panelOpen}<PanelLeftClose />{:else}<PanelLeftOpen />{/if}
		</Button>
	</Tip>
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
						class="{pillItem} w-44 justify-between gap-2 pr-2 font-normal {campaign.active
							? ''
							: 'text-muted-foreground'}"
						aria-label="Campaign: {label}"
					>
						<span class="flex min-w-0 items-center gap-1.5"
							><Route class="shrink-0" /><span class="truncate">{label}</span></span
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
			<DropdownMenu.Item onSelect={importSave}>
				<FileInput /> Import from save…
			</DropdownMenu.Item>
			{#if campaign.active}
				{@const active = campaign.active}
				{#if active.save}
					<DropdownMenu.Item onSelect={() => campaign.updateFromSave()}>
						<RefreshCw /> Update from save
					</DropdownMenu.Item>
				{/if}
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
					<CircleOff /> Stop planning
				</DropdownMenu.Item>
			{/if}
		</DropdownMenu.Content>
	</DropdownMenu.Root>
</Pill>

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
