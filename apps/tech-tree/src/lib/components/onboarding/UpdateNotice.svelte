<script lang="ts">
	import RefreshCw from '@lucide/svelte/icons/refresh-cw';
	import X from '@lucide/svelte/icons/x';
	import { api, type BuildInfo, type ExtractArgs } from '$lib/api';
	import Tip from '$lib/components/toolbar/Tip.svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import { extractArgs, extraction, gameUpdate } from '$lib/extraction.svelte';
	import { settings } from '$lib/settings.svelte';
	import ExtractionProgress from './ExtractionProgress.svelte';

	// Remounted with the page on every data switch, so the build list is current.
	let { extract }: { extract: (args: ExtractArgs) => Promise<void> } = $props();

	let builds = $state.raw<BuildInfo[]>([]);
	let progressOpen = $state(false);

	// After the first paint, and in the background: detection reads the game files.
	$effect(() => {
		let stale = false;
		const timer = setTimeout(() => {
			void Promise.all([extraction.detect(), api.listBuilds()]).then(
				([, result]) => {
					if (!stale) builds = result;
				},
				() => {}
			);
		}, 500);
		return () => {
			stale = true;
			clearTimeout(timer);
		};
	});

	const update = $derived(gameUpdate(extraction.game, builds, settings.dismissedUpdates));
	const buildChange = $derived(
		update && `(build ${update.from ? `${update.from} → ` : ''}${update.to})`
	);

	function start() {
		progressOpen = true;
		if (!extraction.running && extraction.game) void extract(extractArgs(extraction.game));
	}

	function dismiss() {
		if (update) settings.dismissedUpdates = [...settings.dismissedUpdates, update.to];
	}
</script>

{#if update}
	<div
		class="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1.5 border-b bg-card px-3 py-1.5 text-sm"
		role="region"
		aria-label="Game update"
	>
		<RefreshCw class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
		<p role="status">
			<span class="font-medium">Terra Invicta was updated</span>
			<span class="text-muted-foreground">{buildChange}</span>
		</p>
		<div class="ml-auto flex items-center gap-1">
			<Button size="sm" onclick={start}>
				{extraction.running ? 'Show progress' : 'Extract now'}
			</Button>
			<Tip text="Dismiss until the next update">
				<Button
					variant="ghost"
					size="icon"
					class="size-8"
					aria-label="Dismiss update notice"
					onclick={dismiss}
				>
					<X />
				</Button>
			</Tip>
		</div>
	</div>
{/if}

<Dialog.Root bind:open={progressOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Extracting game data</Dialog.Title>
			<Dialog.Description>
				{#if update}Build {update.to}.{/if} You can close this window; the extraction continues and the
				new build opens when it is done.
			</Dialog.Description>
		</Dialog.Header>
		<ExtractionProgress
			onRetry={() => extraction.lastArgs && void extract(extraction.lastArgs)}
			onBack={() => {
				extraction.dismissError();
				progressOpen = false;
			}}
		/>
	</Dialog.Content>
</Dialog.Root>
