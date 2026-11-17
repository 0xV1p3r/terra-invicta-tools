<script lang="ts">
	import Download from '@lucide/svelte/icons/download';
	import FolderOpen from '@lucide/svelte/icons/folder-open';
	import FolderSearch from '@lucide/svelte/icons/folder-search';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { api, mockOnboarding, type ExtractArgs } from '#lib/api.ts';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { extractArgs, extraction, gameLabel } from '#lib/extraction.svelte.ts';
	import ExtractionProgress from './ExtractionProgress.svelte';
	import ExtractOptions from './ExtractOptions.svelte';
	import wordmark from '../../../../branding/wordmark-dark.svg';

	interface Props {
		/** Extract, then load the new build (the app leaves onboarding once it is loaded). */
		extract: (args: ExtractArgs) => Promise<void>;
		/** Pick an existing data folder and load it. */
		chooseDataDir: () => Promise<void>;
		/** Why no data is loaded (the backend's startup error). */
		loadError: string;
	}
	let { extract, chooseDataDir, loadError }: Props = $props();

	/** Data was found but failed to load: worth showing, unlike "nothing extracted yet". */
	const brokenData = $derived(!loadError.startsWith('No extracted game data found'));

	let busy = $state<'install' | 'data' | null>(null);
	let pickError = $state('');

	$effect(() => {
		void extraction.detect().then((game) => {
			// Mock onboarding: show the progress and error states without clicking through.
			if (game && (mockOnboarding === 'progress' || mockOnboarding === 'error'))
				void extract(extractArgs(game));
		});
	});

	async function useAnotherInstall() {
		busy = 'install';
		pickError = '';
		try {
			const path = await api.pickFolder('Choose the Terra Invicta folder', extraction.game?.root);
			if (path) extraction.choose(await api.inspectGame(path));
		} catch (cause) {
			pickError = String(cause);
		} finally {
			busy = null;
		}
	}

	async function useExtractedData() {
		busy = 'data';
		pickError = '';
		try {
			await chooseDataDir();
		} catch (cause) {
			pickError = String(cause);
		} finally {
			busy = null;
		}
	}

	const showProgress = $derived(extraction.running || extraction.error !== null);
</script>

<main class="grid min-h-screen place-items-center overflow-y-auto p-6">
	<div class="grid w-full max-w-xl gap-6">
		<header class="grid gap-4">
			<h1><img src={wordmark} alt="Terra Invicta Tools" class="h-14 w-auto" /></h1>
			<p class="text-muted-foreground">These tools are built from your own copy of the game.</p>
		</header>

		<Card.Root>
			{#if showProgress}
				<Card.Header>
					<Card.Title>
						<h2>{extraction.running ? 'Extracting game data' : 'Extraction stopped'}</h2>
					</Card.Title>
					{#if extraction.game}
						<Card.Description>From {gameLabel(extraction.game)}</Card.Description>
					{/if}
				</Card.Header>
				<Card.Content>
					<ExtractionProgress
						onRetry={() => extraction.lastArgs && void extract(extraction.lastArgs)}
						onBack={() => extraction.dismissError()}
					/>
				</Card.Content>
			{:else if !extraction.detected}
				<Card.Content>
					<p class="flex items-center gap-2 text-muted-foreground" role="status">
						<LoaderCircle class="size-4 animate-spin" /> Looking for Terra Invicta…
					</p>
				</Card.Content>
			{:else}
				{@const game = extraction.game}
				<Card.Header>
					<Card.Title>
						<h2>{game ? `Found ${gameLabel(game)}` : 'Terra Invicta was not found'}</h2>
					</Card.Title>
					<Card.Description>
						{#if game}
							<span class="font-mono text-xs break-all">{game.root}</span>
						{:else}
							It is not in your Steam libraries. Choose the folder you installed it to.
						{/if}
					</Card.Description>
				</Card.Header>
				<Card.Content class="grid gap-5">
					{#if game}
						<ExtractOptions />
					{/if}
					<div>
						{#if game}
							<Button disabled={busy !== null} onclick={() => void extract(extractArgs(game))}>
								<Download /> Extract game data
							</Button>
						{:else}
							<Button disabled={busy !== null} onclick={useAnotherInstall}>
								{#if busy === 'install'}<LoaderCircle class="animate-spin" />{:else}<FolderSearch
									/>{/if}
								Choose game folder…
							</Button>
						{/if}
					</div>
					{#if pickError}
						<p class="text-sm text-destructive" role="alert">{pickError}</p>
					{/if}
				</Card.Content>
				<Card.Footer class="flex flex-wrap gap-1 border-t">
					{#if game}
						<Button
							variant="ghost"
							size="sm"
							class="-ml-2.5"
							disabled={busy !== null}
							onclick={useAnotherInstall}
						>
							{#if busy === 'install'}<LoaderCircle class="animate-spin" />{:else}<FolderSearch
								/>{/if}
							Use another install…
						</Button>
					{/if}
					<Button
						variant="ghost"
						size="sm"
						class={game ? '' : '-ml-2.5'}
						disabled={busy !== null}
						onclick={useExtractedData}
					>
						{#if busy === 'data'}<LoaderCircle class="animate-spin" />{:else}<FolderOpen />{/if}
						I already have extracted data…
					</Button>
				</Card.Footer>
			{/if}
		</Card.Root>

		{#if brokenData}
			<Alert.Root>
				<TriangleAlert />
				<Alert.Title>The game data found could not be loaded</Alert.Title>
				<Alert.Description class="font-mono text-xs break-words">{loadError}</Alert.Description>
			</Alert.Root>
		{/if}
	</div>
</main>
