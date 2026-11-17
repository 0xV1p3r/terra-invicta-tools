// Major bodies of the solar system map, shared by the focus picker and the hotbar (Svelte 5 runes).
import type { EntityKey } from '#lib/api.ts';
import { app } from '#lib/state.svelte.ts';
import { mapState } from './map-state.svelte';
import type { MapBody } from './scene';

/** Dwarf planets at least this large (km) are offered next to the planets. */
const MAJOR_DWARF_KM = 450;

const byOrbit = (list: MapBody[]) =>
	list
		.filter((b) => b.info.orbit)
		.sort((a, b) => a.info.orbit!.semiMajorAxisKm - b.info.orbit!.semiMajorAxisKm);

class MajorBodies {
	/** The Sun, then its planets from the inside out. */
	planets = $derived.by((): MapBody[] => {
		const root = mapState.index?.root;
		return root
			? [root, ...byOrbit(root.children.filter((b) => b.info.bodyType === 'planet'))]
			: [];
	});
	/** Large dwarf planets from the inside out. */
	dwarfs = $derived.by((): MapBody[] => {
		const root = mapState.index?.root;
		if (!root) return [];
		return byOrbit(
			root.children.filter(
				(b) => b.info.bodyType === 'dwarfPlanet' && b.info.radiusKm >= MAJOR_DWARF_KM
			)
		);
	});
	all = $derived([...this.planets, ...this.dwarfs]);

	/** What's in focus: the selected major body, else the system's center if it's one. */
	focused = $derived.by((): EntityKey | undefined => {
		const keys = new Set(this.all.map((b) => b.key));
		if (app.selected && keys.has(app.selected)) return app.selected;
		const center = app.mapSystem ?? mapState.index?.root?.key;
		return center && keys.has(center) ? center : undefined;
	});

	/** A body with moons opens its system; one without is centered in the Sun's. */
	focus(key: EntityKey) {
		const index = mapState.index;
		const body = index?.bodies.get(key);
		if (!body) return;
		app.selected = key;
		if (body === index?.root || body.children.some((c) => c.info.orbit)) {
			app.mapSystem = body === index?.root ? null : key;
			mapState.fitRequest++;
		} else app.mapFocus = key;
	}
}

export const majors = new MajorBodies();
