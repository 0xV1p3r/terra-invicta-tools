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
} from '#lib/api.ts';

export type { FullLayout };

/** What the tree view shows. Changing it (except for option tweaks) is a history step. */
export type ViewSpec =
	| { kind: 'full'; layout: FullLayout }
	| { kind: 'subtree'; root: EntityKey; direction: Direction; alternatives: boolean }
	/** One tech with its direct requirements and dependents; clicking a card re-roots. */
	| { kind: 'focus'; root: EntityKey };

export type ViewKind = ViewSpec['kind'];

/** What the tree canvas shows: the tree page's view, or the plan page's path. */
export type CanvasView = ViewSpec | { kind: 'path' };

/** Top-level page: the research tree, the research plan or the solar system map. */
export type Page = 'tree' | 'plan' | 'map';

/** The plan page's canvas: only the research the plan still needs, or the whole tree. */
export type PlanView = 'path' | 'full';

/** A campaign's scenario and faction, which the plan page shows the data in. */
export interface PlanScope {
	scenario: string;
	faction: string;
}

/** Templates the solar system map shows (navigating to one opens the map). */
const MAP_TEMPLATES = ['TISpaceBodyTemplate:', 'TINavigableTemplate:'];
export const isMapKey = (key: EntityKey) => MAP_TEMPLATES.some((t) => key.startsWith(t));

/** A history step: the page, the view and the entity selected in it. */
interface HistoryEntry {
	page: Page;
	view: ViewSpec;
	planView: PlanView;
	selected: EntityKey | null;
	/** The selection was a node of the tree, so restoring it centers on it again. */
	inTree: boolean;
}

const sameView = (a: ViewSpec, b: ViewSpec) => JSON.stringify(a) === JSON.stringify(b);
const HISTORY_LIMIT = 100;

const STORAGE_KEY = 'ti-tech-tree:scope';
const PLAN_VIEW_KEY = 'ti-tech-tree:plan-view';

function loadPlanView(): PlanView {
	try {
		return typeof localStorage !== 'undefined' && localStorage.getItem(PLAN_VIEW_KEY) === 'full'
			? 'full'
			: 'path';
	} catch {
		return 'path';
	}
}

function loadScope(): ScopeArgs {
	const defaults: ScopeArgs = { scenario: null, faction: null, lang: 'en' };
	try {
		const raw = typeof localStorage === 'undefined' ? null : localStorage.getItem(STORAGE_KEY);
		const parsed: unknown = raw ? JSON.parse(raw) : null;
		if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return defaults;
		const scope = parsed as Record<string, unknown>;
		return {
			scenario: typeof scope.scenario === 'string' ? scope.scenario : null,
			faction: typeof scope.faction === 'string' ? scope.faction : null,
			lang: typeof scope.lang === 'string' ? scope.lang : 'en'
		};
	} catch {
		return defaults;
	}
}

class AppState {
	/** Loaded once on startup; null until then (or on error, see `error`). */
	private metadata = $state<Meta | null>(null);

	get meta() {
		return this.metadata;
	}

	set meta(meta: Meta | null) {
		if (meta) {
			if (!meta.scenarios.some((scenario) => scenario.name === this.scope.scenario))
				this.scope.scenario = null;
			if (!meta.factions.some((faction) => faction.name === this.scope.faction))
				this.scope.faction = null;
			if (!meta.languages.includes(this.scope.lang)) this.scope.lang = 'en';
		}
		this.metadata = meta;
	}
	error = $state<string | null>(null);

	page = $state<Page>('tree');

	/** Data scope of the tree and map pages: overview by default (all scenarios, no faction). */
	scope = $state<ScopeArgs>(loadScope());

	/** The active campaign's scenario and faction (set by the campaign store); null without one. */
	planScope = $state.raw<PlanScope | null>(null);
	/** The plan page's canvas (remembered). */
	planView = $state<PlanView>(loadPlanView());

	/** The plan page with a campaign to show. */
	get planning(): boolean {
		return this.page === 'plan' && this.planScope !== null;
	}

	/**
	 * The scope the current page shows its data in: the campaign's on the plan page, else the
	 * scope bar's. The language is always the one from the settings.
	 */
	get dataScope(): ScopeArgs {
		const plan = this.planning ? this.planScope : null;
		return plan ? { ...plan, lang: this.scope.lang } : this.scope;
	}

	/** What the tree canvas shows on the current page. */
	get canvasView(): CanvasView {
		if (this.page !== 'plan') return this.view;
		return this.planView === 'path' ? { kind: 'path' } : { kind: 'full', layout: this.fullLayout };
	}

	/** Switch the plan page's canvas (not a history step). */
	setPlanView(view: PlanView) {
		this.planView = view;
		try {
			localStorage.setItem(PLAN_VIEW_KEY, view);
		} catch {
			// Storage unavailable: the choice lasts for the session.
		}
	}

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

	/** Root of the canvas's sub-tree/focus view, null in the full tree and on the plan page. */
	get root(): EntityKey | null {
		const view = this.canvasView;
		return view.kind === 'subtree' || view.kind === 'focus' ? view.root : null;
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
			planView: this.planView,
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
	private restore({ page, view, planView, selected, inTree }: HistoryEntry) {
		this.page = page;
		this.setView(view);
		if (page === 'plan') this.setPlanView(planView);
		this.selected = selected;
		if (selected) this.panelOpen = true;
		const shown = this.canvasView;
		this.focus = inTree ? selected : 'root' in shown ? shown.root : null;
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
		// The plan page keeps research on its own canvas: the whole tree when it isn't on the path.
		if (research && this.planning) {
			if (ref.key !== this.selected) this.record();
			if (this.planView === 'path' && !this.treeKeys.has(ref.key)) this.setPlanView('full');
			this.selected = ref.key;
			this.focus = ref.key;
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

$effect.root(() => {
	$effect(() => {
		const json = JSON.stringify(app.scope);
		try {
			localStorage.setItem(STORAGE_KEY, json);
		} catch {
			// Storage unavailable (private mode, quota): scope lasts for the session.
		}
	});
});
