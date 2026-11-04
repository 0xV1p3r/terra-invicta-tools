// App-wide UI state shared by all components (Svelte 5 runes).
import type {
	Direction,
	EntityKey,
	EntityRef,
	FullLayout,
	Meta,
	ScopeArgs,
	TechCategory,
	TreeFilter
} from '$lib/api';

export type { FullLayout };

/** What the tree view shows. Changing it (except for option tweaks) is a history step. */
export type ViewSpec =
	| { kind: 'full'; layout: FullLayout }
	| { kind: 'subtree'; root: EntityKey; direction: Direction; alternatives: boolean }
	/** One tech with its direct requirements and dependents; clicking a card re-roots. */
	| { kind: 'focus'; root: EntityKey };

export type ViewKind = ViewSpec['kind'];

const sameView = (a: ViewSpec, b: ViewSpec) => JSON.stringify(a) === JSON.stringify(b);
const HISTORY_LIMIT = 100;

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

	/** The full-tree filters as sent to the backend. */
	get filter(): TreeFilter {
		return {
			techs: this.showTechs,
			projects: this.showProjects,
			categories: $state.snapshot(this.categories),
			properties: $state.snapshot(this.properties)
		};
	}

	clearFilters() {
		this.showTechs = true;
		this.showProjects = true;
		this.categories = [];
		this.properties = [];
	}

	/** Current view; change it through `go`/`update` so history stays consistent. */
	view = $state<ViewSpec>({ kind: 'full', layout: 'layered' });
	/** Views before/after the current one (most recent last). */
	back = $state<ViewSpec[]>([]);
	forward = $state<ViewSpec[]>([]);
	/** Remembered so leaving a sub-tree returns to the same full-tree layout. */
	private fullLayout: FullLayout = 'layered';
	private subtreeAlternatives = true;

	get mode(): ViewKind {
		return this.view.kind;
	}

	/** Root of the sub-tree/focus view, null in the full tree. */
	get root(): EntityKey | null {
		return this.view.kind === 'full' ? null : this.view.root;
	}

	/** Show `next`, recording the current view in the history. */
	go(next: ViewSpec) {
		if (sameView(next, this.view)) return;
		this.back = [...this.back.slice(-HISTORY_LIMIT + 1), $state.snapshot(this.view)];
		this.forward = [];
		this.setView(next);
	}

	/** Change options of the current view without a history step. */
	update(next: ViewSpec) {
		this.setView(next);
	}

	goBack() {
		const prev = this.back.at(-1);
		if (!prev) return;
		this.forward = [...this.forward, $state.snapshot(this.view)];
		this.back = this.back.slice(0, -1);
		this.restore(prev);
	}

	goForward() {
		const next = this.forward.at(-1);
		if (!next) return;
		this.back = [...this.back, $state.snapshot(this.view)];
		this.forward = this.forward.slice(0, -1);
		this.restore(next);
	}

	private setView(next: ViewSpec) {
		if (next.kind === 'full') this.fullLayout = next.layout;
		if (next.kind === 'subtree') this.subtreeAlternatives = next.alternatives;
		this.view = next;
	}

	/** History navigation: also select the root again, like when the view was opened. */
	private restore(spec: ViewSpec) {
		this.setView(spec);
		if (spec.kind !== 'full') {
			this.selected = spec.root;
			this.focus = spec.root;
		}
	}

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
			if (this.view.kind === 'focus') return this.openFocus(ref.key);
			if (this.view.kind !== 'full' && !this.treeKeys.has(ref.key)) this.showFullTree();
			this.focus = ref.key;
		}
	}

	openSubtree(root: EntityKey, direction: Direction = 'up') {
		this.go({ kind: 'subtree', root, direction, alternatives: this.subtreeAlternatives });
		this.selected = root;
		this.focus = root;
	}

	openFocus(root: EntityKey) {
		this.go({ kind: 'focus', root });
		this.selected = root;
		this.focus = root;
	}

	showFullTree(focus: EntityKey | null = null) {
		this.go({ kind: 'full', layout: this.fullLayout });
		this.focus = focus;
	}
}

export const app = new AppState();
