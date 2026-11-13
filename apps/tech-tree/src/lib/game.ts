// Game install labels and the update check.
import type { BuildInfo, GameInfo } from '$lib/api';

/** "Terra Invicta 1.0.53b (build 21474836)"; the build only for Steam installs. */
export function gameLabel(game: GameInfo): string {
	const version = game.version ? ` ${game.version}` : '';
	const build = game.buildId ? ` (build ${game.buildId})` : '';
	return `Terra Invicta${version}${build}`;
}

/**
 * The update the detected game is at, if its build is neither extracted nor dismissed: `from` is
 * the latest extracted build (null if unknown). Installs without a build id (not Steam) can't tell.
 */
export function gameUpdate(
	game: GameInfo | null,
	builds: readonly BuildInfo[],
	dismissed: readonly string[]
): { from: string | null; to: string } | null {
	const to = game?.buildId;
	if (!to || builds.length === 0) return null;
	if (builds.some((b) => b.buildId === to) || dismissed.includes(to)) return null;
	return { from: (builds.find((b) => b.latest) ?? builds.at(-1))?.buildId ?? null, to };
}

/** Bytes as a short size: 873_000_000 → "873 MB", 1_746_000_000 → "1.7 GB". */
export function formatBytes(bytes: number): string {
	const units = ['B', 'kB', 'MB', 'GB', 'TB'];
	let value = bytes;
	let unit = 0;
	while (value >= 1000 && unit < units.length - 1) {
		value /= 1000;
		unit += 1;
	}
	const digits = unit > 0 && value < 10 ? 1 : 0;
	return `${value.toFixed(digits)} ${units[unit]}`;
}
