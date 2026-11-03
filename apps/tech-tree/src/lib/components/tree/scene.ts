// Render-ready form of a TreeGraph: per-node geometry and labels, edge paths, bounds and
// a grid for hit-testing/culling. Built once per loaded graph.
import type { EntityKey, TreeGraph, TreeNode } from '$lib/api';
import { CATEGORIES } from '$lib/categories';
import { formatCost } from '$lib/format';
import { activeFlags, type FlagInfo } from './flags';
import type { Rect } from './camera';

export interface SceneNode {
	item: TreeNode;
	key: EntityKey;
	x: number;
	y: number;
	/** Category color (hex). */
	color: string;
	cost: string;
	kind: string;
	flags: FlagInfo[];
	/** Truncated/wrapped labels, filled lazily by the renderer per font. */
	labels: Map<string, string[]>;
}

export interface EdgePaths {
	/** Listed prerequisites. */
	solid: Path2D;
	/** `altPrereqN` alternatives (drawn dashed). */
	dashed: Path2D;
}

export interface Scene {
	graph: TreeGraph;
	nodes: SceneNode[];
	byKey: Map<EntityKey, SceneNode>;
	width: number;
	height: number;
	bounds: Rect;
	edges: EdgePaths;
	/** Edges touching a node (for highlighting the selection). */
	edgesOf: Map<EntityKey, EdgePaths>;
	/** Uniform grid of node indices for hit-testing and culling. */
	grid: Map<string, number[]>;
}

const CELL = 1000;
const cellKey = (cx: number, cy: number) => `${cx},${cy}`;

/** Bezier from the right middle of `from` to the left middle of `to` (as xyflow drew them). */
function edgePath(sx: number, sy: number, tx: number, ty: number) {
	const offset = tx >= sx ? 0.5 * (tx - sx) : 0.25 * 25 * Math.sqrt(sx - tx);
	return `M${sx},${sy}C${sx + offset},${sy} ${tx - offset},${ty} ${tx},${ty}`;
}

export function buildScene(graph: TreeGraph): Scene {
	const w = graph.nodeWidth;
	const h = graph.nodeHeight;
	const nodes: SceneNode[] = graph.nodes.map((item) => ({
		item,
		key: item.key,
		x: item.x,
		y: item.y,
		color: CATEGORIES[item.category]?.color ?? '#888888',
		cost: formatCost(item.cost),
		kind: item.kind === 'tech' ? 'Tech' : 'Project',
		flags: activeFlags(item.flags),
		labels: new Map()
	}));
	const byKey = new Map(nodes.map((n) => [n.key, n]));

	let minX = Infinity,
		minY = Infinity,
		maxX = -Infinity,
		maxY = -Infinity;
	const grid = new Map<string, number[]>();
	nodes.forEach((n, i) => {
		minX = Math.min(minX, n.x);
		minY = Math.min(minY, n.y);
		maxX = Math.max(maxX, n.x + w);
		maxY = Math.max(maxY, n.y + h);
		for (let cx = Math.floor(n.x / CELL); cx <= Math.floor((n.x + w) / CELL); cx++) {
			for (let cy = Math.floor(n.y / CELL); cy <= Math.floor((n.y + h) / CELL); cy++) {
				const k = cellKey(cx, cy);
				const cell = grid.get(k);
				if (cell) cell.push(i);
				else grid.set(k, [i]);
			}
		}
	});
	const bounds = nodes.length
		? { x: minX, y: minY, width: maxX - minX, height: maxY - minY }
		: { x: 0, y: 0, width: 1, height: 1 };

	const edges: EdgePaths = { solid: new Path2D(), dashed: new Path2D() };
	const edgesOf = new Map<EntityKey, EdgePaths>();
	for (const e of graph.edges) {
		const from = byKey.get(e.from);
		const to = byKey.get(e.to);
		if (!from || !to) continue;
		const path = new Path2D(edgePath(from.x + w, from.y + h / 2, to.x, to.y + h / 2));
		const kind = e.alternative ? 'dashed' : 'solid';
		edges[kind].addPath(path);
		for (const key of [e.from, e.to]) {
			let p = edgesOf.get(key);
			if (!p) edgesOf.set(key, (p = { solid: new Path2D(), dashed: new Path2D() }));
			p[kind].addPath(path);
		}
	}

	return { graph, nodes, byKey, width: w, height: h, bounds, edges, edgesOf, grid };
}

/** Nodes intersecting a world rectangle (each once). */
export function nodesIn(scene: Scene, r: Rect): SceneNode[] {
	const out: SceneNode[] = [];
	const seen = new Set<number>();
	const x1 = Math.floor(r.x / CELL),
		x2 = Math.floor((r.x + r.width) / CELL);
	const y1 = Math.floor(r.y / CELL),
		y2 = Math.floor((r.y + r.height) / CELL);
	// Zoomed far out the grid walk costs more than a plain scan.
	if ((x2 - x1 + 1) * (y2 - y1 + 1) > scene.grid.size) {
		return scene.nodes.filter((n) => intersects(scene, n, r));
	}
	for (let cx = x1; cx <= x2; cx++) {
		for (let cy = y1; cy <= y2; cy++) {
			for (const i of scene.grid.get(cellKey(cx, cy)) ?? []) {
				if (seen.has(i)) continue;
				seen.add(i);
				if (intersects(scene, scene.nodes[i], r)) out.push(scene.nodes[i]);
			}
		}
	}
	return out;
}

function intersects(scene: Scene, n: SceneNode, r: Rect) {
	return (
		n.x < r.x + r.width &&
		n.x + scene.width > r.x &&
		n.y < r.y + r.height &&
		n.y + scene.height > r.y
	);
}

/** Node under a world point, if any. */
export function nodeAt(scene: Scene, wx: number, wy: number): SceneNode | null {
	const cell = scene.grid.get(cellKey(Math.floor(wx / CELL), Math.floor(wy / CELL)));
	for (const i of cell ?? []) {
		const n = scene.nodes[i];
		if (wx >= n.x && wx <= n.x + scene.width && wy >= n.y && wy <= n.y + scene.height) return n;
	}
	return null;
}
