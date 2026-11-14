import { expect, it } from 'vitest';
import {
	formatCost,
	formatCostFull,
	readableKey,
	requirementChoices,
	templateLabel
} from './format';

it.each([
	[null, '–'],
	[undefined, '–'],
	[0, '0'],
	[949.6, '950'],
	[999, '999'],
	[1000, '1k'],
	[12500, '12.5k'],
	[100000, '100k'],
	[999999, '1000k'],
	[1000000, '1M'],
	[2000000, '2M'],
	[-12500, '-12.5k']
])('formats cost %s as %s', (cost, label) => expect(formatCost(cost)).toBe(label));

it('formats full costs with English grouping and rounds fractions', () => {
	expect(formatCostFull(12500.6)).toBe('12,501');
	expect(formatCostFull(-12500)).toBe('-12,500');
	expect(formatCostFull(undefined)).toBe('–');
});
it('counts unresolved requirements as choices', () => {
	expect(
		requirementChoices({ options: [], hidden: [], unresolved: ['Missing'], alternative: [] })
	).toBe(1);
	expect(requirementChoices({ options: [], hidden: [], unresolved: [], alternative: [] })).toBe(0);
});
it('makes fallback keys readable', () => {
	expect(readableKey('TITechTemplate:AdAstra')).toBe('Ad Astra');
	expect(readableKey('TIProjectTemplate:Project_FusionRockets')).toBe('Fusion Rockets');
	expect(readableKey('NoPrefix')).toBe('No Prefix');
});
it('prefers player labels, then section names, then a plural fallback', () => {
	expect(templateLabel('TIStartTimeTemplate', 'Ignored')).toBe('Scenarios');
	expect(templateLabel('TIFooTemplate', 'Custom')).toBe('Custom');
	expect(templateLabel('TIFooBarTemplate')).toBe('Foo Bars');
	expect(templateLabel('TIPlayersTemplate')).toBe('Players');
});
