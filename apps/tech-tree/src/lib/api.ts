// Typed wrapper around the Tauri backend commands. Mirrors src-tauri/src/dto.rs
// one-to-one; change both together.
import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';

/** Stable entity address: `"<Template>:<dataName>"`, e.g. `"TITechTemplate:AdAstra"`. */
export type EntityKey = string;

export type TechCategory =
	| 'Energy'
	| 'InformationScience'
	| 'LifeScience'
	| 'Materials'
	| 'MilitaryScience'
	| 'SocialScience'
	| 'SpaceScience'
	| 'Xenology';

export interface ScopeArgs {
	/** TIMetaTemplate dataName (e.g. `ModernScenario`); null = overview over all scenarios. */
	scenario: string | null;
	/** TIFactionTemplate dataName (e.g. `ResistCouncil`); null = all factions. */
	faction: string | null;
	lang: string;
}

export interface Meta {
	buildId: string;
	gameVersion: string | null;
	dataDir: string;
	languages: string[];
	scenarios: ScenarioInfo[];
	factions: FactionInfo[];
}

export interface ScenarioInfo {
	name: string;
	label: string;
	listed: boolean;
	dlc: boolean;
	tags: string[];
}

export interface FactionInfo {
	name: string;
	label: string;
	icon: string | null;
}

export type NodeKind = 'tech' | 'project';

export interface TreeFilter {
	techs: boolean;
	projects: boolean;
	/** Empty = all categories. */
	categories: TechCategory[];
	/** Property ids; a node matches if it has any of them. Empty = no property filter. */
	properties: string[];
}

export type PropertyGroup = 'effect' | 'ability' | 'unlock';

/** A filterable property of techs/projects (effect it has, or kind of thing it unlocks). */
export interface PropertyInfo {
	/** `effect:<context>:up|down`, `effect:<context>` (abilities) or `unlock:<kind>`. */
	id: string;
	/** English label, e.g. "Increases control point capacity". */
	label: string;
	group: PropertyGroup;
	/** Techs/projects in scope that have it. */
	count: number;
	/** Rendered description of one effect giving it (effects only). */
	example: string | null;
	/** For the acting faction, in the labelled direction. */
	sentiment: EffectSentiment;
	/** Icon group; null for unlocks. */
	domain: EffectDomain | null;
	/** Research category for effects on one science. */
	category: TechCategory | null;
	/** Matching game icon (asset path; turn into a URL with `assetUrl`). */
	icon: string | null;
}

export type Direction = 'up' | 'down' | 'both';

export interface SubtreeArgs {
	root: EntityKey;
	direction: Direction;
	alternatives: boolean;
	maxDepth: number | null;
}

/** Layout of the full tree. */
export type FullLayout = 'layered' | 'bands';

export interface TreeGraph {
	nodes: TreeNode[];
	edges: TreeEdge[];
	nodeWidth: number;
	nodeHeight: number;
	root: EntityKey | null;
	totalCost: number;
	/** Category bands, top to bottom (`bands` layout only). */
	bands: Band[];
	/** Focus view: requirement groups satisfied by any one of several cards. */
	anyOf: Span[];
}

/** Vertical extent of a category band (cards and edge routes), in px. */
export interface Band {
	category: TechCategory;
	top: number;
	bottom: number;
}

/** Vertical extent of a group of cards in the column at `x`, in px. */
export interface Span {
	x: number;
	top: number;
	bottom: number;
	/** Nesting level of the bracket (0 next to the cards); overlapping groups alternate. */
	level: number;
}

export interface TreeNode {
	key: EntityKey;
	dataName: string;
	kind: NodeKind;
	name: string;
	category: TechCategory;
	cost: number;
	/** Layout column (0 = leftmost). */
	column: number;
	/** Top-left position in px. */
	x: number;
	y: number;
	flags: NodeFlags;
	/** Property ids (see `PropertyInfo`). */
	properties: string[];
}

export interface NodeFlags {
	scenarioSpecific: boolean;
	variesByScenario: boolean;
	dlc: boolean;
	factionLocked: boolean;
	endGame: boolean;
	repeatable: boolean;
	obtainable: boolean;
}

export interface TreeEdge {
	from: EntityKey;
	to: EntityKey;
	/** Via altPrereqN: render dashed. */
	alternative: boolean;
	/** Points `[x, y]` the edge passes between its endpoints, left to right (empty = direct). */
	points: [number, number][];
	/** Implied by another chain of listed prereqs: the UI may hide it. */
	redundant: boolean;
}

export type EntityKind =
	| 'tech'
	| 'project'
	| 'effect'
	| 'habModule'
	| 'shipPart'
	| 'org'
	| 'trait'
	| 'faction'
	| 'objective'
	| 'mission'
	| 'codex'
	| 'other';

export interface EntityRef {
	spoiler?: boolean;
	key: EntityKey;
	template: string;
	dataName: string;
	name: string;
	kind: EntityKind;
	subtype: string | null;
	category: TechCategory | null;
	/** Asset path; turn into a URL with `assetUrl`. */
	icon: string | null;
}

export interface SearchHit {
	entity: EntityRef;
	score: number;
	matched: 'name' | 'dataName' | 'text' | 'value';
}

export interface WikiSection {
	spoiler?: boolean;
	template: string;
	label: string;
	group: string;
	count: number;
}
export interface WikiEntry {
	entity: EntityRef;
	subtitle?: string | null;
	effect?: EffectLine | null;
}
export interface RefGroup {
	template: string;
	field: string;
	entities: EntityRef[];
}

