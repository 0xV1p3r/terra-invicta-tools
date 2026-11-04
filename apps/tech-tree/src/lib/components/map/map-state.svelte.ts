// UI state of the solar system map shared by its canvas and header controls (Svelte 5 runes).
import type { ResourceRating, SpaceBodyInfo, SpaceResource } from '$lib/api';
import type { ItemType } from './scene';

export const ITEM_TYPES: ItemType[] = [
	'planet',
	'dwarfPlanet',
	'planetaryMoon',
	'asteroidalMoon',
	'asteroid',
	'comet',
	'lagrange'
];

export const RESOURCES: SpaceResource[] = ['water', 'volatiles', 'metals', 'nobles', 'fissiles'];
export const RATINGS: ResourceRating[] = ['empty', 'possible', 'low', 'medium', 'high', 'max'];
const rank = (r: ResourceRating | undefined) => (r ? RATINGS.indexOf(r) : -1);

export const DAY_MS = 86_400_000;

class MapState {
	/** Map date as a UTC timestamp (ms); NaN until the data is loaded. */
	time = $state(NaN);
	/** Scenario start date (ms), the slider's origin. */
	start = $state(NaN);
	playing = $state(false);
	/** Simulated days per real second while playing. */
	speed = $state(30);

	/** Types shown (the system's center always is). */
	types = $state<ItemType[]>([...ITEM_TYPES]);
	habSitesOnly = $state(false);
	/** Minimum expected rating per resource. */
	minRatings = $state<Partial<Record<SpaceResource, ResourceRating>>>({});

	get filtersActive() {
		return (
			this.types.length < ITEM_TYPES.length ||
			this.habSitesOnly ||
			Object.keys(this.minRatings).length > 0
		);
	}

	/** Whether a body or Lagrange point (`info` null) passes the filters. */
	matches(type: ItemType, info: SpaceBodyInfo | null): boolean {
		if (!this.types.includes(type)) return false;
		const ratings = Object.entries(this.minRatings) as [SpaceResource, ResourceRating][];
		if (!info) return !this.habSitesOnly && ratings.length === 0;
		if (this.habSitesOnly && info.habSiteCount === 0) return false;
		return ratings.every(([res, min]) => rank(info.resources[res]) >= rank(min));
	}

	clearFilters() {
		this.types = [...ITEM_TYPES];
		this.habSitesOnly = false;
		this.minRatings = {};
	}
}

export const mapState = new MapState();
