<script lang="ts">
	import { untrack } from 'svelte';
	import ArrowLeft from '@lucide/svelte/icons/arrow-left';
	import ArrowRight from '@lucide/svelte/icons/arrow-right';
	import BookOpen from '@lucide/svelte/icons/book-open';
	import EyeOff from '@lucide/svelte/icons/eye-off';
	import Wrench from '@lucide/svelte/icons/wrench';
	import { api, type EntityRef, type WikiEntry, type SearchHit } from '#lib/api.ts';
	import { wikiIcon } from '#lib/wiki-icons.ts';
	import { SENTIMENT } from '#lib/effect-style.ts';
	import EffectIcon from '#lib/components/detail/EffectIcon.svelte';
	import { app } from '#lib/state.svelte.ts';
	import { settings } from '#lib/settings.svelte.ts';
	import { Switch } from '#lib/components/ui/switch/index.ts';
	import { Label } from '#lib/components/ui/label/index.ts';
	import { wiki } from '#lib/wiki.svelte.ts';
	import { isResearch, KIND_LABELS } from '#lib/format.ts';
	import { setEntityNavigator, setSpoilerPolicy } from '#lib/components/detail/navigation.ts';
	import EntityView from '#lib/components/detail/EntityView.svelte';
	import EntityIcon from '#lib/components/detail/EntityIcon.svelte';
	import * as Dialog from '#lib/components/ui/dialog/index.ts';
	import * as Card from '#lib/components/ui/card/index.ts';
	import Tip from '#lib/components/toolbar/Tip.svelte';
	import { Button } from '#lib/components/ui/button/index.ts';
	import { Badge } from '#lib/components/ui/badge/index.ts';
	import { Input } from '#lib/components/ui/input/index.ts';
	import { ScrollArea } from '#lib/components/ui/scroll-area/index.ts';
	import { Skeleton } from '#lib/components/ui/skeleton/index.ts';
	import * as Select from '#lib/components/ui/select/index.ts';
	setEntityNavigator((entity) => wiki.openEntity(entity.key, entity.spoiler));
	setSpoilerPolicy(() => settings.wikiSpoilers);
	$effect(() => {
		if (!settings.wikiSpoilers) untrack(() => wiki.hideSpoilers());
	});
	/** Header bar of each column: one height and border, so the three line up. */
	const columnHeader = 'flex h-14 shrink-0 items-center justify-between gap-2 border-b px-3';
	/** Entries fetched per request ("Show more" fetches the next page). */
	const PAGE = 200;
	let sectionFilter = $state('');
	let filter = $state('');
	/** `filter` once typing pauses: what the loaded entries match. */
	let appliedFilter = $state('');
	let query = $state('');
	/** Loaded pages of the section's entries matching `appliedFilter`. */
	let entries = $state.raw<WikiEntry[]>([]);
	/** All entries matching `appliedFilter`, loaded or not. */
	let total = $state(0);
	let loadingMore = $state(false);
	/** Bumped by every fresh entry request; responses of older ones are dropped. */
	let generation = 0;
	/** Section and scope of the loaded entries, so a filter change keeps them on screen. */
	let entriesFor = '';
	let hits = $state.raw<SearchHit[]>([]);
	let indexLoading = $state(false);
	let loading = $state(false);
	let searching = $state(false);
	let indexError = $state('');
	let error = $state('');
	let searchError = $state('');
	let indexRetry = $state(0);
	let entriesRetry = $state(0);
	let searchRetry = $state(0);
	let cursor = $state(0);
	const global = $derived(!!query.trim());
	const browsing = $derived(global || !!wiki.template);
	const groups = $derived([
		...new Set(
			wiki.sections
				.filter((s) => s.label.toLowerCase().includes(sectionFilter.toLowerCase()))
				.map((s) => s.group)
		)
	]);
	const section = $derived(
		wiki.sections.find(
			(s) => s.template === wiki.template && !!s.spoiler === !!wiki.section?.spoiler
		)
	);
	const sectionId = (s: { template: string; spoiler?: boolean }) => `${s.template}|${!!s.spoiler}`;
	const hiddenHits = $derived(
		settings.wikiSpoilers ? 0 : hits.filter((h) => h.entity.spoiler).length
	);
	const matching = $derived(
		global
			? hits
					.filter((h) => settings.wikiSpoilers || !h.entity.spoiler)
					.map((h): WikiEntry => ({ entity: h.entity, subtitle: null, effect: null }))
			: wiki.template
				? entries
				: []
	);
	const matchCount = $derived(global ? matching.length : total);
	const remaining = $derived(global ? 0 : total - entries.length);
	$effect(() => {
		const scope = $state.snapshot(app.dataScope);
		const spoilers = settings.wikiSpoilers;
		void indexRetry;
		if (!wiki.open) return;
		let stale = false;
		indexLoading = true;
		indexError = '';
		wiki.sections = [];
		api
			.getWikiIndex(scope, spoilers)
			.then(
				(result) => {
					if (!stale) {
						wiki.sections = result
							.filter((s) => spoilers || !s.spoiler)
							.sort(
								(a, b) =>
									(a.group === 'Technical' ? 2 : a.group === 'Spoilers' ? 1 : 0) -
									(b.group === 'Technical' ? 2 : b.group === 'Spoilers' ? 1 : 0)
							);
						untrack(() => {
							if (!wiki.template && !wiki.selected) {
								const initial =
									wiki.sections.find((s) => s.template === 'TITechTemplate' && !s.spoiler) ??
									wiki.sections.find((s) => s.count > 0 && !s.spoiler);
								if (initial)
									wiki.section = { template: initial.template, spoiler: !!initial.spoiler };
							}
						});
					}
				},
				(cause) => {
					if (!stale) indexError = String(cause);
				}
			)
			.finally(() => {
				if (!stale) indexLoading = false;
			});
		return () => {
			stale = true;
		};
	});
	$effect(() => {
		const text = filter;
		if (!text) {
			appliedFilter = '';
			return;
		}
		const timer = setTimeout(() => (appliedFilter = text), 150);
		return () => clearTimeout(timer);
	});
	$effect(() => {
		const scope = $state.snapshot(app.dataScope);
		void entriesRetry;
		const template = wiki.template;
		const spoiler = wiki.section?.spoiler ?? false;
		const text = appliedFilter;
		void settings.wikiSpoilers;
		if (!wiki.open || !template) return;
		const id = ++generation;
		const key = JSON.stringify([scope, template, spoiler]);
		// Another section starts empty; a new filter keeps the old matches until the new ones arrive.
		if (key !== entriesFor) {
			entries = [];
			total = 0;
			entriesFor = key;
		}
		loading = true;
		loadingMore = false;
		error = '';
		cursor = 0;
		api
			.getWikiEntries(scope, template, spoiler, { filter: text, offset: 0, limit: PAGE })
			.then(
				(page) => {
					if (id !== generation) return;
					entries = page.entries;
					total = page.total;
					for (const e of page.entries) wiki.remember(e.entity.key, !!e.entity.spoiler);
				},
				(cause) => {
					if (id === generation) error = String(cause);
				}
			)
			.finally(() => {
				if (id === generation) loading = false;
			});
		return () => {
			// Makes this request's late response stale.
			if (id === generation) generation++;
		};
	});
	/** Append the next page of the current section and filter. */
	function loadMore() {
		const template = wiki.template;
		if (!template || loading || loadingMore || entries.length >= total) return;
		const id = generation;
		loadingMore = true;
		api
			.getWikiEntries($state.snapshot(app.dataScope), template, wiki.section?.spoiler ?? false, {
				filter: appliedFilter,
				offset: entries.length,
				limit: PAGE
			})
			.then(
				(page) => {
					if (id !== generation) return;
					entries = [...entries, ...page.entries];
					total = page.total;
					for (const e of page.entries) wiki.remember(e.entity.key, !!e.entity.spoiler);
				},
				(cause) => {
					if (id === generation) error = String(cause);
				}
			)
			.finally(() => {
				if (id === generation) loadingMore = false;
			});
	}
	$effect(() => {
		const scope = $state.snapshot(app.dataScope);
		void searchRetry;
		const text = query.trim();
		hits = [];
		searchError = '';
		searching = false;
		cursor = 0;
		if (!wiki.open || !text) return;
		let stale = false;
		searching = true;
		const timer = setTimeout(() => {
			api
				.search(scope, text, 50, true)
				.then(
					(result) => {
						if (!stale) hits = result;
					},
					(cause) => {
						if (!stale) searchError = String(cause);
					}
				)
				.finally(() => {
					if (!stale) searching = false;
				});
		}, 150);
		return () => {
			stale = true;
			clearTimeout(timer);
		};
	});
	$effect(() => {
		void wiki.template;
		void wiki.section?.spoiler;
		filter = '';
	});
	function openEntity(entity: EntityRef) {
		if (!entity.spoiler || settings.wikiSpoilers) wiki.openEntity(entity.key, !!entity.spoiler);
	}
	/** Cursor keys from search and entry filter inputs; focused buttons keep their own activation. */
	function listKey(event: KeyboardEvent) {
		if (!(event.target instanceof HTMLInputElement)) return;
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			event.preventDefault();
			cursor = Math.max(
				0,
				Math.min(matching.length - 1, cursor + (event.key === 'ArrowDown' ? 1 : -1))
			);
			if (cursor >= matching.length - 1 && remaining > 0) loadMore();
			document.getElementById(`wiki-entry-${cursor}`)?.scrollIntoView({ block: 'nearest' });
		}
		if (event.key === 'Enter' && matching[cursor]) {
			event.preventDefault();
			openEntity(matching[cursor].entity);
		}
	}
	function shortcut(event: KeyboardEvent) {
		if (
			(event.ctrlKey || event.metaKey) &&
			event.shiftKey &&
			event.key.toLowerCase() === 'k' &&
			!event.repeat &&
			app.meta
		) {
			event.preventDefault();
			wiki.open = !wiki.open;
		}
	}
	function inTree() {
		if (!wiki.selected) return;
		wiki.open = false;
		app.navigate({
			key: wiki.selected,
			kind: wiki.selected.startsWith('TITechTemplate:') ? 'tech' : 'project'
		});
	}
