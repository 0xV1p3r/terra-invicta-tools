import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { TreeGraph, TreeNode } from '$lib/api';
import { buildScene, nodeAt, nodesIn } from './scene';

// Flag icons are Svelte components; their presentation does not affect geometry.
vi.mock('./flags', () => ({ activeFlags: () => [] }));

// Hit-testing needs no canvas. Empty-edge fixtures only construct paths.
beforeEach(() => vi.stubGlobal('Path2D', class {}));
afterEach(() => vi.unstubAllGlobals());

function scene() {
	const node = (key: string, x: number, y: number): TreeNode => ({
		key,
		x,
		y,
		dataName: key,
		kind: 'tech',
		name: key,
		category: 'Energy',
		cost: 0,
		column: 0,
		properties: [],
		flags: {
			scenarioSpecific: false,
			variesByScenario: false,
			dlc: false,
			factionLocked: false,
			endGame: false,
			repeatable: false,
			obtainable: true
		}
	});
	const graph: TreeGraph = {
		nodes: [node('crossing', 950, 950), node('negative', -100, -100)],
		edges: [],
		bands: [],
		anyOf: [],
		nodeWidth: 200,
		nodeHeight: 100,
		root: null,
		totalCost: 0
	};
	return buildScene(graph, { hideRedundant: false });
}

it('finds a card across grid boundaries, including its edges', () => {
	const s = scene();
	for (const [x, y] of [
		[950, 950],
		[1000, 1000],
		[1150, 1050]
	]) {
		expect(nodeAt(s, x, y)?.key).toBe('crossing');
	}
	expect(nodeAt(s, 1151, 1050)).toBeNull();
	expect(nodeAt(s, 1000, 1100)).toBeNull();
	expect(nodeAt(s, -50, -50)?.key).toBe('negative');
	expect(nodeAt(s, 5000, 5000)).toBeNull();
});

it('returns intersecting cards once, even across several cells', () => {
	const s = scene();
	expect(nodesIn(s, { x: 940, y: 940, width: 220, height: 120 }).map((n) => n.key)).toEqual([
		'crossing'
	]);
	expect(nodesIn(s, { x: -10000, y: -10000, width: 20000, height: 20000 })).toHaveLength(2);
	expect(nodesIn(s, { x: 1150, y: 950, width: 10, height: 10 })).toEqual([]);
});
