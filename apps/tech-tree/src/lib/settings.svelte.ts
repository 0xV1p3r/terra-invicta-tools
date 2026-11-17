// User preferences, persisted in localStorage (Svelte 5 runes).
import type { ImageFormat } from '#lib/api.ts';

export type FilterMode = 'hide' | 'highlight';

/** Radial distance scale of the solar system map. */
export type MapScale = 'log' | 'linear';

export const CORNERS = ['top-left', 'top-right', 'bottom-left', 'bottom-right'] as const;
export type Corner = (typeof CORNERS)[number];

export interface Settings {
	wikiSpoilers: boolean;
	/** Hide links implied by another chain of listed prerequisites (still shown in a lineage). */
	hideRedundantLinks: boolean;
	/** Long links run as shared lines (fan-out/fan-in trunks, buses between cards). */
	mergeLinks: boolean;
	/** Full-tree filters remove non-matching research, or only dim it (layout stays put). */
	filterMode: FilterMode;
	/** Zoomed far out: plain category blocks instead of cards. */
	overviewLabels: boolean;
	/** Zoomed out: screen-sized name labels on the most depended-on research. */
	landmarkLabels: boolean;
	/** Corner of the tree canvas the minimap sits in. */
	minimapCorner: Corner;
	/** Log scale keeps inner and outer orbits readable together; linear shows true proportions. */
	mapScale: MapScale;
	/** Map filters hide non-matching bodies, or only dim them. */
	mapFilterMode: FilterMode;
	/** Extraction: also the large artwork (illustrations, portraits, hab modules). */
	extractLargeArt: boolean;
	extractImageFormat: ImageFormat;
	/** Game folder chosen in onboarding ('' = detect: Steam or `TI_GAME_DIR`). */
	gameDir: string;
	/** Game build ids whose update notice was dismissed. */
	dismissedUpdates: string[];
	/** Look for a newer version of the app at startup. */
	checkAppUpdates: boolean;
	/** App versions whose update notice was dismissed. */
	dismissedAppUpdates: string[];
}

const DEFAULTS: Settings = {
	wikiSpoilers: false,
	hideRedundantLinks: true,
	mergeLinks: true,
	filterMode: 'hide',
	overviewLabels: true,
	landmarkLabels: false,
	minimapCorner: 'top-left',
	mapScale: 'log',
	mapFilterMode: 'highlight',
	extractLargeArt: true,
	extractImageFormat: 'webp',
	gameDir: '',
	dismissedUpdates: [],
	checkAppUpdates: true,
	dismissedAppUpdates: []
};

const STORAGE_KEY = 'ti-tech-tree:settings';

function load(): Settings {
	try {
		const raw = typeof localStorage === 'undefined' ? null : localStorage.getItem(STORAGE_KEY);
		const parsed = raw ? (JSON.parse(raw) as Partial<Settings>) : {};
		// Keep only known keys whose type matches the default.
		const out = { ...DEFAULTS };
		for (const key of Object.keys(DEFAULTS) as (keyof Settings)[]) {
			if (typeof parsed[key] === typeof DEFAULTS[key])
				(out as Record<string, unknown>)[key] = parsed[key];
		}
		if (out.filterMode !== 'hide' && out.filterMode !== 'highlight')
			out.filterMode = DEFAULTS.filterMode;
		if (out.mapScale !== 'log' && out.mapScale !== 'linear') out.mapScale = DEFAULTS.mapScale;
		if (out.mapFilterMode !== 'hide' && out.mapFilterMode !== 'highlight')
			out.mapFilterMode = DEFAULTS.mapFilterMode;
		if (!CORNERS.includes(out.minimapCorner)) out.minimapCorner = DEFAULTS.minimapCorner;
		if (out.extractImageFormat !== 'png' && out.extractImageFormat !== 'webp')
			out.extractImageFormat = DEFAULTS.extractImageFormat;
		out.dismissedUpdates = Array.isArray(out.dismissedUpdates)
			? out.dismissedUpdates.filter((id) => typeof id === 'string')
			: [];
		out.dismissedAppUpdates = Array.isArray(out.dismissedAppUpdates)
			? out.dismissedAppUpdates.filter((v) => typeof v === 'string')
			: [];
		return out;
	} catch {
		return { ...DEFAULTS, dismissedUpdates: [], dismissedAppUpdates: [] };
	}
}

export const settings: Settings = $state(load());

$effect.root(() => {
	$effect(() => {
		const json = JSON.stringify(settings);
		try {
			localStorage.setItem(STORAGE_KEY, json);
		} catch {
			// Storage unavailable (private mode, quota): settings last for the session.
		}
	});
});
