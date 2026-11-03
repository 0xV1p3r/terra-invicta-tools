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
}

export type Direction = 'up' | 'down' | 'both';

export interface SubtreeArgs {
	root: EntityKey;
	direction: Direction;
	alternatives: boolean;
	maxDepth: number | null;
}

export interface TreeGraph {
	nodes: TreeNode[];
	edges: TreeEdge[];
	nodeWidth: number;
	nodeHeight: number;
	root: EntityKey | null;
	totalCost: number;
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

export interface EntityDetail {
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
}

/** Outside Tauri (plain `pnpm dev` in a browser) the API is served from static fixtures. */
export const mockMode = !isTauri();

const tauriApi = {
	getMeta: () => invoke<Meta>('get_meta'),
	getTree: (scope: ScopeArgs, filter: TreeFilter) =>
		invoke<TreeGraph>('get_tree', { scope, filter }),
	getSubtree: (scope: ScopeArgs, args: SubtreeArgs) =>
		invoke<TreeGraph>('get_subtree', { scope, args }),
	getProperties: (scope: ScopeArgs) => invoke<PropertyInfo[]>('get_properties', { scope }),
	getEntity: (scope: ScopeArgs, key: EntityKey) =>
		invoke<EntityDetail>('get_entity', { scope, key }),
	search: (scope: ScopeArgs, query: string, limit = 30) =>
		invoke<SearchHit[]>('search', { scope, query, limit })
};

// --- Browser mock mode -------------------------------------------------------
// Fixtures live in `static/mock/` and ignore the scope. Missing fixtures reject
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
	const nodes = graph.nodes.filter(
		(n) =>
			(n.kind === 'tech' ? filter.techs : filter.projects) &&
			(filter.categories.length === 0 || filter.categories.includes(n.category)) &&
			(filter.properties.length === 0 || n.properties.some((p) => filter.properties.includes(p)))
	);
	const keys = new Set(nodes.map((n) => n.key));
	const edges = graph.edges.filter((e) => keys.has(e.from) && keys.has(e.to));
	return { ...graph, nodes, edges };
}

const mockApi: typeof tauriApi = {
	getMeta: () => fixture<Meta>('meta'),
	getTree: async (_scope, filter) => {
		const unfiltered = filter.categories.length === 0 && filter.properties.length === 0;
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
	getProperties: () => fixture<PropertyInfo[]>('properties'),
	getEntity: (_scope, key) => fixture<EntityDetail>(`entity-${key.replaceAll(':', '__')}`),
	search: async (_scope, query, limit = 30) => {
		const name = query.trim().replace(/\s+/g, '_');
		const hits = await fixture<SearchHit[]>(`search-${name}`, `search-${name.toLowerCase()}`);
		return hits.slice(0, limit);
	}
};

export const api = mockMode ? mockApi : tauriApi;

/** URL for an asset path returned by the backend (served by the `tiasset` protocol). */
export function assetUrl(path: string | null): string | null {
	if (mockMode) return null;
	return path ? convertFileSrc(path, 'tiasset') : null;
}
