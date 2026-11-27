<script lang="ts">
	// Side panel of the research planner: the active campaign's goals, what to research next, the
	// "any of" choices the plan made and the full research order. Steps are numbered like the
	// badges on the tree (position + 1). Costs are research points; with an imported save they use
	// the campaign's research speed and leave out points already put in.
	import ArrowDown from '@lucide/svelte/icons/arrow-down';
	import ArrowUp from '@lucide/svelte/icons/arrow-up';
	import Check from '@lucide/svelte/icons/check';
	import Circle from '@lucide/svelte/icons/circle';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import Ban from '@lucide/svelte/icons/ban';
	import FlaskConical from '@lucide/svelte/icons/flask-conical';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import Lock from '@lucide/svelte/icons/lock';
	import Plus from '@lucide/svelte/icons/plus';
	import Target from '@lucide/svelte/icons/target';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import Wrench from '@lucide/svelte/icons/wrench';
	import X from '@lucide/svelte/icons/x';
	import Hourglass from '@lucide/svelte/icons/hourglass';
	import type { Component } from 'svelte';
	import type { EntityKey, EntityRef, PlanChoice, PlanStatus } from '#lib/api.ts';
	import { campaign } from '#lib/campaign.svelte.ts';
	import EntityIcon from '#lib/components/detail/EntityIcon.svelte';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { ScrollArea } from '#lib/components/ui/scroll-area/index.js';
	import * as Select from '#lib/components/ui/select/index.js';
	import * as Tooltip from '#lib/components/ui/tooltip/index.js';
	import { formatCost, formatCostFull, readableKey } from '#lib/format.ts';
	import { app } from '#lib/state.svelte.ts';
	import {
		panelHeader,
		panelPadding,
		panelTitle,
		sectionHeading,
		statLabel,
		statTile,
		statValue,
		subHeading
	} from '#lib/components/styles.ts';
	import { planUi, researchKind } from './plan-ui.svelte.ts';

	const AUTO = '__auto__';
	/** Research-next rows shown per kind before "Show all". */
	const NEXT_PREVIEW = 5;

	const plan = $derived(campaign.plan);
	const active = $derived(campaign.active);
	const goals = $derived(active?.goals ?? []);

	/** Names of the research the plan mentions; finished goals only have a key. */
	const refs = $derived.by(() => {
		// Rebuilt whole on every plan, never mutated afterwards.
		// eslint-disable-next-line svelte/prefer-svelte-reactivity
		const out = new Map<EntityKey, EntityRef>();
		for (const s of plan?.steps ?? []) out.set(s.entity.key, s.entity);
		for (const r of plan?.unreachable ?? []) out.set(r.key, r);
		for (const c of plan?.choices ?? []) {
			out.set(c.node.key, c.node);
			for (const o of c.options) out.set(o.key, o);
		}
		return out;
	});
	const nameOf = (key: EntityKey) => refs.get(key)?.name ?? readableKey(key);

	const next = $derived.by(() => {
		const steps = (plan?.next ?? []).flatMap((key) => {
			const step = campaign.step(key);
			return step ? [step] : [];
		});
		return {
			tech: steps.filter((s) => s.entity.kind === 'tech'),
			project: steps.filter((s) => s.entity.kind === 'project')
		};
	});
	let showAll = $state<Record<'tech' | 'project', boolean>>({ tech: false, project: false });

	const STATUS: Record<PlanStatus, { label: string; icon: Component }> = {
		done: { label: 'Researched', icon: CircleCheck },
		available: { label: 'Available now', icon: Circle },
		notUnlocked: {
			label:
				'Prerequisites met, but the save has not unlocked it yet (unlock roll or other condition)',
			icon: Hourglass
		},
		locked: { label: 'Needs earlier steps', icon: Lock },
		unobtainable: { label: "Can't be researched in this scenario and faction", icon: Ban }
	};

	function navigate(key: EntityKey) {
		app.navigate({ key, kind: refs.get(key)?.kind ?? researchKind(key) });
	}

	function choiceLabel(c: PlanChoice) {
		const chosen = c.options.find((o) => o.key === c.chosen)?.name ?? nameOf(c.chosen);
		return c.picked ? chosen : `Auto: ${chosen}`;
	}
</script>

