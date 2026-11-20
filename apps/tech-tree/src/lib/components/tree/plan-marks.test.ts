import { expect, it } from 'vitest';
import type { PlanResult, PlanStep } from '#lib/api.ts';
import { planMarks, statusLabels } from './plan-marks';

it('collects statuses, step numbers and goals', () => {
	const step = (key: string) => ({ entity: { key } }) as PlanStep;
	const plan = {
		status: { 'a:1': 'done', 'a:2': 'available', 'a:3': 'locked', 'a:4': 'unobtainable' },
		steps: [step('a:2'), step('a:3')]
	} as unknown as PlanResult;
	const marks = planMarks(plan, ['a:3']);
	expect([...marks.done]).toEqual(['a:1']);
	expect([...marks.available]).toEqual(['a:2']);
	expect([...marks.steps]).toEqual([
		['a:2', '1'],
		['a:3', '2']
	]);
	expect(marks.goals.has('a:3')).toBe(true);
});

it('describes a status in words', () => {
	expect(statusLabels('done', undefined, false, 1)).toEqual(['Researched']);
	expect(statusLabels('done', undefined, true, 3)).toEqual(['Researched ×3', 'Goal']);
	expect(statusLabels('available', 0, false, 0)).toEqual(['Can be researched now', 'Plan step 1']);
	expect(statusLabels('locked', 3, true, 0)).toEqual(['Plan step 4', 'Goal']);
	expect(statusLabels(undefined, undefined, false, 0)).toEqual([]);
});
