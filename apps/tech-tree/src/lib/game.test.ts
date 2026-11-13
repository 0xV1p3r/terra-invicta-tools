import { expect, it } from 'vitest';
import type { BuildInfo, GameInfo } from './api';
import { formatBytes, gameLabel, gameUpdate } from './game';

const game = (buildId: string | null): GameInfo => ({
	root: '/games/Terra Invicta',
	version: '1.0.53b',
	buildId
});
const build = (buildId: string, latest = false): BuildInfo => ({
	dir: buildId,
	buildId,
	lastUpdated: null,
	gameVersion: null,
	loaded: latest,
	latest
});

it('labels installs with and without a Steam build', () => {
	expect(gameLabel(game('21474836'))).toBe('Terra Invicta 1.0.53b (build 21474836)');
	expect(gameLabel({ ...game(null), version: null })).toBe('Terra Invicta');
});

it('reports a game build that is neither extracted nor dismissed', () => {
	const builds = [build('100'), build('200', true)];
	expect(gameUpdate(game('300'), builds, [])).toEqual({ from: '200', to: '300' });
	// An older extracted build counts too: the game was rolled back to it.
	expect(gameUpdate(game('100'), builds, [])).toBeNull();
	expect(gameUpdate(game('300'), builds, ['300'])).toBeNull();
	expect(gameUpdate(game('300'), builds, ['400'])).not.toBeNull();
	// Not a Steam install, no install, or nothing extracted yet: nothing to compare.
	expect(gameUpdate(game(null), builds, [])).toBeNull();
	expect(gameUpdate(null, builds, [])).toBeNull();
	expect(gameUpdate(game('300'), [], [])).toBeNull();
});

it.each([
	[0, '0 B'],
	[999, '999 B'],
	[1500, '1.5 kB'],
	[873_000_000, '873 MB'],
	[1_746_000_000, '1.7 GB'],
	[25_000_000_000, '25 GB']
])('formatBytes(%s) = %s', (bytes, text) => {
	expect(formatBytes(bytes)).toBe(text);
});
