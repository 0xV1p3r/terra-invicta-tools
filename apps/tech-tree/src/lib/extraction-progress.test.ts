import { describe, expect, it } from 'vitest';
import { advanceProgress, initialProgress } from './extraction-progress';

describe('overall extraction progress', () => {
	it('weights stages by time, including resumed snapshots', () => {
		for (const [stage, start, end] of [
			['templates', 0, 4],
			['locale', 4, 8],
			['schema', 8, 10],
			['images', 10, 98],
			['finish', 98, 100]
		] as const) {
			const progress = advanceProgress(initialProgress(), stage);
			expect(progress.percent).toBeCloseTo(start);
			expect(advanceProgress(progress, stage, 1, 1).percent).toBe(end);
		}
	});

	it('holds fraction and counts across reordered workers and a second Images series', () => {
		let progress = advanceProgress(initialProgress(), 'images', 900, 1000);
		for (const [done, total] of [
			[800, 1000],
			[0, 100],
			[50, 100],
			[100, 100]
		]) {
			const next = advanceProgress(progress, 'images', done, total);
			expect(next.percent).toBeGreaterThanOrEqual(progress.percent);
			expect(next.done).toBeGreaterThanOrEqual(progress.done);
			expect(next.total).toBeGreaterThanOrEqual(progress.total);
			progress = next;
		}
		expect(progress.percent).toBe(98);
		expect(advanceProgress(progress, 'finish', 1, 1).percent).toBe(100);
	});

	it('renormalizes when schema jumps to finish without images', () => {
		const schema = advanceProgress(initialProgress(), 'schema', 1, 1);
		const progress = advanceProgress(schema, 'finish');
		expect(progress.imagesSkipped).toBe(true);
		expect(progress.percent).toBeCloseTo((100 * 10) / 12);
		expect(progress.done).toBe(0);
		expect(advanceProgress(progress, 'finish', 1, 1).percent).toBe(100);
	});

	it('clamps counts and ignores stale stages, resetting only for a new run', () => {
		const progress = advanceProgress(initialProgress(), 'images', 200, 100);
		expect(progress.percent).toBe(98);
		expect(advanceProgress(progress, 'schema', 0, 0)).toEqual(progress);
		expect(advanceProgress(progress, 'images', -1, 0).percent).toBe(98);
		expect(advanceProgress(progress, 'finish', 200, 1).percent).toBe(100);
		expect(initialProgress().percent).toBe(0);
	});

	it('accepts the fake onboarding stage and step sequence', () => {
		let progress = initialProgress();
		for (const stage of ['templates', 'locale', 'schema', 'images', 'finish'] as const) {
			progress = advanceProgress(progress, stage);
			for (let done = 1; done <= 20; done++) {
				const next = advanceProgress(progress, stage, done, 20);
				expect(next.percent).toBeGreaterThanOrEqual(progress.percent);
				expect(next.percent).toBeLessThanOrEqual(100);
				progress = next;
			}
		}
		expect(progress.percent).toBe(100);
	});
});
