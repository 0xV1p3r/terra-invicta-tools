// Tauri command wrappers. DTOs are generated from src-tauri/src/dto.rs.
// Regenerate from the workspace root: cargo test -p ti-tech-tree export_bindings
import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';

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
	getSolarSystem: (scope: ScopeArgs) => invoke<SolarSystem>('get_solar_system', { scope })
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
	getMeta: async () => ({
		latest: true,
		pinnedBuild: null,
		dataDirSource: 'found' as const,
		...(await fixture<Partial<Meta> & Omit<Meta, 'latest' | 'pinnedBuild' | 'dataDirSource'>>(
			'meta'
		))
	}),
	listBuilds: async () => {
		const meta = await fixture<Meta>('meta');
		return [
			{
				dir: meta.buildId,
				buildId: meta.buildId,
				lastUpdated: null,
				gameVersion: meta.gameVersion,
				loaded: true,
				latest: true
			}
		];
	},
	setBuild: async () => {
		throw 'Switching builds is not available in mock mode';
	},
	setDataDir: async () => {
		throw 'Choosing a data folder is not available in mock mode';
	},
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
	getSolarSystem: () => fixture<SolarSystem>('solar-system')
};

export const api = mockMode ? mockApi : tauriApi;

/** URL for an asset path returned by the backend (served by the `tiasset` protocol). */
export function assetUrl(path: string | null): string | null {
	if (mockMode) return null;
	return path ? convertFileSrc(path, 'tiasset') : null;
}
