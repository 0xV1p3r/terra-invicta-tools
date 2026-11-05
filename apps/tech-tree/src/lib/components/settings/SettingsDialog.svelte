<script lang="ts">
	import ArrowDownLeft from '@lucide/svelte/icons/arrow-down-left';
	import ArrowDownRight from '@lucide/svelte/icons/arrow-down-right';
	import ArrowUpLeft from '@lucide/svelte/icons/arrow-up-left';
	import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right';
	import Tip from '$lib/components/toolbar/Tip.svelte';
	import Settings from '@lucide/svelte/icons/settings';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Select from '$lib/components/ui/select/index.js';
	import { Switch } from '$lib/components/ui/switch/index.js';
	import * as ToggleGroup from '$lib/components/ui/toggle-group/index.js';
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

	const CORNER_OPTIONS = [
		{ value: 'top-left', label: 'Top left', icon: ArrowUpLeft },
		{ value: 'top-right', label: 'Top right', icon: ArrowUpRight },
		{ value: 'bottom-left', label: 'Bottom left', icon: ArrowDownLeft },
		{ value: 'bottom-right', label: 'Bottom right', icon: ArrowDownRight }
	] as const;
</script>

<Dialog.Root>
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
				Terra Invicta Tech Tree{#if app.meta?.gameVersion}, game data v{app.meta.gameVersion}{/if}
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
		</div>
	</Dialog.Content>
</Dialog.Root>
