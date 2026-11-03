// App-wide UI state shared by all components (Svelte 5 runes).
import type { Direction, EntityKey, EntityRef, Meta, ScopeArgs, TechCategory } from '$lib/api';

export type ViewMode = 'full' | 'subtree';

class AppState {
	/** Loaded once on startup; null until then (or on error, see `error`). */
	meta = $state<Meta | null>(null);
	error = $state<string | null>(null);

	/** Data scope: overview by default (all scenarios, no faction). */
	scope = $state<ScopeArgs>({ scenario: null, faction: null, lang: 'en' });

	/** Full tree filters. */
	showTechs = $state(true);
	showProjects = $state(true);
	categories = $state<TechCategory[]>([]);
	/** Property ids (`PropertyInfo.id`); empty = no property filter. */
	properties = $state<string[]>([]);

	get filtersActive() {
		return (
			!this.showTechs ||
			!this.showProjects ||
			this.categories.length > 0 ||
			this.properties.length > 0
		);
	}

	clearFilters() {
		this.showTechs = true;
		this.showProjects = true;
		this.categories = [];
		this.properties = [];
	}

	/** Full tree, or the sub-tree around `subtreeRoot`. */
	mode = $state<ViewMode>('full');
	subtreeRoot = $state<EntityKey | null>(null);
	subtreeDirection = $state<Direction>('up');
	subtreeAlternatives = $state(true);

	/** Entity shown in the detail panel. */
	selected = $state<EntityKey | null>(null);
	/** Node the tree view should center on next (e.g. after a search). */
	focus = $state<EntityKey | null>(null);

	/** Keys of the nodes currently loaded in the tree view (set by the canvas). */
	treeKeys = $state.raw<ReadonlySet<EntityKey>>(new Set());
	/** Detail panel visibility. */
	panelOpen = $state(true);

	/** Follow an entity reference: show it in the panel; techs/projects are also focused in the tree. */
	navigate(ref: Pick<EntityRef, 'key' | 'kind'>) {
		this.selected = ref.key;
		this.panelOpen = true;
		if (ref.kind === 'tech' || ref.kind === 'project') {
			if (this.mode === 'subtree' && !this.treeKeys.has(ref.key)) this.mode = 'full';
			this.focus = ref.key;
		}
	}

	openSubtree(root: EntityKey, direction: Direction = 'up') {
		this.subtreeRoot = root;
		this.subtreeDirection = direction;
		this.mode = 'subtree';
		this.selected = root;
		this.focus = root;
	}

	showFullTree(focus: EntityKey | null = null) {
		this.mode = 'full';
		this.focus = focus;
	}
}

export const app = new AppState();
