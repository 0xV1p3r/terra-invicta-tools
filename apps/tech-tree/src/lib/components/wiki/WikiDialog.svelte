<script lang="ts">
	import { untrack } from 'svelte';
	import { BookOpen, ArrowLeft, ArrowRight, EyeOff, Wrench } from '@lucide/svelte';
	import { api, type EntityRef, type WikiEntry, type SearchHit } from '$lib/api';
	import { wikiIcon } from '$lib/wiki-icons';
	import { SENTIMENT } from '$lib/effect-style';
	import EffectIcon from '$lib/components/detail/EffectIcon.svelte';
	import { app } from '$lib/state.svelte';
	import { settings } from '$lib/settings.svelte';
	import { Switch } from '$lib/components/ui/switch';
	import { Label } from '$lib/components/ui/label';
	import { wiki } from '$lib/wiki.svelte';
	import { isResearch, KIND_LABELS } from '$lib/format';
	import { setEntityNavigator, setSpoilerPolicy } from '$lib/components/detail/navigation';
	import EntityView from '$lib/components/detail/EntityView.svelte';
	import EntityIcon from '$lib/components/detail/EntityIcon.svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Card from '$lib/components/ui/card';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Input } from '$lib/components/ui/input';
	import { ScrollArea } from '$lib/components/ui/scroll-area';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import * as Select from '$lib/components/ui/select';
	setEntityNavigator((entity) => wiki.openEntity(entity.key, entity.spoiler));
	setSpoilerPolicy(() => settings.wikiSpoilers);
	$effect(() => {
		if (!settings.wikiSpoilers) untrack(() => wiki.hideSpoilers());
	});
	let sectionFilter = $state('');
	let filter = $state('');
	let query = $state('');
	let entries = $state.raw<WikiEntry[]>([]);
	let hits = $state.raw<SearchHit[]>([]);
	let indexLoading = $state(false);
	let loading = $state(false);
	let searching = $state(false);
	let indexError = $state('');
	let error = $state('');
	let searchError = $state('');
	let limit = $state(200);
	let cursor = $state(0);
	const global = $derived(!!query.trim());
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
			: entries.filter((e) =>
					`${e.entity.name} ${e.entity.dataName} ${e.subtitle ?? ''}`
						.toLowerCase()
						.includes(filter.toLowerCase())
				)
	);
	const visible = $derived(matching.slice(0, limit));
	$effect(() => {
		const scope = $state.snapshot(app.scope);
		const spoilers = settings.wikiSpoilers;
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
		const scope = $state.snapshot(app.scope);
		const template = wiki.template;
		const spoiler = wiki.section?.spoiler ?? false;
		if (!wiki.open || !template) return;
		let stale = false;
		entries = [];
		loading = true;
		error = '';
		filter = '';
		api
			.getWikiEntries(scope, template, spoiler)
			.then(
				(result) => {
					if (!stale) {
						entries = result;
						for (const e of result) wiki.remember(e.entity.key, !!e.entity.spoiler);
					}
				},
				(cause) => {
					if (!stale) error = String(cause);
				}
			)
			.finally(() => {
				if (!stale) loading = false;
			});
		return () => {
			stale = true;
		};
	});
	$effect(() => {
		const scope = $state.snapshot(app.scope);
		const text = query.trim();
		hits = [];
		searchError = '';
		searching = false;
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
		void matching;
		limit = 200;
		cursor = 0;
	});
	function openEntity(entity: EntityRef) {
		if (!entity.spoiler || settings.wikiSpoilers) wiki.openEntity(entity.key, !!entity.spoiler);
	}
	/** Cursor keys only from the filter input; focused buttons keep their own activation. */
	function listKey(event: KeyboardEvent) {
		if (!(event.target instanceof HTMLInputElement)) return;
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			event.preventDefault();
			cursor = Math.max(
				0,
				Math.min(matching.length - 1, cursor + (event.key === 'ArrowDown' ? 1 : -1))
			);
			limit = Math.max(limit, cursor + 1);
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
			>alien and story content</span
		></Label
	><Switch {id} bind:checked={settings.wikiSpoilers} />{/snippet}
<svelte:window onkeydown={shortcut} />
<Tooltip.Root
	><Tooltip.Trigger
		>{#snippet child({ props })}<Button
				{...props}
				variant="ghost"
				size="sm"
				class="rounded-full"
				disabled={!app.meta}
				onclick={() => wiki.show()}
				aria-keyshortcuts="Control+Shift+k Meta+Shift+k"><BookOpen /> Wiki</Button
			>{/snippet}</Tooltip.Trigger
	><Tooltip.Content>Browse the wiki (Ctrl/Cmd+Shift+K)</Tooltip.Content></Tooltip.Root
>
<Dialog.Root bind:open={wiki.open}>
	<Dialog.Content
		overlayClass="bg-black/50 supports-backdrop-filter:backdrop-blur-none"
		class="wiki-dialog top-[5vh] right-[2vw] bottom-[5vh] left-[2vw] flex w-auto max-w-none translate-x-0 translate-y-0 flex-col gap-0 overflow-hidden p-0 sm:max-w-none"
	>
		<Dialog.Header class="shrink-0 border-b p-4 pr-14">
			<Dialog.Title>Wiki</Dialog.Title><Dialog.Description
				>Browse game entities and their connections.</Dialog.Description
			>
			<Input
				bind:value={query}
				placeholder="Search all entity types…"
				aria-label="Search all wiki entities"
			/>
		</Dialog.Header>
		<Card.Content class="flex min-h-0 flex-1 p-0">
			<Card.Root
				class="hidden w-[240px] shrink-0 gap-0 rounded-none border-0 border-r py-0 shadow-none ring-0 lg:flex"
			>
				<Card.Header class="p-3"
					><Input
						bind:value={sectionFilter}
						placeholder="Filter sections…"
						aria-label="Filter wiki sections"
					/></Card.Header
				>
				<ScrollArea class="min-h-0 flex-1"
					><Card.Content class="p-2">
						{#if indexLoading}<Skeleton class="h-24 w-full" />{:else if indexError}<Card.Description
								role="alert">{indexError}</Card.Description
							>{/if}
						{#each groups as group (group)}
							<Card.Description
								class="px-2 pt-5 pb-2 text-[10px] font-semibold tracking-widest uppercase"
								>{#if group === 'Spoilers'}<EyeOff
										class="mr-1 inline size-3 text-amber-500"
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
											? 'text-amber-600'
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
				<Card.Header class="gap-2 p-3"
					><div class="flex items-center justify-between gap-2 lg:hidden">
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
						<Select.Trigger size="sm" class="w-full lg:hidden" aria-label="Section"
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
					</Select.Root><Card.Title
						>{global ? 'Search results' : (section?.label ?? 'Entries')}</Card.Title
					><Card.Description>{matching.length.toLocaleString()} entries</Card.Description
					>{#if global && hiddenHits}<Card.Description
							>{hiddenHits} spoiler results hidden</Card.Description
						>{/if}
					{#if !global}<Input
							bind:value={filter}
							placeholder="Filter name or data name…"
							aria-label="Filter entries"
						/>{/if}</Card.Header
				>
				<ScrollArea class="min-h-0 flex-1"
					><Card.Content class="flex flex-col gap-1 p-2">
						{#if global ? searching : loading}<Skeleton
								class="h-24 w-full"
							/>{:else if global ? searchError : error}<Card.Description role="alert"
								>{global ? searchError : error}</Card.Description
							>{:else if !matching.length}<Card.Description>No entries found.</Card.Description
							>{/if}
						{#each visible as entry, i (entry.entity.key)}
							{@const entity = entry.entity}
							{@const tone = SENTIMENT[entry.effect?.sentiment ?? 'neutral']}
							<Button
								title={entity.dataName}
								id={`wiki-entry-${i}`}
								variant={wiki.selected === entity.key || cursor === i ? 'secondary' : 'ghost'}
								class="h-auto min-h-16 w-full justify-start gap-3 px-3 py-2.5 text-left whitespace-normal"
								aria-current={wiki.selected === entity.key ? 'page' : undefined}
								onclick={() => {
									cursor = i;
									openEntity(entity);
								}}
							>
								{#if entry.effect}<EffectIcon line={entry.effect} />{:else}<EntityIcon
										{entity}
										size={32}
										fallbackIcon={wikiIcon(
											entity.template,
											wiki.sections.find((s) => s.template === entity.template)?.group
										)}
									/>{/if}<Card.Content class="min-w-0 flex-1 p-0"
									><Card.Content class="flex items-start justify-between gap-2 p-0"
										><Card.Title class="text-sm leading-snug {entry.effect ? tone.text : ''}"
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
										>{/if}{#if global}<Card.Description class="text-xs"
											>{KIND_LABELS[entity.kind]} · {wiki.label(entity.template)}</Card.Description
										>{/if}</Card.Content
								>
							</Button>{/each}
						{#if matching.length > limit}<Button variant="outline" onclick={() => (limit += 200)}
								>Show more ({matching.length - limit} remaining)</Button
							>{/if}
					</Card.Content></ScrollArea
				></Card.Root
			>
			<Card.Root
				class="min-w-0 flex-1 gap-0 rounded-none border-0 py-0 shadow-none ring-0 {wiki.selected
					? ''
					: 'max-md:hidden'}"
			>
				<Card.Header class="flex flex-row items-center gap-2 border-b p-2"
					><Button variant="ghost" size="sm" class="md:hidden" onclick={() => wiki.closeEntity()}
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
						>{/if}</Card.Header
				>
				<ScrollArea class="min-h-0 flex-1"
					><Card.Content class="p-5"
						>{#if wiki.selected}<EntityView entityKey={wiki.selected} inWiki />{:else}<Card.Title
								>{section?.label ?? 'Explore the wiki'}</Card.Title
							><Card.Description class="mt-3"
								>Select an entry to read its details and follow its references. {section?.count.toLocaleString() ??
									0} entries in this section.</Card.Description
							>{/if}</Card.Content
					></ScrollArea
				>
			</Card.Root>
		</Card.Content>
	</Dialog.Content>
</Dialog.Root>

<style>
	/* WebKitGTK rasterizes translated/zoomed dialog layers at fractional pixels. */
	:global(.wiki-dialog) {
		transform: none;
		--tw-enter-scale: 1;
		--tw-exit-scale: 1;
		animation-name: wiki-fade;
	}
	:global(.wiki-dialog[data-state='closed']) {
		animation-direction: reverse;
	}
	@keyframes wiki-fade {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}
</style>
