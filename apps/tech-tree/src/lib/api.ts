// Tauri command wrappers. DTOs are generated from src-tauri/src/dto.rs.
// Regenerate from the workspace root: cargo test -p ti-tech-tree export_bindings
import { Channel, convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';

import type { EntityKey } from './generated/EntityKey';
export type { EntityKey } from './generated/EntityKey';
export type { TechCategory } from './generated/TechCategory';
import type { ScopeArgs } from './generated/ScopeArgs';
export type { ScopeArgs } from './generated/ScopeArgs';
import type { Meta } from './generated/Meta';
export type { Meta } from './generated/Meta';
export type { DataDirSource } from './generated/DataDirSource';
import type { BuildInfo } from './generated/BuildInfo';
export type { BuildInfo } from './generated/BuildInfo';
import type { BuildSize } from './generated/BuildSize';
export type { BuildSize } from './generated/BuildSize';
import type { GameInfo } from './generated/GameInfo';
export type { GameInfo } from './generated/GameInfo';
export type { ImageFormat } from './generated/ImageFormat';
import type { ExtractArgs } from './generated/ExtractArgs';
export type { ExtractArgs } from './generated/ExtractArgs';
import type { ExtractStage } from './generated/ExtractStage';
export type { ExtractStage } from './generated/ExtractStage';
import type { ExtractEvent } from './generated/ExtractEvent';
export type { ExtractEvent } from './generated/ExtractEvent';
import type { ExtractResult } from './generated/ExtractResult';
export type { ExtractResult } from './generated/ExtractResult';
import type { ExtractStatus } from './generated/ExtractStatus';
export type { ExtractStatus } from './generated/ExtractStatus';
export type { ScenarioInfo } from './generated/ScenarioInfo';
export type { FactionInfo } from './generated/FactionInfo';
export type { NodeKind } from './generated/NodeKind';
import type { TreeFilter } from './generated/TreeFilter';
export type { TreeFilter } from './generated/TreeFilter';
export type { PropertyGroup } from './generated/PropertyGroup';
import type { PropertyInfo } from './generated/PropertyInfo';
export type { PropertyInfo } from './generated/PropertyInfo';
export type { Direction } from './generated/Direction';
import type { SubtreeArgs } from './generated/SubtreeArgs';
export type { SubtreeArgs } from './generated/SubtreeArgs';
import type { FullLayout } from './generated/FullLayout';
export type { FullLayout } from './generated/FullLayout';
import type { TreeGraph } from './generated/TreeGraph';
export type { TreeGraph } from './generated/TreeGraph';
export type { Band } from './generated/Band';
export type { Span } from './generated/Span';
import type { TreeNode } from './generated/TreeNode';
export type { TreeNode } from './generated/TreeNode';
export type { NodeFlags } from './generated/NodeFlags';
import type { TreeEdge } from './generated/TreeEdge';
export type { TreeEdge } from './generated/TreeEdge';
import type { EntityKind } from './generated/EntityKind';
export type { EntityKind } from './generated/EntityKind';
import type { EntityRef } from './generated/EntityRef';
export type { EntityRef } from './generated/EntityRef';
import type { SearchHit } from './generated/SearchHit';
export type { SearchHit } from './generated/SearchHit';
import type { WikiSection } from './generated/WikiSection';
export type { WikiSection } from './generated/WikiSection';
import type { WikiEntry } from './generated/WikiEntry';
export type { WikiEntry } from './generated/WikiEntry';
import type { WikiPage } from './generated/WikiPage';
export type { WikiPage } from './generated/WikiPage';
export type { RefGroup } from './generated/RefGroup';
import type { EntityDetail } from './generated/EntityDetail';
export type { EntityDetail } from './generated/EntityDetail';
export type { AnnotationsDto as Annotations } from './generated/AnnotationsDto';
export type { RequirementGroup } from './generated/RequirementGroup';
export type { EffectSentiment } from './generated/EffectSentiment';
export type { EffectDomain } from './generated/EffectDomain';
export type { EffectSegment } from './generated/EffectSegment';
export type { EffectLine } from './generated/EffectLine';
export type { UnlockKind } from './generated/UnlockKind';
export type { UnlockItem } from './generated/UnlockItem';
export type { EntityStats } from './generated/EntityStats';
export type { StatGroup } from './generated/StatGroup';
export type { Stat } from './generated/Stat';
export type { StatLinks } from './generated/StatLinks';
import type { SolarSystem } from './generated/SolarSystem';
export type { SolarSystem } from './generated/SolarSystem';
export type { BodyType } from './generated/BodyType';
export type { SpaceResource } from './generated/SpaceResource';
export type { ResourceRating } from './generated/ResourceRating';
export type { SpaceBodyInfo } from './generated/SpaceBodyInfo';
export type { OrbitElements } from './generated/OrbitElements';
export type { LagrangePointInfo } from './generated/LagrangePointInfo';
import type { Campaign } from './generated/Campaign';
export type { Campaign } from './generated/Campaign';
export type { Completion } from './generated/Completion';
export type { PlanPick } from './generated/PlanPick';
import type { PlanStatus } from './generated/PlanStatus';
export type { PlanStatus } from './generated/PlanStatus';
import type { PlanStep } from './generated/PlanStep';
export type { PlanStep } from './generated/PlanStep';
export type { PlanChoice } from './generated/PlanChoice';
import type { PlanResult } from './generated/PlanResult';
export type { PlanResult } from './generated/PlanResult';

/** Text filter and page of `getWikiEntries`. */
export interface WikiPageArgs {
	/** Case-insensitive substring of name, dataName or subtitle; empty = all. */
	filter?: string;
	offset?: number;
	/** Null = all remaining entries. */
	limit?: number | null;
}

/** The backend's tree filter rule for one node (see `get_tree` in src-tauri/src/commands.rs). */
export function matchesFilter(n: TreeNode, filter: TreeFilter): boolean {
	return (
		(n.kind === 'tech' ? filter.techs : filter.projects) &&
		(filter.categories.length === 0 || filter.categories.includes(n.category)) &&
		(filter.properties.length === 0 || n.properties.some((p) => filter.properties.includes(p)))
	);
}

/** Outside Tauri (plain `pnpm dev` in a browser) the API is served from static fixtures. */
export const mockMode = !isTauri();

/** Error message of a cancelled extraction (`extraction::CANCELLED` in the backend). */
export const EXTRACTION_CANCELLED = 'Extraction cancelled.';

const tauriApi = {
	getWikiIndex: (scope: ScopeArgs, spoilers = false) =>
		invoke<WikiSection[]>('get_wiki_index', { scope, spoilers }),
	getWikiEntries: (
		scope: ScopeArgs,
		template: string,
		spoiler = false,
		{ filter = '', offset = 0, limit = null }: WikiPageArgs = {}
	) => invoke<WikiPage>('get_wiki_entries', { scope, template, spoiler, filter, offset, limit }),
	/** Scenario and faction labels are in `lang` (English if the build lacks it). */
	getMeta: (lang = 'en') => invoke<Meta>('get_meta', { lang }),
	listBuilds: () => invoke<BuildInfo[]>('list_builds'),
	/** Load another build of the data folder (null = always the latest) and remember the choice. */
	setBuild: (buildId: string | null) => invoke<void>('set_build', { buildId }),
	/** Load the data folder at `path` (it must contain `index.json`) and remember it. */
	setDataDir: (path: string) => invoke<void>('set_data_dir', { path }),
	/** Disk usage of every build of the data folder (walks the folders: only when shown). */
	getBuildSizes: () => invoke<BuildSize[]>('get_build_sizes'),
	/** Delete a build (by `BuildInfo.dir`) of the data folder; the loaded one is refused. */
	deleteBuild: (dir: string) => invoke<void>('delete_build', { dir }),
	/** The install at `saved` (chosen earlier), else Steam's or `TI_GAME_DIR`; null if none. */
	detectGame: (saved: string | null = null) => invoke<GameInfo | null>('detect_game', { saved }),
	/** The install at `path`; rejects with a message when it is not one. */
	inspectGame: (path: string) => invoke<GameInfo>('inspect_game', { path }),
	/**
	 * Extract the game data, reporting progress to `onEvent`; resolves once the new build is
	 * loaded. Rejects with `EXTRACTION_CANCELLED` after `cancelExtraction`.
	 */
	extractGameData: (args: ExtractArgs, onEvent: (event: ExtractEvent) => void) => {
		const onProgress = new Channel<ExtractEvent>();
		onProgress.onmessage = onEvent;
		return invoke<ExtractResult>('extract_game_data', { args, onProgress });
	},
	cancelExtraction: () => invoke<void>('cancel_extraction'),
	/** Progress of the running extraction (also one started before a window reload); null if none. */
	extractionStatus: () => invoke<ExtractStatus | null>('extraction_status'),
	/** Native folder picker; null when dismissed. */
	pickFolder: async (title: string, defaultPath?: string) => {
		const { open } = await import('@tauri-apps/plugin-dialog');
		const path = await open({ directory: true, title, defaultPath });
		return typeof path === 'string' ? path : null;
	},
	/** Native file picker for a game save; null when dismissed. */
	pickSaveFile: async (title: string, defaultPath?: string) => {
		const { open } = await import('@tauri-apps/plugin-dialog');
		const path = await open({
			title,
			defaultPath,
			filters: [
				{ name: 'Terra Invicta saves', extensions: ['gz'] },
				{ name: 'All files', extensions: ['*'] }
			]
		});
		return typeof path === 'string' ? path : null;
	},
	/** `mergeEdges`: long edges run as shared lines (more compact, not traceable one by one). */
	getTree: (
		scope: ScopeArgs,
		filter: TreeFilter,
		layout: FullLayout = 'layered',
		mergeEdges = true
	) => invoke<TreeGraph>('get_tree', { scope, filter, layout, mergeEdges }),
	getSubtree: (scope: ScopeArgs, args: SubtreeArgs, mergeEdges = true) =>
		invoke<TreeGraph>('get_subtree', { scope, args, mergeEdges }),
	getFocus: (scope: ScopeArgs, root: EntityKey) => invoke<TreeGraph>('get_focus', { scope, root }),
	/** Properties of the full-tree research passing `filter` (its `properties` are ignored). */
	getProperties: (scope: ScopeArgs, filter: TreeFilter) =>
		invoke<PropertyInfo[]>('get_properties', { scope, filter }),
	getEntity: (scope: ScopeArgs, key: EntityKey) =>
		invoke<EntityDetail>('get_entity', { scope, key }),
	/**
	 * `all`: also other factions' projects (the wiki); otherwise only what the faction sees.
	 * `kinds`: only hits of these kinds (empty = any), applied before `limit`.
	 */
	search: (scope: ScopeArgs, query: string, limit = 30, all = false, kinds: EntityKind[] = []) =>
		invoke<SearchHit[]>('search', { scope, query, limit, all, kinds }),
	getSolarSystem: (scope: ScopeArgs) => invoke<SolarSystem>('get_solar_system', { scope }),
	/** Saved campaigns, most recently saved first. */
	listCampaigns: () => invoke<Campaign[]>('list_campaigns'),
	/** Save a campaign (`id: ''` creates one); resolves to it as saved (id, `updated`). */
	saveCampaign: (campaign: Campaign) => invoke<Campaign>('save_campaign', { campaign }),
	deleteCampaign: (id: string) => invoke<void>('delete_campaign', { id }),
	/** Status of every tech/project and the goals' plan, in the campaign's scenario and faction. */
	getPlan: (lang: string, campaign: Campaign) => invoke<PlanResult>('get_plan', { lang, campaign }),
	/** The plan's path (the research still needed for the goals) laid out as a tree. */
	getPlanGraph: (lang: string, campaign: Campaign, mergeEdges = true) =>
		invoke<TreeGraph>('get_plan_graph', { lang, campaign, mergeEdges }),
	/** Keys to mark completed for "researched up to here" (`key` last). */
	planUpTo: (campaign: Campaign, key: EntityKey) =>
		invoke<EntityKey[]>('plan_up_to', { campaign, key }),
	/** Completed research that loses a requirement when `key` is un-marked (not `key`). */
	planDependentsLost: (campaign: Campaign, key: EntityKey) =>
		invoke<EntityKey[]>('plan_dependents_lost', { campaign, key }),
	/** The game's save folder, if found. */
	defaultSaveDir: () => invoke<string | null>('default_save_dir'),
	/**
	 * The campaign as of the save at `path`: `into` updated from it (keeping its name, goals and
	 * choices), or a new one. Not saved yet.
	 */
	importSave: (lang: string, path: string, into: Campaign | null = null) =>
		invoke<Campaign>('import_save', { lang, path, into })
};

// --- Browser mock mode -------------------------------------------------------
// Fixtures live in `static/mock/` and ignore the scope (and edge merging: always on). Missing fixtures reject
// with an error message, like a failing backend command would.

async function fixture<T>(...names: string[]): Promise<T> {
	for (const name of names) {
		const res = await fetch(`/mock/${encodeURIComponent(name)}.json`);
		if (res.ok && res.headers.get('content-type')?.includes('json')) return (await res.json()) as T;
	}
	throw `Mock mode: no fixture ${names.map((n) => `static/mock/${n}.json`).join(' / ')}`;
}

/** Approximates the backend filter on the full fixture (keeps backend positions). */
function filterGraph(graph: TreeGraph, filter: TreeFilter): TreeGraph {
	const nodes = graph.nodes.filter((n) => matchesFilter(n, filter));
	const keys = new Set(nodes.map((n) => n.key));
	// The implying chain of a redundant edge may be filtered out (the backend does the same).
	const edges = graph.edges
		.filter((e) => keys.has(e.from) && keys.has(e.to))
		.map((e) => ({ ...e, redundant: false }));
	return { ...graph, nodes, edges };
}

/** Approximates `get_focus` from the full fixture (no requirement groups: no brackets). */
function focusFromTree(graph: TreeGraph, root: EntityKey): TreeGraph {
	const byKey = new Map(graph.nodes.map((n) => [n.key, n]));
	const center = byKey.get(root);
	if (!center) throw `Mock mode: ${root} is not in the tree fixture`;
	const step = graph.nodeWidth + 180;
	const rowHeight = graph.nodeHeight + 24;
	const left = graph.edges.filter((e) => e.to === root && e.from !== root);
	const right = graph.edges.filter((e) => e.from === root && e.to !== root);
	const tallest = Math.max(left.length, right.length, 1);
	const column = (edges: TreeEdge[], c: number, key: (e: TreeEdge) => EntityKey) =>
		edges.flatMap((e, i) => {
			const n = byKey.get(key(e));
			const offset = ((tallest - edges.length) * rowHeight) / 2;
			return n ? [{ ...n, column: c, x: c * step, y: offset + i * rowHeight }] : [];
		});
	const nodes = [
		{ ...center, column: 1, x: step, y: ((tallest - 1) * rowHeight) / 2 },
		...column(left, 0, (e) => e.from),
		...column(right, 2, (e) => e.to)
	];
	const edges = [...left, ...right].map((e) => ({ ...e, points: [], redundant: false }));
	return { ...graph, nodes, edges, root, bands: [], anyOf: [] };
}

/**
 * Mock onboarding for UI work: `?onboarding=detect|none|progress|error` starts without data
 * (`none`: no install found; `progress`: an extraction runs slowly; `error`: it fails), and
 * `?update` reports a newer game build than the fixtures. `?builds=2` adds an older build.
 */
const mockParams = new URLSearchParams(typeof location === 'undefined' ? '' : location.search);
export const mockOnboarding = mockMode
	? (['detect', 'none', 'progress', 'error'] as const).find(
			(m) => m === mockParams.get('onboarding')
		)
	: undefined;
let mockExtracted = !mockOnboarding;
let mockCancel = false;

const MOCK_GAME_ROOT = '/home/player/.local/share/Steam/steamapps/common/Terra Invicta';
const MOCK_OLD_BUILD = '18823307';

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Fake extraction: walks the stages with step counts; `error` mode fails during images. */
async function mockExtract(args: ExtractArgs, onEvent: (event: ExtractEvent) => void) {
	mockCancel = false;
	// [stage, steps, milliseconds, step label]; `progress` mode lingers in the images.
	const imagesMs = mockOnboarding === 'progress' ? 60_000 : 2500;
	const stages: [ExtractStage, number, number, (i: number) => string | null][] = [
		['templates', 60, 1200, (i) => `Templates/TITechTemplate${i}.json`],
		['locale', 14, 600, () => null],
		['schema', 1, 300, () => null],
		['images', args.largeArt ? 4000 : 1200, imagesMs, (i) => (i < 800 ? 'icons' : 'illustrations')],
		['finish', 1, 300, () => null]
	];
	for (const [stage, total, ms, label] of stages) {
		onEvent({ kind: 'stage', stage });
		for (let done = 1; done <= total; done++) {
			await sleep(ms / total);
			if (mockCancel) throw EXTRACTION_CANCELLED;
			if (mockOnboarding === 'error' && stage === 'images' && done > total / 3)
				throw 'Could not read bundle illustrations: unexpected end of file';
			onEvent({ kind: 'step', done, total, label: label(done) });
		}
	}
	mockExtracted = true;
	const meta = await fixture<Meta>('meta');
	return { buildId: meta.buildId, extracted: true, issues: 3, parseErrors: 0, seconds: 12.4 };
}

const mockApi: typeof tauriApi = {
	getWikiIndex: async (_scope, spoilers = false) =>
		(await fixture<WikiSection[]>('wiki-index')).filter((s) => spoilers || !s.spoiler),
	getWikiEntries: async (
		_scope,
		template,
		spoiler = false,
		{ filter = '', offset = 0, limit = null } = {}
	) => {
		const rows = await fixture<(WikiEntry | EntityRef)[]>(
			`wiki-entries-${spoiler ? 'spoiler-' : ''}${template}`
		);
		const needle = filter.toLowerCase();
		const entries = rows
			.map((entry): WikiEntry =>
				'entity' in entry ? entry : { entity: entry, subtitle: null, effect: null }
			)
			.filter((e) =>
				`${e.entity.name} ${e.entity.dataName} ${e.subtitle ?? ''}`.toLowerCase().includes(needle)
			);
		return {
			total: entries.length,
			entries: entries.slice(offset, limit === null ? undefined : offset + limit)
		};
	},
	// Older fixtures lack the build fields; mock data counts as the latest build of a found folder.
	getMeta: async () => {
		if (!mockExtracted) throw 'No extracted game data found. (mock onboarding)';
		return {
			latest: true,
			pinnedBuild: null,
			dataDirSource: 'found' as const,
			...(await fixture<Partial<Meta> & Omit<Meta, 'latest' | 'pinnedBuild' | 'dataDirSource'>>(
				'meta'
			))
		};
	},
	listBuilds: async () => {
		const meta = await fixture<Meta>('meta');
		const builds: BuildInfo[] = [
			{
				dir: meta.buildId,
				buildId: meta.buildId,
				lastUpdated: null,
				gameVersion: meta.gameVersion,
				loaded: true,
				latest: true
			}
		];
		if (mockParams.has('builds'))
			builds.unshift({
				dir: MOCK_OLD_BUILD,
				buildId: MOCK_OLD_BUILD,
				lastUpdated: 1751328000,
				gameVersion: '1.0.29',
				loaded: false,
				latest: false
			});
		return builds;
	},
	setBuild: async () => {
		throw 'Switching builds is not available in mock mode';
	},
	setDataDir: async () => {
		throw 'Choosing a data folder is not available in mock mode';
	},
	getBuildSizes: async () =>
		(await mockApi.listBuilds()).map((b, i) => ({ dir: b.dir, bytes: (i + 1) * 873_000_000 })),
	deleteBuild: async () => {
		throw 'Deleting builds is not available in mock mode';
	},
	// The fixtures' build, so a mock extraction yields the detected build (`?update`: a newer one).
	detectGame: async () => {
		if (mockOnboarding === 'none') return null;
		const meta = await fixture<Meta>('meta');
		return {
			root: MOCK_GAME_ROOT,
			version: meta.gameVersion,
			buildId: mockParams.has('update') ? '21474836' : meta.buildId
		};
	},
	inspectGame: async (path) => {
		if (!path.includes('Terra Invicta'))
			throw `${path} is not a Terra Invicta install. Choose the game folder, the one that contains TerraInvicta_Data.`;
		return { root: path, version: '1.0.53b', buildId: null };
	},
	extractGameData: mockExtract,
	cancelExtraction: async () => {
		mockCancel = true;
	},
	// A page reload restarts the mock: no extraction survives it.
	extractionStatus: async () => null,
	// No native dialog in a browser.
	pickFolder: async (title, defaultPath) => prompt(title, defaultPath ?? MOCK_GAME_ROOT),
	pickSaveFile: async (title, defaultPath) => prompt(title, defaultPath ?? 'ExitSave.gz'),
	getTree: async (_scope, filter, layout = 'layered') => {
		const unfiltered = filter.categories.length === 0 && filter.properties.length === 0;
		if (layout === 'bands') {
			const bands = await fixture<TreeGraph>('tree-bands');
			return unfiltered && filter.techs && filter.projects ? bands : filterGraph(bands, filter);
		}
		if (filter.techs && filter.projects && unfiltered) return fixture<TreeGraph>('tree');
		if (filter.techs && !filter.projects && unfiltered) {
			try {
				return await fixture<TreeGraph>('tree-techs');
			} catch {
				// fall through to filtering the full tree
			}
		}
		return filterGraph(await fixture<TreeGraph>('tree'), filter);
	},
	// Fixtures exist only for a few roots/queries, with alternatives on (see src-tauri/src/fixtures.rs).
	getSubtree: (_scope, args) => {
		const dataName = args.root.slice(args.root.indexOf(':') + 1);
		return fixture<TreeGraph>(`subtree-${args.direction}-${dataName}`);
	},
	// Fixtures exist for a few roots; others are approximated from the full tree.
	getFocus: (_scope, root) =>
		fixture<TreeGraph>(`focus-${root.slice(root.indexOf(':') + 1)}`).catch(async () =>
			focusFromTree(await fixture<TreeGraph>('tree'), root)
		),
	// Counts are recomputed from the tree fixture when other filters narrow it.
	getProperties: async (_scope, filter) => {
		const all = await fixture<PropertyInfo[]>('properties');
		const base = { ...filter, properties: [] };
		if (base.techs && base.projects && base.categories.length === 0) return all;
		const counts = new Map<string, number>();
		for (const n of (await fixture<TreeGraph>('tree')).nodes) {
			if (!matchesFilter(n, base)) continue;
			for (const p of n.properties) counts.set(p, (counts.get(p) ?? 0) + 1);
		}
		return all.flatMap((p) => (counts.has(p.id) ? [{ ...p, count: counts.get(p.id)! }] : []));
	},
	getEntity: (_scope, key) => fixture<EntityDetail>(`entity-${key.replaceAll(':', '__')}`),
	search: async (_scope, query, limit = 30, _all, kinds = []) => {
		const name = query.trim().replace(/\s+/g, '_');
		const hits = await fixture<SearchHit[]>(`search-${name}`, `search-${name.toLowerCase()}`);
		return hits.filter((h) => !kinds.length || kinds.includes(h.entity.kind)).slice(0, limit);
	},
	getSolarSystem: () => fixture<SolarSystem>('solar-system'),
	listCampaigns: async () => mockCampaigns().sort((a, b) => b.updated - a.updated),
	saveCampaign: async (campaign) => {
		const saved = {
			...campaign,
			id: campaign.id || `mock-${Date.now().toString(36)}`,
			name: campaign.name.trim(),
			updated: Math.floor(Date.now() / 1000)
		};
		if (!saved.name) throw 'A campaign needs a name.';
		storeMockCampaigns([...mockCampaigns().filter((c) => c.id !== saved.id), saved]);
		return saved;
	},
	defaultSaveDir: async () => null,
	importSave: async () => {
		throw 'Importing saves is not available in mock mode';
	},
	deleteCampaign: async (id) => storeMockCampaigns(mockCampaigns().filter((c) => c.id !== id)),
	getPlan: async (_lang, campaign) => mockPlan(await fixture<TreeGraph>('tree'), campaign),
	getPlanGraph: async (_lang, campaign) =>
		mockPlanGraph(await fixture<TreeGraph>('tree'), campaign),
	planUpTo: async (campaign, key) => {
		const graph = await fixture<TreeGraph>('tree');
		const done = new Set(campaign.completed.map((c) => c.key));
		return mockChain(graph, [key], done);
	},
	planDependentsLost: async (campaign, key) => {
		const graph = await fixture<TreeGraph>('tree');
		const done = new Set(campaign.completed.map((c) => c.key));
		done.delete(key);
		const lost: EntityKey[] = [];
		for (let changed = true; changed;) {
			changed = false;
			for (const k of done) {
				if (mockRequires(graph, k).some((p) => !done.has(p))) {
					done.delete(k);
					lost.push(k);
					changed = true;
				}
			}
		}
		return lost;
	}
};

// Mock campaigns live in localStorage; plans approximate the backend from the tree fixture's
// listed (non-alternative) edges: no "any of" choices, no repeat costs.
const MOCK_CAMPAIGNS = 'ti-tech-tree:mock-campaigns';

function mockCampaigns(): Campaign[] {
	try {
		return JSON.parse(localStorage.getItem(MOCK_CAMPAIGNS) ?? '[]') as Campaign[];
	} catch {
		return [];
	}
}

function storeMockCampaigns(list: Campaign[]) {
	try {
		localStorage.setItem(MOCK_CAMPAIGNS, JSON.stringify(list));
	} catch {
		// Storage unavailable: mock campaigns are lost on reload.
	}
}

const mockRequires = (graph: TreeGraph, key: EntityKey) =>
	graph.edges.filter((e) => e.to === key && !e.alternative).map((e) => e.from);

/** Missing listed prerequisites of `goals`, prerequisites first (goals included). */
function mockChain(graph: TreeGraph, goals: EntityKey[], done: Set<EntityKey>): EntityKey[] {
	const out: EntityKey[] = [];
	const seen = new Set<EntityKey>();
	const visit = (key: EntityKey) => {
		if (done.has(key) || seen.has(key)) return;
		seen.add(key);
		for (const p of mockRequires(graph, key)) visit(p);
		out.push(key);
	};
	for (const g of goals) visit(g);
	return out;
}

function mockPlan(graph: TreeGraph, campaign: Campaign): PlanResult {
	const byKey = new Map(graph.nodes.map((n) => [n.key, n]));
	const done = new Set(campaign.completed.map((c) => c.key));
	const statusOf = (key: EntityKey): PlanStatus =>
		done.has(key)
			? 'done'
			: !byKey.get(key)?.flags.obtainable
				? 'unobtainable'
				: mockRequires(graph, key).every((p) => done.has(p))
					? 'available'
					: 'locked';
	const status: Record<EntityKey, PlanStatus> = {};
	for (const n of graph.nodes) status[n.key] = statusOf(n.key);
	const goals = campaign.goals.filter((g) => byKey.has(g));
	const keys = mockChain(graph, goals, done);
	const inPlan = new Set(keys);
	const neededBy = new Map<EntityKey, EntityKey[]>();
	for (const k of keys)
		for (const p of mockRequires(graph, k))
			if (inPlan.has(p)) neededBy.set(p, [...(neededBy.get(p) ?? []), k]);
	const unblocks = (key: EntityKey) => {
		const seen = new Set<EntityKey>();
		const stack = [key];
		for (let k = stack.pop(); k !== undefined; k = stack.pop())
			for (const d of neededBy.get(k) ?? [])
				if (!seen.has(d)) {
					seen.add(d);
					stack.push(d);
				}
		return seen.size;
	};
	const steps: PlanStep[] = keys.map((key) => {
		const n = byKey.get(key)!;
		return {
			entity: {
				spoiler: false,
				key,
				template: key.slice(0, key.indexOf(':')),
				dataName: n.dataName,
				name: n.name,
				kind: n.kind,
				subtype: null,
				category: n.category as PlanStep['entity']['category'],
				icon: null
			},
			cost: n.cost,
			progress: 0,
			active: false,
			status: status[key],
			unblocks: unblocks(key),
			goal: goals.includes(key)
		} as PlanStep;
	});
	const cost = (kind: string) =>
		steps.filter((s) => s.entity.kind === kind).reduce((sum, s) => sum + s.cost, 0);
	return {
		status,
		steps,
		next: steps
			.map((s, i) => ({ s, i }))
			.filter(({ s }) => s.status === 'available')
			.sort((a, b) => b.s.unblocks - a.s.unblocks || a.i - b.i)
			.map(({ s }) => s.entity.key),
		choices: [],
		techCost: cost('tech'),
		projectCost: cost('project'),
		researchSpeed: 1,
		unreachable: [],
		unknown: [...campaign.completed.map((c) => c.key), ...campaign.goals].filter(
			(k) => !byKey.has(k)
		)
	};
}

/** The plan's steps in columns by prerequisite depth (direct edges, no routing). */
function mockPlanGraph(graph: TreeGraph, campaign: Campaign): TreeGraph {
	const plan = mockPlan(graph, campaign);
	const keys = new Set(plan.steps.map((s) => s.entity.key));
	const edges = graph.edges
		.filter((e) => keys.has(e.from) && keys.has(e.to))
		.map((e) => ({ ...e, redundant: false, points: [] }));
	const depth = new Map<EntityKey, number>();
	// Steps come prerequisites first, so each one's depth is known when it is reached.
	for (const s of plan.steps) {
		const parents = edges.filter((e) => e.to === s.entity.key).map((e) => depth.get(e.from) ?? 0);
		depth.set(s.entity.key, parents.length ? Math.max(...parents) + 1 : 0);
	}
	const rows = new Map<number, number>();
	const nodes = graph.nodes
		.filter((n) => keys.has(n.key))
		.map((n) => {
			const column = depth.get(n.key) ?? 0;
			const row = rows.get(column) ?? 0;
			rows.set(column, row + 1);
			return { ...n, column, x: column * graph.nodeWidth * 1.6, y: row * graph.nodeHeight * 1.5 };
		});
	return {
		...graph,
		nodes,
		edges,
		root: null,
		bands: [],
		anyOf: [],
		totalCost: plan.steps.reduce((sum, s) => sum + s.cost, 0)
	};
}

export const api = mockMode ? mockApi : tauriApi;

/** URL for an asset path returned by the backend (served by the `tiasset` protocol). */
export function assetUrl(path: string | null): string | null {
	if (mockMode) return null;
	return path ? convertFileSrc(path, 'tiasset') : null;
}