export interface EntityDetail {
	effect?: EffectLine | null;
	image?: string | null;
	references: RefGroup[];
	referencedBy: RefGroup[];
	entity: EntityRef;
	summary: string | null;
	description: string | null;
	quote: string | null;
	annotations: Annotations;
	requirements: RequirementGroup[];
	/** Listed prereq names that don't exist in this scope (the game ignores them). */
	droppedPrereqs: string[];
	effects: EffectLine[];
	unlocks: UnlockItem[];
	unlockedBy: EntityRef[];
	/** Display stats for any wiki template. */
	stats: EntityStats | null;
	researchCost: number | null;
	totalCost: number | null;
	record: Record<string, unknown>;
}

export interface Annotations {
	scenarios: string[];
	scenarioSpecific: boolean;
	variesByScenario: boolean;
	dlc: boolean;
	factions: string[];
	factionAlways: string | null;
	requiresNation: string | null;
}

export interface RequirementGroup {
	/** Any one option satisfies the group. */
	options: EntityRef[];
	/** Parallel to `options`: option comes from altPrereqN. */
	alternative: boolean[];
	/** Prereqs that exist but can't be completed in this scope (other faction's project). */
	hidden: EntityRef[];
	/** altPrereqN names that don't exist in this scope. */
	unresolved: string[];
}

export type EffectSentiment = 'good' | 'bad' | 'neutral';

export type EffectDomain =
	| 'research'
	| 'priority'
	| 'mission'
	| 'control'
	| 'councilor'
	| 'army'
	| 'ship'
	| 'mining'
	| 'missionControl'
	| 'exploration'
	| 'transfer'
	| 'hab'
	| 'alien'
	| 'environment'
	| 'economy'
	| 'politics'
	| 'ability'
	| 'other';

export interface EffectSegment {
	/** `value` = a substituted number; `sprite` = inline game icon by name (`metal`, `water`, …). */
	kind: 'text' | 'value' | 'sprite';
	text: string;
	/** Image of a `sprite` (asset path; turn into a URL with `assetUrl`). */
	icon: string | null;
}

export interface EffectLine {
	entity: EntityRef;
	/** Rendered plain-text description (null if the game hides it). */
	text: string | null;
	/** The description split into text, numbers and inline sprites (empty if hidden). */
	segments: EffectSegment[];
	/** What the effect changes ("Materials research") or grants ("Can found outposts"). */
	label: string | null;
	/** Change of `label`: `+100%`, `−15%`, `+70`, or a new level `→ 12`. */
	modifier: string | null;
	/** For the acting faction. */
	sentiment: EffectSentiment;
	domain: EffectDomain;
	/** Research category for effects on one science. */
	category: TechCategory | null;
	/** Who it applies to, if not just the acting faction. */
	target: string | null;
	/** Matching game icon (asset path; turn into a URL with `assetUrl`); `domain` is the fallback. */
	icon: string | null;
}

export type UnlockKind =
	| 'research'
	| 'researchAlternative'
	| 'shipPart'
	| 'habModule'
	| 'org'
	| 'grantedOrg'
	| 'claim'
	| 'trait'
	| 'codex'
	| 'mission'
	| 'objective';

export interface UnlockItem {
	kind: UnlockKind;
	entity: EntityRef;
	/** Stats of unlocked hab modules, ship parts and orgs. */
	stats: EntityStats | null;
}

/** Every stat of a hab module, ship part or org, grouped for display (see `stats.rs`). */
export interface EntityStats {
	/** Short classifications ("Tier 2", "Station", "Fusion Thermal", "Criminal"). */
	tags: string[];
	groups: StatGroup[];
	/** Rules and conditions as sentences (special rules, eligibility, drive notes). */
	notes: string[];
	/** Related entities (missions granted, upgrade chain, required tech). */
	links: StatLinks[];
}

export interface StatGroup {
	/** "Combat", "Income / month", "Build materials", … */
	label: string;
	stats: Stat[];
}

export interface Stat {
	label: string;
	/** Formatted value with unit ("1,200 t", "+45", "35%"). */
	value: string;
	/** Randomized orgs: roll range and chance, e.g. "+0–4 · 50% chance". */
	detail: string | null;
	/** For the owning faction: `good`, `bad` or `neutral`. */
	sentiment: EffectSentiment;
	/** Game icon (asset path, like `EntityRef::icon`). */
	icon: string | null;
	/** Record field the value comes from. */
	field: string;
}

export interface StatLinks {
	label: string;
	entities: EntityRef[];
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

const tauriApi = {
	getWikiIndex: (scope: ScopeArgs, spoilers = false) =>
		invoke<WikiSection[]>('get_wiki_index', { scope, spoilers }),
	getWikiEntries: (scope: ScopeArgs, template: string, spoiler = false) =>
		invoke<WikiEntry[]>('get_wiki_entries', { scope, template, spoiler }),
	getMeta: () => invoke<Meta>('get_meta'),
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
		invoke<SearchHit[]>('search', { scope, query, limit, all, kinds })
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

const mockApi: typeof tauriApi = {
	getWikiIndex: async (_scope, spoilers = false) =>
		(await fixture<WikiSection[]>('wiki-index')).filter((s) => spoilers || !s.spoiler),
	getWikiEntries: async (_scope, template, spoiler = false) => {
		const entries = await fixture<(WikiEntry | EntityRef)[]>(
			`wiki-entries-${spoiler ? 'spoiler-' : ''}${template}`
		);
		return entries.map((entry): WikiEntry =>
			'entity' in entry ? entry : { entity: entry, subtitle: null, effect: null }
		);
	},
	getMeta: () => fixture<Meta>('meta'),
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
	}
};

export const api = mockMode ? mockApi : tauriApi;

/** URL for an asset path returned by the backend (served by the `tiasset` protocol). */
export function assetUrl(path: string | null): string | null {
	if (mockMode) return null;
	return path ? convertFileSrc(path, 'tiasset') : null;
}
