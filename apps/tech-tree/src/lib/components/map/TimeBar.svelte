<script lang="ts">
	// Map date: slider over the campaign years, step buttons, and play/pause.
	import Pause from '@lucide/svelte/icons/pause';
	import Play from '@lucide/svelte/icons/play';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { untrack } from 'svelte';
	import * as Select from '$lib/components/ui/select/index.js';
	import { Slider } from '$lib/components/ui/slider/index.js';
	import { DAY_MS, mapState } from './map-state.svelte';

	/** Slider span after the start date. */
	const YEARS = 120;
	const SPAN_DAYS = Math.round(YEARS * 365.25);

	const SPEEDS = [
		{ value: '1', label: '1 day/s' },
		{ value: '7', label: '1 week/s' },
		{ value: '30', label: '1 month/s' },
		{ value: '365', label: '1 year/s' }
	];

	const day = $derived(Math.round((mapState.time - mapState.start) / DAY_MS));
	const label = $derived(
		Number.isFinite(mapState.time) ? new Date(mapState.time).toISOString().slice(0, 10) : '–'
	);

	const startLabel = $derived(new Date(mapState.start).toISOString().slice(0, 10));
	const endLabel = $derived(
		new Date(mapState.start + SPAN_DAYS * DAY_MS).toISOString().slice(0, 10)
	);
	let draft = $state('');
	let editing = $state(false);
	let dateError = $state(false);
	const dateErrorId = $props.id();
	let status = $state('');

	$effect(() => {
		if (!editing && !dateError) draft = label;
	});

	// Observe playback transitions only, including pauses from the map keyboard shortcut.
	let wasPlaying = false;
	$effect(() => {
		const playing = mapState.playing;
		if (wasPlaying && !playing) untrack(() => announce('Paused.'));
		wasPlaying = playing;
	});

	function announce(prefix = '') {
		status = `${prefix} Map date ${new Date(mapState.time).toISOString().slice(0, 10)}.`.trim();
	}

	function commitDate() {
		if (!editing) return;
		const parsed = Date.parse(`${draft}T00:00:00Z`);
		if (
			!/^\d{4}-\d{2}-\d{2}$/.test(draft) ||
			!Number.isFinite(parsed) ||
			new Date(parsed).toISOString().slice(0, 10) !== draft ||
			parsed < mapState.start ||
			parsed > mapState.start + SPAN_DAYS * DAY_MS
		) {
			dateError = true;
			return;
		}
		dateError = false;
		mapState.playing = false;
		setDay(Math.round((parsed - mapState.start) / DAY_MS));
		announce();
		draft = new Date(mapState.time).toISOString().slice(0, 10);
		editing = false;
	}

	interface Step {
		unit: 'day' | 'month' | 'year';
		n: number;
		short: string;
		label: string;
	}
	const STEPS: Step[] = [
		{ unit: 'year', n: -1, short: '−Y', label: 'Back one year' },
		{ unit: 'month', n: -1, short: '−M', label: 'Back one month' },
		{ unit: 'day', n: -1, short: '−D', label: 'Back one day' },
		{ unit: 'day', n: 1, short: '+D', label: 'Forward one day' },
		{ unit: 'month', n: 1, short: '+M', label: 'Forward one month' },
		{ unit: 'year', n: 1, short: '+Y', label: 'Forward one year' }
	];

	function setDay(d: number) {
		mapState.time = mapState.start + Math.min(SPAN_DAYS, Math.max(0, d)) * DAY_MS;
	}

	function step(unit: Step['unit'], n: number) {
		mapState.playing = false;
		const t = new Date(mapState.time);
		const [y, m, d] = [t.getUTCFullYear(), t.getUTCMonth(), t.getUTCDate()];
		// Months/years keep the day, clamped to the target month's length (Jan 31 + 1 month = Feb 28).
		const shifted = (ty: number, tm: number) =>
			Date.UTC(ty, tm, Math.min(d, new Date(Date.UTC(ty, tm + 1, 0)).getUTCDate()));
		const next =
			unit === 'day'
				? Date.UTC(y, m, d + n)
				: unit === 'month'
					? shifted(y, m + n)
					: shifted(y + n, m);
		setDay(Math.round((next - mapState.start) / DAY_MS));
		announce();
	}

	// Playback: advance by `speed` days per real second; stop at the end of the slider.
	$effect(() => {
		if (!mapState.playing) return;
		let last = performance.now();
		let frame = requestAnimationFrame(function tick(now) {
			const dt = Math.min(0.25, (now - last) / 1000);
			last = now;
			const end = mapState.start + SPAN_DAYS * DAY_MS;
			mapState.time = Math.min(end, mapState.time + dt * mapState.speed * DAY_MS);
			if (mapState.time >= end) mapState.playing = false;
			else frame = requestAnimationFrame(tick);
		});
		return () => cancelAnimationFrame(frame);
	});
