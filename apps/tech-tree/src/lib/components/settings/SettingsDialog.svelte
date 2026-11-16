<script module lang="ts">
	// Outside the instance: a data switch remounts the page (and this dialog); it stays open.
	let open = $state(false);
</script>

<script lang="ts">
	import ArrowDownLeft from '@lucide/svelte/icons/arrow-down-left';
	import ArrowDownRight from '@lucide/svelte/icons/arrow-down-right';
	import ArrowUpLeft from '@lucide/svelte/icons/arrow-up-left';
	import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right';
	import Tip from '$lib/components/toolbar/Tip.svelte';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
	import Settings from '@lucide/svelte/icons/settings';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { api, type BuildInfo, type ExtractArgs } from '$lib/api';
	import ExtractionProgress from '$lib/components/onboarding/ExtractionProgress.svelte';
	import ExtractOptions from '$lib/components/onboarding/ExtractOptions.svelte';
	import * as AlertDialog from '$lib/components/ui/alert-dialog/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Select from '$lib/components/ui/select/index.js';
	import { Switch } from '$lib/components/ui/switch/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
	import { extractArgs, extraction, gameLabel } from '$lib/extraction.svelte';
	import { formatBytes } from '$lib/game';
	import { settings, type Corner } from '$lib/settings.svelte';
	import { app } from '$lib/state.svelte';
	import { pillIcon } from '$lib/components/toolbar/Pill.svelte';

	const LANGUAGES: Record<string, string> = {
		en: 'English',
		deu: 'Deutsch',
		fr: 'Français',
		esp: 'Español',
		ita: 'Italiano',
		pol: 'Polski',
		por: 'Português',
		rus: 'Русский',
		ukr: 'Українська',
		cze: 'Čeština',
		chs: '简体中文',
		cht: '繁體中文',
		jpn: '日本語',
		kor: '한국어'
	};

	interface Props {
		/** Pick a data folder, load it and reload the page's data; resolves after the switch. */
		chooseDataDir: (current?: string) => Promise<void>;
		/** Load another build (null = always the latest) and reload the page's data. */
		switchBuild: (buildId: string | null) => Promise<void>;
		/** Extract the game data and load the new build. */
		extract: (args: ExtractArgs) => Promise<void>;
	}
	let { chooseDataDir, switchBuild, extract }: Props = $props();

	const LATEST = 'latest';
	let builds = $state.raw<BuildInfo[]>([]);
	/** Bytes per build directory; filled after the list (walking the folders takes a moment). */
	let sizes = $state.raw<Record<string, number>>({});
	let switching = $state(false);
	let dataError = $state('');
	/** Bumped after deleting a build, to reload the list. */
	let buildsRevision = $state(0);
	$effect(() => {
		void app.meta?.buildId;
		void app.meta?.dataDir;
		void buildsRevision;
		if (!open) return;
		let stale = false;
		api.listBuilds().then(
			(result) => {
				if (!stale) builds = result;
			},
			(cause) => {
				if (!stale) dataError = String(cause);
			}
		);
		api.getBuildSizes().then(
			(result) => {
				if (!stale) sizes = Object.fromEntries(result.map((s) => [s.dir, s.bytes]));
			},
			() => {}
		);
		void extraction.detect();
		return () => {
			stale = true;
		};
	});

	/** Build awaiting delete confirmation. */
	let deleting = $state.raw<BuildInfo | null>(null);
	function confirmDelete(build: BuildInfo) {
		const dir = build.dir;
		deleting = null;
		void run(async () => {
			await api.deleteBuild(dir);
			buildsRevision += 1;
		});
	}

	function deleteSummary(build: BuildInfo) {
		const size = sizes[build.dir];
		return `${buildLabel(build, build.buildId)}${size === undefined ? '' : `, ${formatBytes(size)}`}`;
	}

	function extractAgain() {
		if (extraction.game) void extract(extractArgs(extraction.game, true));
	}
	const showProgress = $derived(extraction.running || extraction.error !== null);

	function buildLabel(build: BuildInfo | undefined, id: string) {
		if (!build) return `Build ${id}`;
		const version = `${build.gameVersion ? `v${build.gameVersion}` : 'Unknown version'} · ${build.buildId}`;
		const date = build.lastUpdated ? new Date(build.lastUpdated * 1000).toLocaleDateString() : null;
		return date ? `${version} (${date})` : version;
	}
	const latestBuild = $derived(builds.find((b) => b.latest));
	const buildValue = $derived(app.meta?.pinnedBuild ?? LATEST);
	const buildTriggerLabel = $derived(
		buildValue === LATEST
			? `Latest: ${buildLabel(latestBuild, app.meta?.buildId ?? '')}`
			: buildLabel(
					builds.find((b) => b.buildId === buildValue),
					buildValue
				)
	);

	async function run(action: () => Promise<unknown>) {
		switching = true;
		dataError = '';
		try {
			await action();
		} catch (cause) {
			dataError = String(cause);
		} finally {
			switching = false;
		}
	}

	const CORNER_OPTIONS = [
		{ value: 'top-left', label: 'Top left', icon: ArrowUpLeft },
		{ value: 'top-right', label: 'Top right', icon: ArrowUpRight },
		{ value: 'bottom-left', label: 'Bottom left', icon: ArrowDownLeft },
		{ value: 'bottom-right', label: 'Bottom right', icon: ArrowDownRight }
	] as const;
