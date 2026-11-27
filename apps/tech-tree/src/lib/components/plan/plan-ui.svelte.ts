// UI state of the research planner shared by the plan toolbar, the plan panel and the
// detail panel (Svelte 5 runes). Campaign data itself lives in `#lib/campaign.svelte.ts`.
import type { EntityKey, EntityKind } from '#lib/api.ts';

const PANEL_KEY = 'ti-tech-tree:plan-panel';

function loadPanel(): boolean {
	try {
		return typeof localStorage === 'undefined' || localStorage.getItem(PANEL_KEY) !== '0';
	} catch {
		return true;
	}
}

class PlanUi {
	/** Plan panel on the plan page (left; the detail panel stays on the right). Open by default. */
	panelOpen = $state(loadPanel());
	/** "New campaign" dialog (opened from the switcher and the plan panel's empty state). */
	creating = $state(false);

	togglePanel(open = !this.panelOpen) {
		this.panelOpen = open;
		try {
			localStorage.setItem(PANEL_KEY, open ? '1' : '0');
		} catch {
			// Storage unavailable: the choice lasts for the session.
		}
	}
}

export const planUi = new PlanUi();

/** Kind of a research key, for navigation when only the key is known (e.g. a finished goal). */
export function researchKind(key: EntityKey): EntityKind {
	return key.startsWith('TIProjectTemplate:') ? 'project' : 'tech';
}