</script>

{#snippet stepButton(s: Step)}
	<Button
		variant="ghost"
		size="sm"
		class="h-8 px-1.5 font-mono text-xs"
		aria-label={s.label}
		title={s.label}
		onclick={() => step(s.unit, s.n)}>{s.short}</Button
	>
{/snippet}

<div
	class="relative grid gap-1.5 rounded-xl border bg-card px-2 py-1.5 shadow"
	role="group"
	aria-label="Map date"
>
	{#if dateError}
		<p
			id={dateErrorId}
			role="alert"
			class="absolute right-0 bottom-full mb-2 max-w-full rounded-md border bg-popover px-3 py-2 text-xs text-popover-foreground shadow-md"
		>
			Use YYYY-MM-DD between {startLabel} and {endLabel}.
		</p>
	{/if}
	<div class="flex items-center justify-between gap-1">
		{#each STEPS.slice(0, 3) as s (s.label)}
			{@render stepButton(s)}
		{/each}
		<Button
			variant="outline"
			size="icon"
			class="size-8"
			aria-label={mapState.playing ? 'Pause' : 'Play'}
			title={mapState.playing ? 'Pause' : 'Play'}
			onclick={() => (mapState.playing = !mapState.playing)}
		>
			{#if mapState.playing}<Pause />{:else}<Play />{/if}
		</Button>
		{#each STEPS.slice(3) as s (s.label)}
			{@render stepButton(s)}
		{/each}
		<Input
			class="h-8 w-28 shrink-0 px-1 text-center font-mono text-sm tabular-nums"
			aria-label="Map date, YYYY-MM-DD"
			aria-live="off"
			aria-invalid={dateError}
			aria-describedby={dateError ? dateErrorId : undefined}
			bind:value={draft}
			onfocus={() => {
				mapState.playing = false;
				editing = true;
			}}
			onblur={commitDate}
			onkeydown={(event) => {
				if (event.key === 'Enter') {
					commitDate();
					event.currentTarget.blur();
				}
				if (event.key === 'Escape') {
					dateError = false;
					editing = false;
					draft = label;
					event.currentTarget.blur();
				}
			}}
		/>
	</div>
	<div class="flex items-center gap-2">
		<div class="grid min-w-0 flex-1 gap-1">
			<div class="flex justify-between text-[10px] text-muted-foreground" aria-hidden="true">
				<span title={startLabel}>{startLabel.slice(0, 4)}</span>
				<span title={endLabel}>{endLabel.slice(0, 4)}</span>
			</div>
			<Slider
				type="single"
				class="min-w-0"
				min={0}
				max={SPAN_DAYS}
				step={1}
				value={day}
				onValueChange={(v: number) => {
					mapState.playing = false;
					setDay(v);
				}}
				thumbLabel="Date"
				thumbValueText={label}
			/>
		</div>
		<Select.Root
			type="single"
			value={String(mapState.speed)}
			onValueChange={(v) => v && (mapState.speed = Number(v))}
		>
			<Select.Trigger size="sm" aria-label="Playback speed" class="w-28 shrink-0">
				{SPEEDS.find((s) => s.value === String(mapState.speed))?.label ?? `${mapState.speed} d/s`}
			</Select.Trigger>
			<Select.Content>
				{#each SPEEDS as s (s.value)}
					<Select.Item value={s.value} label={s.label}>{s.label}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
		<Button
			variant="ghost"
			size="icon"
			class="size-8"
			aria-label="Back to the start date"
			title="Start date"
			onclick={() => {
				mapState.playing = false;
				mapState.time = mapState.start;
				announce();
			}}
		>
			<RotateCcw />
		</Button>
	</div>
	<span class="sr-only" role="status" aria-live="polite" aria-atomic="true">{status}</span>
</div>
