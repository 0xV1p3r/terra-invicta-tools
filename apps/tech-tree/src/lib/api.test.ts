import { expect, it } from 'vitest';
import { matchesFilter, type TreeFilter, type TreeNode } from './api';

// Import the real module without installing desktop runtime or browser globals.
const node: TreeNode = {
	key: 'TITechTemplate:Example',
	dataName: 'Example',
	name: 'Example',
	kind: 'tech',
	category: 'Energy',
	cost: 100,
	column: 0,
	x: 0,
	y: 0,
	properties: ['repeatable'],
	flags: {
		scenarioSpecific: false,
		variesByScenario: false,
		dlc: false,
		factionLocked: false,
		endGame: false,
		repeatable: true,
		obtainable: true
	}
};
const all: TreeFilter = { techs: true, projects: true, categories: [], properties: [] };

it('treats empty category and property filters as unrestricted', () => {
	expect(matchesFilter(node, all)).toBe(true);
	expect(matchesFilter({ ...node, properties: [] }, all)).toBe(true);
});
it('applies tech and project switches independently', () => {
	expect(matchesFilter(node, { ...all, techs: false })).toBe(false);
	expect(matchesFilter({ ...node, kind: 'project' }, { ...all, techs: false })).toBe(true);
	expect(matchesFilter({ ...node, kind: 'project' }, { ...all, projects: false })).toBe(false);
});
it('requires a matching category and at least one selected property', () => {
	const filter: TreeFilter = { ...all, categories: ['Energy'], properties: ['dlc', 'repeatable'] };
	expect(matchesFilter(node, filter)).toBe(true);
	expect(matchesFilter({ ...node, category: 'Materials' }, filter)).toBe(false);
	expect(matchesFilter({ ...node, properties: [] }, filter)).toBe(false);
});