</script>

<Dialog.Root bind:open>
	<Tip text="Settings">
		<Dialog.Trigger>
			{#snippet child({ props })}
				<Button {...props} variant="ghost" size="icon" class={pillIcon} aria-label="Settings">
					<Settings />
				</Button>
			{/snippet}
		</Dialog.Trigger>
	</Tip>
	<Dialog.Content class="max-h-[85vh] overflow-y-auto sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Settings</Dialog.Title>
			<Dialog.Description>
				Terra Invicta Tools{#if app.meta?.gameVersion}, game data v{app.meta.gameVersion}{/if}
			</Dialog.Description>
		</Dialog.Header>
		<div class="grid gap-5 text-sm">
			<h2 class="font-semibold">General</h2>
			<div class="flex items-center justify-between gap-4">
				<span id="setting-language" class="font-medium">Game text language</span>
				<Select.Root
					type="single"
					value={app.scope.lang}
					disabled={!app.meta}
					onValueChange={(lang) => {
						app.scope = { ...app.scope, lang };
					}}
				>
					<Select.Trigger size="sm" aria-labelledby="setting-language"
						>{LANGUAGES[app.scope.lang] ?? app.scope.lang}</Select.Trigger
					>
					<Select.Content
						><Select.Group>
							{#each app.meta?.languages ?? [] as lang (lang)}<Select.Item
									value={lang}
									label={LANGUAGES[lang] ?? lang}>{LANGUAGES[lang] ?? lang}</Select.Item
								>{/each}
						</Select.Group></Select.Content
					>
				</Select.Root>
			</div>
			{#if app.meta}
				<h2 class="font-semibold">Game data</h2>
				<div class="flex items-start justify-between gap-4">
					<div class="grid min-w-0 gap-1">
						<span id="setting-data-dir" class="font-medium">Data folder</span>
						<span class="truncate text-xs text-muted-foreground" title={app.meta.dataDir}
							>{app.meta.dataDir}</span
						>
						{#if app.meta.dataDirSource === 'env'}
							<span class="text-xs text-muted-foreground"
								>TI_DATA_DIR overrides the saved folder. Change the environment variable to use
								another folder.</span
							>
						{/if}
					</div>
					<Button
						variant="outline"
						size="sm"
						aria-describedby="setting-data-dir"
						disabled={switching || extraction.running || app.meta.dataDirSource === 'env'}
						onclick={() => run(() => chooseDataDir(app.meta?.dataDir))}>Change…</Button
					>
				</div>
				{#if builds.length > 1}
					<div class="flex items-center justify-between gap-4">
						<span id="setting-build" class="font-medium">Game build</span>
						<Select.Root
							type="single"
							value={buildValue}
							disabled={switching || extraction.running}
							onValueChange={(value) => {
								if (value !== buildValue) run(() => switchBuild(value === LATEST ? null : value));
							}}
						>
							<Select.Trigger size="sm" aria-labelledby="setting-build"
								>{buildTriggerLabel}</Select.Trigger
							>
							<Select.Content
								><Select.Group>
									<Select.Item value={LATEST} label="Always the latest"
										>Always the latest</Select.Item
									>
									{#each builds as build (build.buildId)}<Select.Item
											value={build.buildId}
											label={buildLabel(build, build.buildId)}
											>{buildLabel(build, build.buildId)}{#if build.latest}<span
													class="ml-auto text-xs text-muted-foreground">latest</span
												>{/if}</Select.Item
										>{/each}
								</Select.Group></Select.Content
							>
						</Select.Root>
					</div>
				{/if}
				<div class="flex items-start justify-between gap-4">
					<div class="grid min-w-0 gap-1">
						<span id="setting-extract" class="font-medium">Game install</span>
						<span class="text-xs leading-relaxed text-muted-foreground">
							{#if extraction.game}
								{gameLabel(extraction.game)}. Extracting again replaces the data of this build.
							{:else if extraction.detected}
								No Terra Invicta install found.
							{:else}
								Looking for Terra Invicta…
							{/if}
						</span>
					</div>
					<Button
						variant="outline"
						size="sm"
						aria-describedby="setting-extract"
						disabled={switching || extraction.running || !extraction.game}
						onclick={extractAgain}><RotateCcw /> Extract again</Button
					>
				</div>
				<ExtractOptions disabled={extraction.running} />
				{#if showProgress}
					<ExtractionProgress
						onRetry={() => extraction.lastArgs && void extract(extraction.lastArgs)}
						onBack={() => extraction.dismissError()}
					/>
				{/if}
				{#if builds.length > 1}
					<div class="grid gap-2">
						<span id="setting-stored-builds" class="font-medium">Extracted builds</span>
						<ul class="grid gap-1" aria-labelledby="setting-stored-builds">
							{#each builds as build (build.dir)}
								<li class="flex items-center justify-between gap-3 rounded-md border px-2.5 py-1.5">
									<span class="grid min-w-0">
										<span class="truncate">{buildLabel(build, build.buildId)}</span>
										<span class="text-xs text-muted-foreground"
											>{sizes[build.dir] === undefined ? '…' : formatBytes(sizes[build.dir])}</span
										>
									</span>
									{#if build.loaded}
										<Badge variant="secondary">Loaded</Badge>
									{:else}
										<Button
											variant="ghost"
											size="sm"
											aria-label="Delete build {build.buildId}"
											disabled={switching || extraction.running}
											onclick={() => (deleting = build)}><Trash2 /> Delete</Button
										>
									{/if}
								</li>
							{/each}
						</ul>
					</div>
				{/if}
				{#if switching}
					<p class="flex items-center gap-2 text-muted-foreground" role="status">
						<LoaderCircle class="size-4 animate-spin" /> Loading game data…
					</p>
				{/if}
				{#if dataError}
					<p class="text-destructive" role="alert">{dataError}</p>
				{/if}
			{/if}
			<h2 class="font-semibold">Wiki</h2>
			<div class="flex items-start justify-between gap-4">
				<Label for="setting-wiki-spoilers" class="grid gap-2 font-normal"
					><span class="font-medium">Show spoilers in wiki</span><span
						class="leading-relaxed text-muted-foreground">alien and story content</span
					></Label
				>
				<Switch id="setting-wiki-spoilers" bind:checked={settings.wikiSpoilers} />
			</div>
			<h2 class="font-semibold">Tree display</h2>
			<div class="flex items-start justify-between gap-4">
				<Label for="setting-redundant" class="grid gap-2 font-normal">
					<span class="font-medium">Hide redundant links</span>
					<span class="leading-relaxed text-muted-foreground">
						Links already implied by another chain of prerequisites. Still shown when a tech is
						selected.
					</span>
				</Label>
				<Switch id="setting-redundant" bind:checked={settings.hideRedundantLinks} />
			</div>
			<div class="flex items-start justify-between gap-4">
				<Label for="setting-merge-links" class="grid gap-2 font-normal">
					<span class="font-medium">Merge long links</span>
					<span class="leading-relaxed text-muted-foreground">
						Links spanning several columns share lines, so the tree is more compact. Off, every link
						gets a lane of its own.
					</span>
				</Label>
				<Switch id="setting-merge-links" bind:checked={settings.mergeLinks} />
			</div>
			<div class="flex items-start justify-between gap-4">
				<Label for="setting-filter-mode" class="grid gap-2 font-normal">
					<span class="font-medium">Highlight filter matches</span>
					<span class="leading-relaxed text-muted-foreground">
						Filters dim the rest of the tree instead of hiding it, so the layout stays put.
					</span>
				</Label>
				<Switch
					id="setting-filter-mode"
					bind:checked={
						() => settings.filterMode === 'highlight',
						(on) => (settings.filterMode = on ? 'highlight' : 'hide')
					}
				/>
			</div>
			<div class="flex items-start justify-between gap-4">
				<Label for="setting-overview" class="grid gap-2 font-normal">
					<span class="font-medium">Overview when zoomed out</span>
					<span class="leading-relaxed text-muted-foreground">
						Far out, replace cards with plain category blocks.
					</span>
				</Label>
				<Switch id="setting-overview" bind:checked={settings.overviewLabels} />
			</div>
			<div class="flex items-start justify-between gap-4">
				<Label for="setting-landmarks" class="grid gap-2 font-normal">
					<span class="font-medium">Landmark labels when zoomed out</span>
					<span class="leading-relaxed text-muted-foreground">
						Too far out to read cards, name key research where there is room.
					</span>
				</Label>
				<Switch id="setting-landmarks" bind:checked={settings.landmarkLabels} />
			</div>
			<div class="flex items-center justify-between gap-4">
				<span id="setting-minimap-corner" class="font-medium">Minimap position</span>
				<ToggleGroup.Root
					type="single"
					size="sm"
					variant="outline"
					value={settings.minimapCorner}
					onValueChange={(v) => v && (settings.minimapCorner = v as Corner)}
					aria-labelledby="setting-minimap-corner"
				>
					{#each CORNER_OPTIONS as c (c.value)}
						<ToggleGroup.Item
							value={c.value}
							aria-label={c.label}
							title={c.label}
							class="first:rounded-l-md last:rounded-r-md"
						>
							<c.icon />
						</ToggleGroup.Item>
					{/each}
				</ToggleGroup.Root>
			</div>
			<h2 class="font-semibold">About</h2>
			<div class="grid gap-1 text-xs leading-relaxed text-muted-foreground">
				<span
					><span class="font-medium text-foreground">Terra Invicta Tools {__APP_VERSION__}</span>,
					MIT License</span
				>
				<span class="select-text">github.com/0xV1p3r/terra-invicta-tools</span>
				<span
					>A fan project with no connection to Pavonis Interactive or Hooded Horse. Game data and
					images belong to their owners and are read from your own installed copy.</span
				>
			</div>
		</div>
	</Dialog.Content>
</Dialog.Root>

<AlertDialog.Root
	open={deleting !== null}
	onOpenChange={(next) => {
		if (!next) deleting = null;
	}}
>
	<AlertDialog.Content>
		{#if deleting}
			{@const build = deleting}
			<AlertDialog.Header>
				<AlertDialog.Title>Delete build {build.buildId}?</AlertDialog.Title>
				<AlertDialog.Description>
					This removes {deleteSummary(build)} from the data folder. You can only extract it again while
					the game is at that build.
				</AlertDialog.Description>
			</AlertDialog.Header>
			<AlertDialog.Footer>
				<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
				<AlertDialog.Action variant="destructive" onclick={() => confirmDelete(build)}
					>Delete</AlertDialog.Action
				>
			</AlertDialog.Footer>
		{/if}
	</AlertDialog.Content>
</AlertDialog.Root>
