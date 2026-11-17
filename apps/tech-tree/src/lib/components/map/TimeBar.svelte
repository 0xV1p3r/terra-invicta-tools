<script lang="ts">
	// Map date: day, month and year fields, a month slider over the campaign years, and play/pause.
	import Pause from '@lucide/svelte/icons/pause';
	import Play from '@lucide/svelte/icons/play';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { untrack } from 'svelte';
	import * as Select from '#lib/components/ui/select/index.js';
	import { Slider } from '#lib/components/ui/slider/index.js';
	import { DAY_MS, mapState } from './map-state.svelte';

	/** Span after the start date. */
	const YEARS = 120;

	const SPEEDS = [
		{ value: '1', label: '1 day/s' },
		{ value: '7', label: '1 week/s' },
		{ value: '30', label: '1 month/s' },
		{ value: '365', label: '1 year/s' }
	];

	const MONTHS = Array.from({ length: 12 }, (_, m) => {
		const d = new Date(Date.UTC(2000, m, 1));
		const name = (month: 'long' | 'short') =>
			d.toLocaleDateString('en', { month, timeZone: 'UTC' });
		return { value: String(m), short: name('short'), long: name('long') };
	});

	// Whole calendar years, so the end falls in the slider's last month (the start day, clamped).
	const end = $derived.by(() => {
		const s = new Date(mapState.start);
		const [y, m] = [s.getUTCFullYear() + YEARS, s.getUTCMonth()];
		return Date.UTC(y, m, Math.min(s.getUTCDate(), daysIn(y, m)));
	});
	const clampTime = (t: number) => Math.min(end, Math.max(mapState.start, t));
	const daysIn = (y: number, m: number) => new Date(Date.UTC(y, m + 1, 0)).getUTCDate();

	// Calendar fields of the map date; they only change once a day while playing.
	const year = $derived(new Date(mapState.time).getUTCFullYear());
	const month = $derived(new Date(mapState.time).getUTCMonth());
	const day = $derived(new Date(mapState.time).getUTCDate());

	const startDate = $derived(new Date(mapState.start));
	const endDate = $derived(new Date(end));
	const startYear = $derived(startDate.getUTCFullYear());
	const endYear = $derived(endDate.getUTCFullYear());

	// The slider moves in months: a day step would make it lay out ~44k ticks on every change.
	const monthIndex = $derived((year - startYear) * 12 + month - startDate.getUTCMonth());
	const SPAN_MONTHS = YEARS * 12;
	const monthLabel = $derived(`${MONTHS[month]?.long ?? ''} ${year}`);

	let status = $state('');

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

	/** Years far outside the span would overflow `Date.UTC` into NaN; the span clamp does the rest. */
	const boundYear = (y: number) => Math.min(endYear + 1, Math.max(startYear - 1, y));

	/** Sets the date (UTC midnight, kept within the span); out-of-range days roll over. */
	function setDate(y: number, m: number, d: number) {
		mapState.playing = false;
		mapState.time = clampTime(Date.UTC(boundYear(y), m, Math.min(400, Math.max(-400, d))));
	}

	/** Months and years keep the day, clamped to the target month (Jan 31 → Feb 28). */
	function setMonth(y: number, m: number) {
		const t = new Date(Date.UTC(boundYear(y), m, 1));
		const [ty, tm] = [t.getUTCFullYear(), t.getUTCMonth()];
		setDate(ty, tm, Math.min(day, daysIn(ty, tm)));
	}

	/** Commits a number field; an empty or rejected entry shows the current value again. */
	function commit(input: HTMLInputElement, apply: (n: number) => void, current: () => number) {
		const n = input.valueAsNumber;
		if (Number.isFinite(n)) apply(Math.trunc(n));
		input.value = String(current());
	}

	function fieldKeydown(event: KeyboardEvent & { currentTarget: HTMLInputElement }) {
		if (event.key === 'Enter') event.currentTarget.dispatchEvent(new Event('change'));
		else if (event.key === 'Escape') {
			event.currentTarget.value = String(event.currentTarget.dataset.field === 'year' ? year : day);
			event.currentTarget.blur();
		}
	}

	// Playback: advance by `speed` days per real second; stop at the end of the span.
	$effect(() => {
		if (!mapState.playing) return;
		let last = performance.now();
		let frame = requestAnimationFrame(function tick(now) {
			const dt = Math.min(0.25, (now - last) / 1000);
			last = now;
			mapState.time = Math.min(end, mapState.time + dt * mapState.speed * DAY_MS);
			if (mapState.time >= end) mapState.playing = false;
			else frame = requestAnimationFrame(tick);
		});
		return () => cancelAnimationFrame(frame);
	});
</script>

<div
	class="grid gap-1.5 rounded-xl border bg-card px-2 py-1.5 shadow"
	role="group"
	aria-label="Map date"
>
	<div class="flex items-center gap-1.5">
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
		<Input
			type="number"
			data-field="day"
			class="h-8 w-16 px-2 text-center font-mono tabular-nums"
			aria-label="Day"
			title="Day"
			min={0}
			max={daysIn(year, month) + 1}
			value={day}
			onfocus={() => (mapState.playing = false)}
			onchange={(e) =>
				commit(
					e.currentTarget,
					(d) => setDate(year, month, d),
					() => day
				)}
			onkeydown={fieldKeydown}
		/>
		<Select.Root
			type="single"
			value={String(month)}
			onValueChange={(v) => v && setMonth(year, Number(v))}
		>
			<Select.Trigger size="sm" aria-label="Month" title="Month" class="w-20">
				{MONTHS[month]?.short}
			</Select.Trigger>
			<Select.Content>
				{#each MONTHS as m (m.value)}
					<Select.Item value={m.value} label={m.long}>{m.long}</Select.Item>
				{/each}
			</Select.Content>
		</Select.Root>
		<Input
			type="number"
			data-field="year"
			class="h-8 w-20 px-2 text-center font-mono tabular-nums"
			aria-label="Year, {startYear} to {endYear}"
			title="Year"
			min={startYear}
			max={endYear}
			value={year}
			onfocus={() => (mapState.playing = false)}
			onchange={(e) =>
				commit(
					e.currentTarget,
					(y) => setMonth(y, month),
					() => year
				)}
			onkeydown={fieldKeydown}
		/>
		<Select.Root
			type="single"
			value={String(mapState.speed)}
			onValueChange={(v) => v && (mapState.speed = Number(v))}
		>
			<Select.Trigger size="sm" aria-label="Playback speed" class="ml-auto w-28 shrink-0">
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
	<div class="grid gap-1">
		<div class="flex justify-between text-[10px] text-muted-foreground" aria-hidden="true">
			<span>{startYear}</span>
			<span>{endYear}</span>
		</div>
		<Slider
			type="single"
			class="min-w-0"
			min={0}
			max={SPAN_MONTHS}
			step={1}
			value={monthIndex}
			onValueChange={(v: number) => {
				// Ignore echoes of the current month (they'd reset the day).
				if (v !== monthIndex) setMonth(startYear, startDate.getUTCMonth() + v);
			}}
			thumbLabel="Month"
			thumbValueText={monthLabel}
		/>
	</div>
	<span class="sr-only" role="status" aria-live="polite" aria-atomic="true">{status}</span>
</div>