{#snippet section(title: string, hint?: string)}
	<h3 class="mt-5 mb-2 flex items-baseline justify-between gap-2 first:mt-0 {sectionHeading}">
		{title}
		{#if hint}<span class="font-normal text-faint">{hint}</span>{/if}
	</h3>
{/snippet}

{#snippet tipButton(label: string, onclick: () => void, Icon: Component, disabled = false)}
	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				<Button
					{...props}
					variant="ghost"
					size="icon"
					class="size-7 text-muted-foreground hover:text-foreground"
					aria-label={label}
					{disabled}
					{onclick}><Icon /></Button
				>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content>{label}</Tooltip.Content>
	</Tooltip.Root>
{/snippet}

{#snippet entityName(key: EntityKey)}
	{@const ref = refs.get(key)}
	<Button
		variant="link"
		class="h-auto min-w-0 justify-start gap-1.5 px-1 py-0.5 text-left font-medium text-foreground hover:bg-accent"
		onclick={() => navigate(key)}
	>
		{#if ref}<EntityIcon entity={ref} size={16} />{/if}
		<span class="truncate">{nameOf(key)}</span>
	</Button>
{/snippet}

{#snippet statusIcon(status: PlanStatus | undefined)}
	{@const info = status ? STATUS[status] : null}
	{#if info}
		<span title={info.label} class="grid size-4 shrink-0 place-items-center text-muted-foreground">
			<info.icon class="size-3.5 {status === 'available' ? 'fill-current text-foreground' : ''}" />
			<span class="sr-only">{info.label}</span>
		</span>
	{:else}
		<span class="size-4 shrink-0" aria-hidden="true"></span>
	{/if}
{/snippet}

<!-- Every list row (goals, research next, steps) starts the same way: step number, status icon. -->
{#snippet rowStart(number: number | undefined, status: PlanStatus | undefined)}
	<span class="w-7 shrink-0 text-right text-xs text-muted-foreground tabular-nums"
		>{number ?? ''}</span
	>
	{@render statusIcon(status)}
{/snippet}

<!-- …and ends the same way: markers, then the cost (right-aligned), then one row action slot
     (`noAction` keeps it empty), so costs line up in one column across sections. -->
{#snippet markers(goal: boolean, active = false)}
	{#if goal}<Target class="size-3.5 shrink-0 text-muted-foreground" aria-label="Goal" />{/if}
	{#if active}<LoaderCircle
			class="size-3.5 shrink-0 text-muted-foreground"
			aria-label="Being researched in the save"
		/>{/if}
{/snippet}

{#snippet noAction()}
	<span class="size-7 shrink-0" aria-hidden="true"></span>
{/snippet}

{#snippet cost(text: string, title: string)}
	<span class="min-w-10 shrink-0 text-right text-xs text-muted-foreground tabular-nums" {title}
		>{text}</span
	>
{/snippet}

<aside class="flex h-full min-h-0 flex-col border-r bg-card" aria-label="Research plan">
	<div class={panelHeader}>
		<div class="min-w-0">
			<h2 class="flex items-center gap-2 {panelTitle}">
				Plan
				{#if campaign.loading}<LoaderCircle
						class="size-3.5 animate-spin text-muted-foreground"
						aria-label="Updating plan"
					/>{/if}
			</h2>
			<!-- One subtitle line at most, so the header keeps the height of the other side panels. -->
			{#if active}
				{@const save = active.save}
				{@const from = save
					? `From save · ${save.gameDate}${save.saveVersion ? ` · v${save.saveVersion}` : ''}`
					: ''}
				<p
					class="truncate text-xs text-muted-foreground"
					title={save ? `${active.name}\n${from}\n${save.path}` : active.name}
				>
					{active.name}{#if save}<span> · {from}</span>{/if}
				</p>
			{/if}
		</div>
		<Button
			variant="ghost"
			size="icon"
			class="size-8"
			aria-label="Close plan panel"
			onclick={() => planUi.togglePanel(false)}><X /></Button
		>
	</div>
	{#if campaign.error}
		<div
			class="flex items-start gap-2 border-b {panelPadding} py-1.5 text-xs text-destructive"
			role="alert"
		>
			<TriangleAlert class="mt-px size-3.5 shrink-0" />
			<span class="min-w-0 flex-1 break-words">{campaign.error}</span>
			<button
				type="button"
				class="shrink-0 rounded-sm opacity-70 hover:opacity-100 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
				aria-label="Dismiss error"
				onclick={() => (campaign.error = null)}><X class="size-3.5" /></button
			>
		</div>
	{/if}
	<ScrollArea class="min-h-0 flex-1">
		<div class="{panelPadding} py-3 text-sm">
			{#if !active}
				<div class="grid gap-3 py-6 text-center text-muted-foreground">
					<p>
						Plan research for a playthrough: mark what you have researched, pick goals, and see what
						to research next.
					</p>
					{#if campaign.list?.length}<p>Pick a campaign in the toolbar, or start a new one.</p>{/if}
					<Button class="mx-auto" onclick={() => (planUi.creating = true)}
						><Plus /> New campaign</Button
					>
				</div>
			{:else if !plan}
				<p class="flex items-center gap-2 py-6 text-muted-foreground" role="status">
					<LoaderCircle class="size-4 animate-spin" /> Planning…
				</p>
			{:else}
				{#if plan.unreachable.length}
					<Alert.Root class="mb-3 border-warning-border/60 bg-warning-subtle text-warning">
						<TriangleAlert />
						<Alert.Title>Goals that can't be reached</Alert.Title>
						<Alert.Description class="text-warning/90">
							Not researchable as {app.meta?.factions.find((f) => f.name === active.faction)
								?.label ?? active.faction} in this scenario: {plan.unreachable
								.map((r) => r.name)
								.join(', ')}.
						</Alert.Description>
					</Alert.Root>
				{/if}
				{#if active.save}
					{@const save = active.save}
					{@const notes = [
						save.modded &&
							'Mods were used in this campaign: costs and prerequisites may differ from the game data.',
						save.saveVersion &&
							app.meta?.gameVersion &&
							save.saveVersion !== app.meta.gameVersion &&
							`The save is from game version ${save.saveVersion}, the loaded data from ${app.meta.gameVersion}: costs and prerequisites may differ.`,
						save.scenarioInferred &&
							'The save is from an old game version without a scenario; the default scenario is assumed.'
					].filter((n) => typeof n === 'string')}
					{#if notes.length}
						<Alert.Root class="mb-3 border-warning-border/60 bg-warning-subtle text-warning">
							<TriangleAlert />
							<Alert.Title>Imported save</Alert.Title>
							<Alert.Description class="text-warning/90">
								{#each notes as note (note)}<p>{note}</p>{/each}
							</Alert.Description>
						</Alert.Root>
					{/if}
				{/if}
				{#if plan.unknown.length}
					<Alert.Root class="mb-3 border-warning-border/60 bg-warning-subtle text-warning">
						<TriangleAlert />
						<Alert.Title>Research not found</Alert.Title>
						<Alert.Description class="text-warning/90">
							The campaign mentions research this game data doesn't have (renamed or removed in a
							game update?): {plan.unknown.map(readableKey).join(', ')}. It is kept, but ignored.
						</Alert.Description>
					</Alert.Root>
				{/if}

				{@render section('Goals', goals.length ? 'Planned in this order' : undefined)}
				{#if goals.length}
					<ol class="grid gap-0.5">
						{#each goals as key, i (key)}
							{@const status = campaign.status(key)}
							<li class="flex min-w-0 items-center gap-1">
								{@render rowStart(i + 1, status)}
								<span class="flex min-w-0 flex-1 {status === 'done' ? 'opacity-60' : ''}"
									>{@render entityName(key)}</span
								>
								{@render tipButton(
									'Move up',
									() => campaign.moveGoal(key, i - 1),
									ArrowUp,
									i === 0
								)}
								{@render tipButton(
									'Move down',
									() => campaign.moveGoal(key, i + 1),
									ArrowDown,
									i === goals.length - 1
								)}
								{@render tipButton('Remove goal', () => campaign.removeGoal(key), X)}
							</li>
						{/each}
					</ol>
				{:else}
					<p class="text-muted-foreground">
						Right-click research in the tree or use the detail panel to add goals.
					</p>
				{/if}

				{#if plan.steps.length}
					<dl class="mt-4 grid grid-cols-3 gap-2">
						{#each [{ label: 'Techs', value: plan.techCost }, { label: 'Projects', value: plan.projectCost }] as total (total.label)}
							<div class={statTile}>
								<dt class={statLabel}>{total.label}</dt>
								<dd class={statValue} title="{formatCostFull(total.value)} research points">
									{formatCost(total.value)}
								</dd>
							</div>
						{/each}
						<div class={statTile}>
							<dt class={statLabel}>Steps</dt>
							<dd class={statValue}>{plan.steps.length}</dd>
						</div>
					</dl>
					<p class="mt-1 text-xs text-muted-foreground">
						{#if active.save}
							Research points still needed at the campaign's {plan.researchSpeed}× research speed,
							less points already put in.
						{:else}
							Research points still needed, before research speed bonuses.
						{/if}
					</p>
				{/if}

				{#if plan.next.length}
					{@render section('Research next')}
					{#each [{ kind: 'tech' as const, label: 'Techs', hint: 'Global: shared by all factions', icon: FlaskConical }, { kind: 'project' as const, label: 'Projects', hint: 'Per faction', icon: Wrench }] as group (group.kind)}
						{@const rows = next[group.kind]}
						{#if rows.length}
							<div class="mb-2">
								<h4 class="mb-1 flex items-center gap-1.5 {subHeading}">
									<group.icon class="size-3.5 text-muted-foreground" />
									{group.label}
									<span class="font-normal text-muted-foreground">· {group.hint}</span>
								</h4>
								<ul class="grid gap-0.5">
									{#each showAll[group.kind] ? rows : rows.slice(0, NEXT_PREVIEW) as step (step.entity.key)}
										{@const index = campaign.stepIndex.get(step.entity.key)}
										<li class="flex min-w-0 items-center gap-1">
											{@render rowStart(index === undefined ? undefined : index + 1, step.status)}
											<span class="flex min-w-0 flex-1">{@render entityName(step.entity.key)}</span>
											{@render markers(step.goal, step.active)}
											{@render cost(
												`${formatCost(step.cost)}${step.progress ? ' left' : ''}`,
												`${formatCostFull(step.cost)} research points still needed${step.progress ? ` (${formatCostFull(step.progress)} put in)` : ''}`
											)}
											{@render tipButton(
												'Mark researched',
												() => campaign.markDone(step.entity.key),
												Check
											)}
										</li>
									{/each}
								</ul>
								{#if rows.length > NEXT_PREVIEW}
									<Button
										variant="link"
										class="h-auto px-1 py-0.5 text-xs text-muted-foreground"
										onclick={() => (showAll[group.kind] = !showAll[group.kind])}
										>{showAll[group.kind] ? 'Show less' : `Show all ${rows.length}`}</Button
									>
								{/if}
							</div>
						{/if}
					{/each}
				{/if}

				{#if plan.choices.length}
					{@render section('Choices', 'Any one of')}
					<ul class="grid gap-2">
						{#each plan.choices as choice (`${choice.node.key}:${choice.group}`)}
							<li class="grid gap-1 rounded-md border border-dashed px-2 py-1.5">
								<span class="flex min-w-0">{@render entityName(choice.node.key)}</span>
								<Select.Root
									type="single"
									value={choice.picked ? choice.chosen : AUTO}
									onValueChange={(v) =>
										campaign.pick(choice.node.key, choice.group, v === AUTO ? null : v)}
								>
									<Select.Trigger
										size="sm"
										class="w-full"
										aria-label="Requirement of {choice.node.name}: {choiceLabel(choice)}"
										><span class="truncate">{choiceLabel(choice)}</span></Select.Trigger
									>
									<Select.Content>
										<Select.Item value={AUTO} label="Auto (cheapest)">Auto (cheapest)</Select.Item>
										<Select.Separator />
										{#each choice.options as option (option.key)}
											<Select.Item value={option.key} label={option.name}
												><EntityIcon entity={option} size={16} />{option.name}</Select.Item
											>
										{/each}
									</Select.Content>
								</Select.Root>
							</li>
						{/each}
					</ul>
				{/if}

				{#if plan.steps.length}
					{@render section('Steps', 'In research order')}
					<ol class="grid gap-0.5">
						{#each plan.steps as step, i (step.entity.key)}
							<li class="flex min-w-0 items-center gap-1">
								{@render rowStart(i + 1, step.status)}
								<span class="flex min-w-0 flex-1">{@render entityName(step.entity.key)}</span>
								{@render markers(step.goal, step.active)}
								{@render cost(
									`${formatCost(step.cost)}${step.progress ? ' left' : ''}`,
									`${formatCostFull(step.cost)} research points still needed${step.progress ? ` (${formatCostFull(step.progress)} put in)` : ''}`
								)}
								{@render noAction()}
							</li>
						{/each}
					</ol>
				{:else if goals.length && goals.every((key) => campaign.status(key) === 'done')}
					<p class="mt-4 flex items-center gap-2 text-muted-foreground">
						<CircleCheck class="size-4" /> All goals are researched.
					</p>
				{/if}
			{/if}
		</div>
	</ScrollArea>
</aside>
