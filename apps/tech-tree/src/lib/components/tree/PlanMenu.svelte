<script lang="ts" module>
	import type { EntityKey } from '#lib/api.ts';

	export interface MenuTarget {
		key: EntityKey;
		name: string;
		/** Where the menu opens (client coordinates). */
		x: number;
		y: number;
	}
</script>

<script lang="ts">
	// Context menu of a tree card (right-click, Shift+F10 or the menu key on the selection):
	// mark research completed in the active campaign, or make it a goal. Un-marking research
	// other completed research depends on asks first, as those lose a requirement too.
	import Check from '@lucide/svelte/icons/check';
	import ListChecks from '@lucide/svelte/icons/list-checks';
	import Target from '@lucide/svelte/icons/target';
	import Undo2 from '@lucide/svelte/icons/undo-2';
	import X from '@lucide/svelte/icons/x';
	import { campaign } from '#lib/campaign.svelte.ts';
	import * as AlertDialog from '#lib/components/ui/alert-dialog/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';

	let {
		target = $bindable(null),
		nameOf,
		onclose
	}: {
		target?: MenuTarget | null;
		/** Display name of research (for the un-mark confirmation). */
		nameOf: (key: EntityKey) => string;
		/** The menu or confirmation closed: give the tree its focus back. */
		onclose?: () => void;
	} = $props();

	const MAX_NAMES = 12;

	// Kept while the menu fades out, so its content doesn't vanish first.
	let shown = $state.raw<MenuTarget | null>(null);
	$effect(() => {
		if (target) shown = target;
	});
	const anchor = $derived.by(() => {
		const { x, y } = shown ?? { x: 0, y: 0 };
		return { getBoundingClientRect: () => new DOMRect(x, y, 0, 0) };
	});
	const status = $derived(shown ? campaign.status(shown.key) : undefined);
	const goal = $derived(shown ? campaign.isGoal(shown.key) : false);

	let pending = $state.raw<{ key: EntityKey; name: string; lost: string[] } | null>(null);

	async function unmark(key: EntityKey, name: string) {
		const lost = await campaign.dependentsLost(key);
		if (lost === null) return; // the store reports the error
		if (!lost.length) await campaign.unmark([key]);
		else pending = { key, name, lost };
	}

	async function confirmUnmark() {
		const c = pending;
		pending = null;
		if (c) await campaign.unmark([c.key, ...c.lost]);
	}

	const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;
</script>

<DropdownMenu.Root
	bind:open={
		() => target !== null,
		(open) => {
			if (!open) target = null;
		}
	}
>
	<DropdownMenu.Content
		class="w-64"
		customAnchor={anchor}
		side="bottom"
		align="start"
		sideOffset={2}
		onCloseAutoFocus={(e) => {
			e.preventDefault();
			onclose?.();
		}}
	>
		{#if shown}
			{@const key = shown.key}
			{@const name = shown.name}
			<DropdownMenu.Label class="truncate">{name}</DropdownMenu.Label>
			<DropdownMenu.Separator />
			{#if !campaign.active}
				<DropdownMenu.Item disabled>Choose or create a campaign to plan</DropdownMenu.Item>
			{:else}
				{#if status === 'done'}
					<DropdownMenu.Item onSelect={() => unmark(key, name)}>
						<Undo2 /> Un-mark
					</DropdownMenu.Item>
				{:else if status === 'unobtainable'}
					<DropdownMenu.Item disabled>Not obtainable in this campaign</DropdownMenu.Item>
				{:else}
					<DropdownMenu.Item onSelect={() => campaign.markDone(key)}>
						<Check /> Mark researched
					</DropdownMenu.Item>
					{#if status !== 'available'}
						<DropdownMenu.Item onSelect={() => campaign.markDone(key, true)}>
							<ListChecks /> Mark researched with prerequisites
						</DropdownMenu.Item>
					{/if}
				{/if}
				{#if goal}
					<DropdownMenu.Separator />
					<DropdownMenu.Item onSelect={() => campaign.removeGoal(key)}>
						<X /> Remove goal
					</DropdownMenu.Item>
				{:else if status !== 'done' && status !== 'unobtainable'}
					<DropdownMenu.Separator />
					<DropdownMenu.Item onSelect={() => campaign.addGoal(key)}>
						<Target /> Add as goal
					</DropdownMenu.Item>
				{/if}
			{/if}
		{/if}
	</DropdownMenu.Content>
</DropdownMenu.Root>

<AlertDialog.Root
	bind:open={
		() => pending !== null,
		(open) => {
			if (!open) pending = null;
		}
	}
>
	<AlertDialog.Content
		onCloseAutoFocus={(e) => {
			e.preventDefault();
			onclose?.();
		}}
	>
		{#if pending}
			<AlertDialog.Header>
				<AlertDialog.Title>Un-mark {pending.name}?</AlertDialog.Title>
				<AlertDialog.Description>
					{plural(pending.lost.length, 'other completed item')}
					{pending.lost.length === 1 ? 'needs' : 'need'} it and will be un-marked too:
				</AlertDialog.Description>
			</AlertDialog.Header>
			<ul class="max-h-60 list-disc overflow-y-auto pl-5 text-sm">
				{#each pending.lost.slice(0, MAX_NAMES) as key (key)}
					<li>{nameOf(key)}</li>
				{/each}
				{#if pending.lost.length > MAX_NAMES}
					<li class="text-muted-foreground">+{pending.lost.length - MAX_NAMES} more</li>
				{/if}
			</ul>
			<AlertDialog.Footer>
				<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
				<AlertDialog.Action onclick={confirmUnmark}>
					Un-mark {plural(pending.lost.length + 1, 'item')}
				</AlertDialog.Action>
			</AlertDialog.Footer>
		{/if}
	</AlertDialog.Content>
</AlertDialog.Root>
