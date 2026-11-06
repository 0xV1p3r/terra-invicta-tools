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

/** Top-level page: the research tree or the solar system map. */
export type Page = 'tree' | 'map';

/** Templates the solar system map shows (navigating to one opens the map). */
const MAP_TEMPLATES = ['TISpaceBodyTemplate:', 'TINavigableTemplate:'];
export const isMapKey = (key: EntityKey) => MAP_TEMPLATES.some((t) => key.startsWith(t));

/** A history step: the page, the view and the entity selected in it. */
interface HistoryEntry {
	page: Page;
	view: ViewSpec;
	selected: EntityKey | null;
	/** The selection was a node of the tree, so restoring it centers on it again. */
	inTree: boolean;
}

const sameView = (a: ViewSpec, b: ViewSpec) => JSON.stringify(a) === JSON.stringify(b);
const HISTORY_LIMIT = 100;

class AppState {
	/** Loaded once on startup; null until then (or on error, see `error`). */
	meta = $state<Meta | null>(null);
	error = $state<string | null>(null);

	page = $state<Page>('tree');

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
	/** Steps before/after the current one (most recent last): view changes and followed entity links. */
	back = $state<HistoryEntry[]>([]);
	forward = $state<HistoryEntry[]>([]);
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

	/** Show `next` on the tree page, recording where we came from (page included) in the history. */
	go(next: ViewSpec) {
		if (sameView(next, this.view) && this.page === 'tree') return;
		this.record();
		this.page = 'tree';
		this.setView(next);
	}

	/** Change options of the current view without a history step. */
	update(next: ViewSpec) {
		this.setView(next);
	}

	goBack() {
		const prev = this.back.at(-1);
		if (!prev) return;
		this.forward = [...this.forward, this.current()];
		this.back = this.back.slice(0, -1);
		this.restore(prev);
	}

	goForward() {
		const next = this.forward.at(-1);
		if (!next) return;
		this.back = [...this.back, this.current()];
		this.forward = this.forward.slice(0, -1);
		this.restore(next);
	}

	private current(): HistoryEntry {
		const selected = this.selected;
		return {
			page: this.page,
			view: $state.snapshot(this.view),
			selected,
			inTree: selected !== null && this.treeKeys.has(selected)
		};
	}

	/** Start a new history step from the current state. */
	private record() {
		this.back = [...this.back.slice(-HISTORY_LIMIT + 1), this.current()];
		this.forward = [];
	}

	private setView(next: ViewSpec) {
		if (next.kind === 'full') this.fullLayout = next.layout;
		if (next.kind === 'subtree') this.subtreeAlternatives = next.alternatives;
		this.view = next;
	}

	/**
	 * History navigation: back to the page, select the entity again and center on it (the tree on
	 * the view's root when it wasn't a node).
	 */
	private restore({ page, view, selected, inTree }: HistoryEntry) {
		this.page = page;
		this.setView(view);
		this.selected = selected;
		if (selected) this.panelOpen = true;
		this.focus = inTree ? selected : view.kind === 'full' ? null : view.root;
		if (selected && isMapKey(selected)) this.mapFocus = selected;
	}

	/** Entity shown in the detail panel. */
	selected = $state<EntityKey | null>(null);
	/** Node the tree view should center on next (e.g. after a search). */
	focus = $state<EntityKey | null>(null);

	/** Body whose system the map shows (its satellites); null = the Sun's. */
	mapSystem = $state<EntityKey | null>(null);
	/** Body or Lagrange point the map should show and center on next. */
	mapFocus = $state<EntityKey | null>(null);

	/** Keys of the nodes currently loaded in the tree view (set by the canvas). */
	treeKeys = $state.raw<ReadonlySet<EntityKey>>(new Set());
	/** Detail panel visibility. */
	panelOpen = $state(true);

	/**
	 * Follow an entity reference: show it in the panel; techs/projects are also focused in the
	 * tree, space bodies and Lagrange points on the map. A history step, so Back returns to the
	 * previous entity (selecting on the canvas or the map is not).
	 */
	navigate(ref: Pick<EntityRef, 'key' | 'kind'>) {
		const research = ref.kind === 'tech' || ref.kind === 'project';
		this.panelOpen = true;
		if (isMapKey(ref.key)) {
			if (ref.key !== this.selected || this.page !== 'map') this.record();
			this.page = 'map';
			this.selected = ref.key;
			this.mapFocus = ref.key;
			return;
		}
		// Record before switching pages, so Back returns to the page we came from.
		if (research && this.view.kind === 'focus') return this.openFocus(ref.key);
		if (research && this.view.kind !== 'full' && !this.treeKeys.has(ref.key)) this.showFullTree();
		else if (ref.key !== this.selected || (research && this.page !== 'tree')) this.record();
		if (research) this.page = 'tree';
		this.selected = ref.key;
		if (research) this.focus = ref.key;
	}

	openSubtree(root: EntityKey, direction: Direction = 'up') {
		this.page = 'tree';
		this.go({ kind: 'subtree', root, direction, alternatives: this.subtreeAlternatives });
		this.selected = root;
		this.focus = root;
	}

	openFocus(root: EntityKey) {
		this.page = 'tree';
		this.go({ kind: 'focus', root });
		this.selected = root;
		this.focus = root;
	}

	showFullTree(focus: EntityKey | null = null) {
		this.page = 'tree';
		this.go({ kind: 'full', layout: this.fullLayout });
		this.focus = focus;
	}
}

export const app = new AppState();
