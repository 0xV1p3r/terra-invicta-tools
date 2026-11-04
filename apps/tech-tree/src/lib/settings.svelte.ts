// User preferences, persisted in localStorage (Svelte 5 runes).

export type FilterMode = 'hide' | 'highlight';

export const CORNERS = ['top-left', 'top-right', 'bottom-left', 'bottom-right'] as const;
export type Corner = (typeof CORNERS)[number];

export interface Settings {
	/** Hide links implied by another chain of listed prerequisites (still shown in a lineage). */
	hideRedundantLinks: boolean;
	/** Full-tree filters remove non-matching research, or only dim it (layout stays put). */
	filterMode: FilterMode;
	/** Zoomed far out: plain category blocks with screen-sized landmark labels. */
	overviewLabels: boolean;
	/** Corner of the tree canvas the minimap sits in. */
	minimapCorner: Corner;
}

const DEFAULTS: Settings = {
	hideRedundantLinks: true,
	filterMode: 'hide',
	overviewLabels: false,
	minimapCorner: 'top-left'
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
		if (!CORNERS.includes(out.minimapCorner)) out.minimapCorner = DEFAULTS.minimapCorner;
		return out;
	} catch {
		return { ...DEFAULTS };
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
