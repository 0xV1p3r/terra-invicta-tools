<script lang="ts">
	import Check from '@lucide/svelte/icons/check';
	import Circle from '@lucide/svelte/icons/circle';
	import CircleX from '@lucide/svelte/icons/circle-x';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import * as Alert from '$lib/components/ui/alert/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Progress } from '$lib/components/ui/progress/index.js';
	import { extraction, STAGES } from '$lib/extraction.svelte';

	interface Props {
		/** Run the last extraction again. */
		onRetry: () => void;
		/** Leave the failed state (e.g. back to the options); no button when absent. */
		onBack?: () => void;
	}
	let { onRetry, onBack }: Props = $props();

	let cancelButton = $state<HTMLElement | null>(null);
	let retryButton = $state<HTMLElement | null>(null);
	// The control that started or retried the extraction is gone (or disabled) now: keep keyboard
	// focus in the flow instead of losing it to the page.
	$effect(() => {
		const target = extraction.running ? cancelButton : retryButton;
		const active = document.activeElement;
		if (target && (!active || active === document.body || active.matches(':disabled')))
			target.focus();
	});

	const current = $derived(STAGES.findIndex((s) => s.id === extraction.stage));
	const stageLabel = $derived(current >= 0 ? STAGES[current].label : 'Starting');
	const count = $derived(
		extraction.total > 1
			? `${extraction.done.toLocaleString('en-US')} / ${extraction.total.toLocaleString('en-US')}`
			: ''
	);
	/** Announced when the phase changes; step counts change too often to read out. */
	const announcement = $derived(
		extraction.running
			? extraction.cancelling
				? 'Cancelling extraction'
				: `Extracting game data: ${stageLabel}`
			: ''
	);

	type StepState = 'done' | 'current' | 'failed' | 'pending';
	function stepState(i: number): StepState {
		if (current < 0 || i > current) return 'pending';
		if (i < current) return 'done';
		return extraction.running ? 'current' : extraction.error ? 'failed' : 'done';
	}
</script>

<div class="grid gap-4">
	<p class="sr-only" aria-live="polite">{announcement}</p>
	<ol class="grid gap-1.5 text-sm" aria-label="Extraction steps">
		{#each STAGES as stage, i (stage.id)}
			{@const state = stepState(i)}
			<li
				class="flex items-center gap-2.5"
				class:text-muted-foreground={state === 'pending'}
				aria-current={state === 'current' ? 'step' : undefined}
			>
				{#if state === 'done'}
					<Check class="size-4" aria-hidden="true" />
				{:else if state === 'current'}
					<LoaderCircle class="size-4 animate-spin" aria-hidden="true" />
				{:else if state === 'failed'}
					<CircleX class="size-4 text-destructive" aria-hidden="true" />
				{:else}
					<Circle class="size-4 opacity-50" aria-hidden="true" />
				{/if}
				<span class:font-medium={state === 'current'}>{stage.label}</span>
				<span class="sr-only"
					>{state === 'done'
						? '(done)'
						: state === 'current'
							? '(in progress)'
							: state === 'failed'
								? '(failed)'
								: ''}</span
				>
			</li>
		{/each}
	</ol>

	{#if extraction.running}
		<div class="grid gap-2">
			<Progress
				value={extraction.percent}
				class="h-1.5 [&>[data-slot=progress-indicator]]:transition-transform [&>[data-slot=progress-indicator]]:duration-200 [&>[data-slot=progress-indicator]]:ease-linear motion-reduce:[&>[data-slot=progress-indicator]]:transition-none"
				aria-label="Overall extraction progress"
				aria-valuetext={`${Math.round(extraction.percent)}% overall, ${stageLabel}`}
			/>
			<div class="flex min-w-0 items-baseline justify-between gap-3 text-xs text-muted-foreground">
				<span class="truncate" title={extraction.label ?? undefined}
					>{extraction.label ?? `${stageLabel}…`}</span
				>
				<span class="shrink-0 tabular-nums">{count}</span>
			</div>
		</div>
		<div>
			<Button
				bind:ref={cancelButton}
				variant="outline"
				disabled={extraction.cancelling}
				onclick={() => extraction.cancel()}
			>
				{#if extraction.cancelling}<LoaderCircle class="animate-spin" /> Cancelling…{:else}Cancel{/if}
			</Button>
		</div>
	{:else if extraction.error}
		<Alert.Root variant={extraction.cancelled ? 'default' : 'destructive'}>
			<TriangleAlert />
			<Alert.Title
				>{extraction.cancelled ? 'Extraction cancelled' : 'Extraction failed'}</Alert.Title
			>
			<Alert.Description class="break-words">
				{extraction.cancelled
					? 'Nothing was written. You can start again at any time.'
					: extraction.error}
			</Alert.Description>
		</Alert.Root>
		<div class="flex flex-wrap gap-2">
			<Button bind:ref={retryButton} onclick={onRetry}
				><RotateCcw /> {extraction.cancelled ? 'Start again' : 'Retry'}</Button
			>
			{#if onBack}
				<Button variant="ghost" onclick={onBack}>Back</Button>
			{/if}
		</div>
	{/if}
</div>