</script>

{#snippet spoilerToggle(id: string)}<Label for={id} class="grid gap-1 text-xs"
		>Show spoilers in wiki<span class="font-normal text-muted-foreground"
			>Alien and story content.</span
		></Label
	><Switch {id} bind:checked={settings.wikiSpoilers} />{/snippet}
<svelte:window onkeydown={shortcut} />
<Tip text="Browse the wiki" shortcut="Ctrl+Shift+K">
	<Button
		variant="ghost"
		size="sm"
		class="size-8 gap-2 px-0 @min-[1800px]:w-auto @min-[1800px]:border @min-[1800px]:border-input @min-[1800px]:bg-input/30 @min-[1800px]:px-2.5 @min-[1800px]:shadow-2xs @min-[1800px]:dark:hover:bg-input/50"
		disabled={!app.meta}
		onclick={() => wiki.show()}
		aria-label="Browse the wiki"
		aria-keyshortcuts="Control+Shift+k Meta+Shift+k"
		><BookOpen /><span class="hidden @min-[1800px]:inline">Wiki</span></Button
	>
</Tip>
<Dialog.Root bind:open={wiki.open}>
	<!-- No backdrop blur, unlike the other dialogs: this one covers ~90% of the window, so the blur
	     would only show in a thin margin yet still cost a full-window backdrop-filter pass (slow on
	     WebKitGTK, see .wiki-dialog in app.css). A slightly stronger dim stands in for it. -->
	<Dialog.Content
		overlayClass="bg-black/30 supports-backdrop-filter:backdrop-blur-none"
		class="wiki-dialog top-[5vh] right-[2vw] bottom-[5vh] left-[2vw] flex w-auto max-w-none translate-x-0 translate-y-0 flex-col gap-0 overflow-hidden p-0 sm:max-w-none"
	>
		<Dialog.Header class="shrink-0 border-b p-4 pr-14">
			<Dialog.Title>Wiki</Dialog.Title><Dialog.Description
				>Browse game entities and their connections.</Dialog.Description
			>
			<Input
				bind:value={query}
				onkeydown={listKey}
				aria-describedby="wiki-search-hint"
				placeholder="Search all entity types…"
				aria-label="Search all wiki entities"
				class="focus-visible:border-ring focus-visible:ring-0 focus-visible:ring-offset-0"
			/>
			<Card.Description id="wiki-search-hint" class="text-xs"
				>↑↓ to move, Enter to open</Card.Description
			>
			{#if indexError}
				<div class="flex items-center gap-3">
					<p role="alert" class="text-sm text-destructive">Could not load wiki sections.</p>
					<Button variant="outline" size="sm" onclick={() => indexRetry++}>Retry</Button>
				</div>
			{/if}
		</Dialog.Header>
		<Card.Content class="flex min-h-0 flex-1 p-0">
			<Card.Root
				class="hidden w-[240px] shrink-0 gap-0 rounded-none border-0 border-r py-0 shadow-none ring-0 lg:flex"
			>
				<div class={columnHeader}>
					<Input
						bind:value={sectionFilter}
						placeholder="Filter sections…"
						aria-label="Filter wiki sections"
					/>
				</div>
				<ScrollArea class="min-h-0 flex-1"
					><Card.Content class="p-2">
						{#if indexLoading}<Skeleton class="h-24 w-full" />{/if}
						{#each groups as group (group)}
							<Card.Description class="px-2 pt-5 pb-1.5 text-xs font-medium"
								>{#if group === 'Spoilers'}<EyeOff
										class="mr-1 inline size-3 text-warning"
									/>{:else if group === 'Technical'}<Wrench
										class="mr-1 inline size-3"
									/>{/if}{group}</Card.Description
							>
							{#each wiki.sections.filter((s) => s.group === group && s.label
										.toLowerCase()
										.includes(sectionFilter.toLowerCase())) as item (sectionId(item))}
								{@const SectionIcon = wikiIcon(item.template, item.group)}
								<Button
									variant={wiki.template === item.template &&
									!!wiki.section?.spoiler === !!item.spoiler
										? 'secondary'
										: 'ghost'}
									class="{group === 'Technical'
										? 'text-muted-foreground'
										: group === 'Spoilers'
											? 'text-warning'
											: ''} h-auto min-h-9 w-full justify-start gap-2 text-left whitespace-normal"
									aria-current={wiki.template === item.template &&
									!!wiki.section?.spoiler === !!item.spoiler
										? 'page'
										: undefined}
									onclick={() => {
										wiki.show(item.template, !!item.spoiler);
										query = '';
									}}
									><SectionIcon /><Card.Content class="flex-1 p-0">{item.label}</Card.Content><Badge
										variant="secondary"
										class="text-[10px] text-muted-foreground">{item.count.toLocaleString()}</Badge
									></Button
								>
							{/each}{/each}
					</Card.Content></ScrollArea
				><Card.Footer class="flex items-center justify-between gap-2 border-t p-3"
					>{@render spoilerToggle('wiki-spoilers')}</Card.Footer
				></Card.Root
			>
			<Card.Root
				class="w-full shrink-0 gap-0 rounded-none border-0 py-0 shadow-none ring-0 md:w-[320px] md:border-r {wiki.selected
					? 'max-md:hidden'
					: ''}"
				onkeydown={listKey}
			>
				<div class={columnHeader}>
					<div class="grid min-w-0 flex-1 gap-1">
						<Card.Title class="truncate leading-tight"
							>{global ? 'Search results' : (section?.label ?? 'Entries')}</Card.Title
						>{#if browsing}<Card.Description class="text-xs"
								>{matchCount.toLocaleString()} entries</Card.Description
							>{/if}{#if global && hiddenHits}<Card.Description class="text-xs"
								>{hiddenHits} spoiler results hidden</Card.Description
							>{/if}
					</div>
				</div>
				<!-- Narrow windows only: the sidebar's spoiler switch and sections; always: the filter. -->
				<div class="grid shrink-0 gap-2 p-2 {!global && wiki.template ? '' : 'lg:hidden'}">
					<div class="flex items-center justify-between gap-2 px-1 lg:hidden">
						{@render spoilerToggle('wiki-spoilers-mobile')}
					</div>
					<!-- Narrow windows: the section list is collapsed into this select. -->
					<Select.Root
						type="single"
						value={wiki.section ? sectionId(wiki.section) : undefined}
						onValueChange={(t) => {
							const item = wiki.sections.find((s) => sectionId(s) === t);
							if (item) wiki.show(item.template, !!item.spoiler);
							query = '';
						}}
					>
						<Select.Trigger class="w-full lg:hidden" aria-label="Section"
							>{section?.label ?? 'Choose a section'}</Select.Trigger
						>
						<Select.Content>
							{#each groups as group (group)}
								<Select.Group>
									<Select.GroupHeading>{group}</Select.GroupHeading>
									{#each wiki.sections.filter((s) => s.group === group) as item (sectionId(item))}
										<Select.Item value={sectionId(item)} label={item.label}
											>{item.label}
											<span class="ml-auto text-xs text-muted-foreground"
												>{item.count.toLocaleString()}</span
											></Select.Item
										>
									{/each}
								</Select.Group>
							{/each}
						</Select.Content>
					</Select.Root>
					{#if !global && wiki.template}<Input
							bind:value={filter}
							placeholder="Filter name or data name…"
							aria-label="Filter entries"
						/>{/if}
				</div>
				<ScrollArea class="min-h-0 flex-1"
					><Card.Content class="flex flex-col gap-1 p-2">
						{#if global ? searching : loading && !entries.length}<Skeleton
								class="h-24 w-full"
							/>{:else if global ? searchError : error}<Card.Description role="alert"
								>{global
									? `Could not search for “${query.trim()}”.`
									: `Could not load ${section?.label ?? 'this section'}.`}</Card.Description
							>
							<Button
								variant="outline"
								size="sm"
								onclick={() => {
									if (global) searchRetry++;
									else entriesRetry++;
								}}>Retry</Button
							>{:else if !browsing}<Card.Description class="p-2 leading-relaxed"
								>Choose a section or search all entries.</Card.Description
							>{:else if !matching.length && !(loading && !global)}<Card.Description
								>No entries found.</Card.Description
							>{/if}
						{#each matching as entry, i (entry.entity.key)}
							{@const entity = entry.entity}
							{@const tone = SENTIMENT[entry.effect?.sentiment ?? 'neutral']}
							<!-- Keyboard cursor (the row Enter opens): the hover fill, or the selected row's own
							     fill, plus an inset ring. -->
							<Button
								title={entity.dataName}
								id={`wiki-entry-${i}`}
								variant={wiki.selected === entity.key ? 'secondary' : 'ghost'}
								class="h-auto min-h-10 w-full justify-start gap-2 px-3 py-1.5 text-left whitespace-normal {cursor ===
								i
									? `ring-1 ring-ring ring-inset ${wiki.selected === entity.key ? '' : 'bg-accent dark:bg-accent/50'}`
									: ''}"
								aria-current={wiki.selected === entity.key ? 'page' : undefined}
								onclick={() => {
									cursor = i;
									openEntity(entity);
								}}
							>
								{#if entry.effect}<EffectIcon line={entry.effect} />{:else}<EntityIcon
										{entity}
										size={26}
										fallbackIcon={wikiIcon(
											entity.template,
											wiki.sections.find((s) => s.template === entity.template)?.group
										)}
									/>{/if}<Card.Content class="min-w-0 flex-1 p-0"
									><Card.Content class="flex items-start justify-between gap-2 p-0"
										><Card.Title
											class="text-sm leading-snug font-medium {entry.effect ? tone.text : ''}"
											>{entry.effect?.label ?? entity.name}</Card.Title
										>
										{#if entry.effect?.modifier}<Badge
												variant="outline"
												class="shrink-0 font-mono text-xs {tone.badge}"
												>{entry.effect.modifier}</Badge
											>{/if}</Card.Content
									>
									{#if entry.subtitle && !entry.effect}<Card.Description
											class="mt-0.5 truncate text-xs"
											title={entry.subtitle}>{entry.subtitle}</Card.Description
										>{/if}{#if cursor === i && wiki.selected !== entity.key}<Card.Description
											class="mt-1 text-xs">Enter to open</Card.Description
										>{/if}{#if global}<Card.Description class="text-xs"
											>{KIND_LABELS[entity.kind]} · {wiki.label(entity.template)}</Card.Description
										>{/if}</Card.Content
								>
							</Button>{/each}
						{#if remaining > 0 && !error}<Button
								variant="outline"
								disabled={loadingMore}
								onclick={loadMore}
								>{loadingMore
									? 'Loading…'
									: `Show more (${remaining.toLocaleString()} remaining)`}</Button
							>{/if}
					</Card.Content></ScrollArea
				></Card.Root
			>
			<Card.Root
				class="min-w-0 flex-1 gap-0 rounded-none border-0 py-0 shadow-none ring-0 {wiki.selected
					? ''
					: 'max-md:hidden'}"
			>
				<div class="{columnHeader} justify-start px-2">
					<Button variant="ghost" size="sm" class="md:hidden" onclick={() => wiki.closeEntity()}
						><ArrowLeft /> List</Button
					><Button
						variant="ghost"
						size="icon"
						aria-label="Back"
						disabled={!wiki.back.length}
						onclick={() => wiki.goBack()}><ArrowLeft /></Button
					><Button
						variant="ghost"
						size="icon"
						aria-label="Forward"
						disabled={!wiki.forward.length}
						onclick={() => wiki.goForward()}><ArrowRight /></Button
					>
					{#if wiki.selected && isResearch(wiki.selected.startsWith('TITechTemplate:') ? 'tech' : wiki.selected.startsWith('TIProjectTemplate:') ? 'project' : 'other')}<Button
							variant="outline"
							size="sm"
							onclick={inTree}>In tech tree</Button
						>{/if}
				</div>
				<ScrollArea class="min-h-0 flex-1"
					><Card.Content class="p-5"
						>{#if wiki.selected}<EntityView entityKey={wiki.selected} inWiki />{:else}<div
								class="flex flex-col items-center gap-3 py-16 text-center text-sm text-muted-foreground"
							>
								<BookOpen class="size-8 opacity-50" aria-hidden="true" />
								<p class="max-w-xs leading-relaxed">
									{browsing
										? 'Select an entry to read its details and follow its references.'
										: 'Choose a section or search all entries.'}
								</p>
							</div>{/if}</Card.Content
					></ScrollArea
				>
			</Card.Root>
		</Card.Content>
	</Dialog.Content>
</Dialog.Root>
