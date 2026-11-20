<script lang="ts">
	// The detail panel's planner controls for a tech or project of the active campaign: its status,
	// marking it researched (alone or with what it still needs), un-marking, goals, and for
	// repeatable projects and end-game techs how often it was completed.
	import Ban from '@lucide/svelte/icons/ban';
	import Check from '@lucide/svelte/icons/check';
	import CheckCheck from '@lucide/svelte/icons/check-check';
	import Circle from '@lucide/svelte/icons/circle';
	import CircleCheck from '@lucide/svelte/icons/circle-check';
	import Lock from '@lucide/svelte/icons/lock';
	import Minus from '@lucide/svelte/icons/minus';
	import Plus from '@lucide/svelte/icons/plus';
	import Target from '@lucide/svelte/icons/target';
	import Undo2 from '@lucide/svelte/icons/undo-2';
	import X from '@lucide/svelte/icons/x';
	import type { Component } from 'svelte';
	import type { EntityKey, EntityRef, PlanStatus } from '#lib/api.ts';
	import { campaign } from '#lib/campaign.svelte.ts';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { readableKey } from '#lib/format.ts';

	let { entity, repeatable = false }: { entity: EntityRef; repeatable?: boolean } = $props();

	const STATUS: Record<PlanStatus, { label: string; icon: Component }> = {
		done: { label: 'Researched', icon: CircleCheck },
		available: { label: 'Available now', icon: Circle },
		locked: { label: 'Needs research not marked yet', icon: Lock },
		unobtainable: { label: "Can't be researched in this scenario and faction", icon: Ban }
	};

	const status = $derived(campaign.status(entity.key));
	const info = $derived(status ? STATUS[status] : null);
	const stepIndex = $derived(campaign.stepIndex.get(entity.key));
	const count = $derived(campaign.completions(entity.key));
	const goal = $derived(campaign.isGoal(entity.key));
	let busy = $state(false);
	/** Completed research that un-marking would leave without a requirement (awaiting confirmation). */
	let lost = $state.raw<EntityKey[] | null>(null);

	async function run(action: () => Promise<unknown>) {
		busy = true;
		try {
			await action();
		} finally {
			busy = false;
		}
	}

	/** Un-mark, first asking when research marked later depends on it. */
	function unmark() {
		void run(async () => {
			const dependents = await campaign.dependentsLost(entity.key);
			if (!dependents) return;
			if (dependents.length) lost = dependents;
			else await campaign.unmark([entity.key]);
		});
	}

	function confirmUnmark(keys: EntityKey[]) {
		lost = null;
		void run(() => campaign.unmark([entity.key, ...keys]));
	}

	function setCount(next: number) {
		if (next <= 0) unmark();
		else if (count === 0) void run(() => campaign.markDone(entity.key));
		else void run(() => campaign.setCompletions(entity.key, next));
	}
</script>

<div class="mt-4 grid gap-2 rounded-lg border px-3 py-2.5" aria-label="Campaign plan" role="group">
	<div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
		{#if info}
			<span class="flex items-center gap-1.5 font-medium">
				<info.icon
					class="size-4 shrink-0 {status === 'available'
						? 'fill-current'
						: 'text-muted-foreground'}"
				/>
				{info.label}
			</span>
		{:else}
			<span class="text-muted-foreground">Not part of this campaign's research</span>
		{/if}
		{#if stepIndex !== undefined}
			<span class="text-xs text-muted-foreground">Step {stepIndex + 1} of the plan</span>
		{/if}
		{#if goal}
			<span class="flex items-center gap-1 text-xs text-muted-foreground"
				><Target class="size-3.5" /> Goal</span
			>
		{/if}
	</div>
	{#if status && status !== 'unobtainable'}
		<div class="grid grid-cols-2 gap-2">
			{#if repeatable}
				<div
					class="col-span-2 flex items-center justify-between gap-2 text-sm"
					role="group"
					aria-label="Times completed"
				>
					<span class="text-muted-foreground">Times completed</span>
					<span class="flex items-center gap-1">
						<Button
							variant="outline"
							size="icon"
							class="size-8"
							aria-label="One fewer"
							disabled={busy || count === 0}
							onclick={() => setCount(count - 1)}><Minus /></Button
						>
						<span class="w-8 text-center font-semibold tabular-nums" aria-live="polite"
							>{count}</span
						>
						<Button
							variant="outline"
							size="icon"
							class="size-8"
							aria-label="One more"
							disabled={busy || (count === 0 && status === 'locked')}
							onclick={() => setCount(count + 1)}><Plus /></Button
						>
					</span>
				</div>
			{/if}
			{#if status === 'done'}
				{#if !repeatable}
					<Button variant="outline" size="sm" disabled={busy} onclick={unmark}
						><Undo2 /> Un-mark</Button
					>
				{/if}
			{:else}
				{#if !repeatable}
					<Button
						variant="outline"
						size="sm"
						disabled={busy || status === 'locked'}
						title={status === 'locked' ? 'Some requirement is not marked yet' : undefined}
						onclick={() => run(() => campaign.markDone(entity.key))}
						><Check /> Mark researched</Button
					>
				{/if}
				<Button
					variant="outline"
					size="sm"
					disabled={busy}
					title="Also mark everything this still needs"
					onclick={() => run(() => campaign.markDone(entity.key, true))}
					><CheckCheck /> With prerequisites</Button
				>
			{/if}
			{#if goal}
				<Button
					variant="outline"
					size="sm"
					disabled={busy}
					onclick={() => run(() => campaign.removeGoal(entity.key))}><X /> Remove goal</Button
				>
			{:else if status !== 'done'}
				<Button
					variant="outline"
					size="sm"
					disabled={busy}
					onclick={() => run(() => campaign.addGoal(entity.key))}><Target /> Add goal</Button
				>
			{/if}
		</div>
	{:else if goal}
		<Button
			variant="outline"
			size="sm"
			class="w-fit"
			disabled={busy}
			onclick={() => run(() => campaign.removeGoal(entity.key))}><X /> Remove goal</Button
		>
	{/if}
</div>

<AlertDialog.Root
	open={lost !== null}
	onOpenChange={(next) => {
		if (!next) lost = null;
	}}
>
	<AlertDialog.Content>
		{#if lost}
			{@const keys = lost}
			<AlertDialog.Header>
				<AlertDialog.Title>Un-mark {entity.name}?</AlertDialog.Title>
				<AlertDialog.Description>
					{keys.length === 1 ? 'This research needs' : `These ${keys.length} need`} it and will be un-marked
					too:
				</AlertDialog.Description>
			</AlertDialog.Header>
			<ul class="max-h-48 list-disc overflow-y-auto pl-5 text-sm">
				{#each keys as key (key)}<li>{readableKey(key)}</li>{/each}
			</ul>
			<AlertDialog.Footer>
				<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
				<AlertDialog.Action variant="destructive" onclick={() => confirmUnmark(keys)}
					>Un-mark {keys.length + 1}</AlertDialog.Action
				>
			</AlertDialog.Footer>
		{/if}
	</AlertDialog.Content>
</AlertDialog.Root>
