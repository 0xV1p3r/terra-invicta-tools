// Research planning state: saved campaigns, the active one and its plan (Svelte 5 runes).
// The backend computes statuses and plans (src-tauri/src/campaigns.rs); this keeps the
// campaign being edited, saves every change and refetches the plan.
import { untrack } from 'svelte';
import {
	api,
	type Campaign,
	type EntityKey,
	type PlanResult,
	type PlanStatus,
	type PlanStep
} from '#lib/api.ts';
import { app } from '#lib/state.svelte.ts';

const ACTIVE_KEY = 'ti-tech-tree:campaign';

function loadActive(): string | null {
	try {
		return typeof localStorage === 'undefined' ? null : localStorage.getItem(ACTIVE_KEY);
	} catch {
		return null;
	}
}

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

class CampaignState {
	/** Saved campaigns, most recently saved first (null until loaded). */
	list = $state<Campaign[] | null>(null);
	/** Id of the campaign being planned; null = none (the tree shows no statuses). */
	activeId = $state<string | null>(loadActive());
	/** Plan of the active campaign; stays on screen while a newer one loads. */
	plan = $state.raw<PlanResult | null>(null);
	loading = $state(false);
	error = $state<string | null>(null);

	/** Plan step position (0-based) by key, for badges on the tree. */
	stepIndex = $derived.by(() => {
		// Rebuilt whole on every plan, never mutated afterwards.
		// eslint-disable-next-line svelte/prefer-svelte-reactivity
		const out = new Map<EntityKey, number>();
		this.plan?.steps.forEach((s, i) => out.set(s.entity.key, i));
		return out;
	});

	/**
	 * The plan matches the campaign as it is now: shown plans stay up while a newer one loads, so
	 * their statuses may predate the latest edit (e.g. a requirement just un-marked).
	 */
	get planCurrent(): boolean {
		return this.plan !== null && !this.loading;
	}

	get active(): Campaign | null {
		return this.list?.find((c) => c.id === this.activeId) ?? null;
	}

	/** Status of a tech or project in the active campaign (undefined without one). */
	status(key: EntityKey): PlanStatus | undefined {
		return this.active ? this.plan?.status[key] : undefined;
	}

	step(key: EntityKey): PlanStep | undefined {
		const i = this.stepIndex.get(key);
		return i === undefined ? undefined : this.plan?.steps[i];
	}

	completions(key: EntityKey): number {
		return this.active?.completed.find((c) => c.key === key)?.count ?? 0;
	}

	isGoal(key: EntityKey): boolean {
		return this.active?.goals.includes(key) ?? false;
	}

	async load() {
		try {
			this.list = await api.listCampaigns();
			if (this.activeId && !this.active) this.activate(null);
		} catch (e) {
			this.list = [];
			this.error = message(e);
		}
	}

	/** Plan for `id` (null = stop planning). The plan page shows the campaign's scope. */
	activate(id: string | null) {
		this.activeId = id;
		this.plan = null;
		try {
			if (id) localStorage.setItem(ACTIVE_KEY, id);
			else localStorage.removeItem(ACTIVE_KEY);
		} catch {
			// Storage unavailable: the choice lasts for the session.
		}
	}

	async create(name: string, scenario: string, faction: string) {
		const saved = await this.write({
			id: '',
			name,
			scenario,
			faction,
			completed: [],
			goals: [],
			picks: [],
			save: null,
			updated: 0
		});
		if (saved) this.activate(saved.id);
		return saved;
	}

	/** Pick a save file (starting in the game's save folder) and import it into a new campaign. */
	async importSaveFile() {
		const dir = await api.defaultSaveDir().catch(() => null);
		const path = await this.call(() =>
			api.pickSaveFile('Import a Terra Invicta save', dir ?? undefined)
		);
		return path ? this.importSave(path) : null;
	}

	/** A new campaign from the game save at `path` (activated). */
	async importSave(path: string) {
		const imported = await this.call(() => api.importSave(app.scope.lang, path));
		const saved = imported && (await this.write(imported));
		if (saved) this.activate(saved.id);
		return saved;
	}

	/**
	 * Replace the active campaign's research state with the save at `path` (default: the save
	 * it was imported from); goals, choices and name stay.
	 */
	async updateFromSave(path = this.active?.save?.path) {
		const c = this.snapshot();
		if (!c || !path) return null;
		const imported = await this.call(() => api.importSave(app.scope.lang, path, c));
		return imported && this.write(imported);
	}

	async rename(id: string, name: string) {
		const c = this.list?.find((x) => x.id === id);
		if (c) await this.write({ ...$state.snapshot(c), name });
	}

	async remove(id: string) {
		try {
			await api.deleteCampaign(id);
			this.list = (this.list ?? []).filter((c) => c.id !== id);
			if (this.activeId === id) this.activate(null);
		} catch (e) {
			this.error = message(e);
		}
	}

	// ---- edits of the active campaign (each one is saved) ----

	/** Mark `key` completed; `upTo` also marks everything it still needs. */
	async markDone(key: EntityKey, upTo = false) {
		const c = this.snapshot();
		if (!c) return;
		const keys = upTo ? await this.call(() => api.planUpTo(c, key)) : [key];
		if (!keys) return;
		const have = new Set(c.completed.map((x) => x.key));
		c.completed.push(...keys.filter((k) => !have.has(k)).map((k) => ({ key: k, count: 1 })));
		await this.write(c);
	}

	/** Completed research that would lose a requirement if `key` were un-marked. */
	dependentsLost(key: EntityKey): Promise<EntityKey[] | null> {
		const c = this.snapshot();
		return c ? this.call(() => api.planDependentsLost(c, key)) : Promise.resolve([]);
	}

	/** Un-mark completed research (pass `key` plus `dependentsLost(key)` to keep the state consistent). */
	async unmark(keys: EntityKey[]) {
		const c = this.snapshot();
		if (!c) return;
		const drop = new Set(keys);
		c.completed = c.completed.filter((x) => !drop.has(x.key));
		await this.write(c);
	}

	/** Times a repeatable project or end-game tech was completed (0 un-marks it). */
	async setCompletions(key: EntityKey, count: number) {
		const c = this.snapshot();
		if (!c) return;
		c.completed = c.completed.filter((x) => x.key !== key);
		if (count > 0) c.completed.push({ key, count: Math.floor(count) });
		await this.write(c);
	}

	async addGoal(key: EntityKey) {
		const c = this.snapshot();
		if (!c || c.goals.includes(key)) return;
		c.goals.push(key);
		await this.write(c);
	}

	async removeGoal(key: EntityKey) {
		const c = this.snapshot();
		if (!c) return;
		c.goals = c.goals.filter((g) => g !== key);
		await this.write(c);
	}

	/** Move a goal to position `to` (goals are planned in order). */
	async moveGoal(key: EntityKey, to: number) {
		const c = this.snapshot();
		if (!c || !c.goals.includes(key)) return;
		c.goals = c.goals.filter((g) => g !== key);
		c.goals.splice(Math.max(0, Math.min(to, c.goals.length)), 0, key);
		await this.write(c);
	}

	/** Research `option` for "any of" group `group` of `node` (null = let the plan choose). */
	async pick(node: EntityKey, group: number, option: EntityKey | null) {
		const c = this.snapshot();
		if (!c) return;
		c.picks = c.picks.filter((p) => !(p.node === node && p.group === group));
		if (option) c.picks.push({ node, group, option });
		await this.write(c);
	}

	// ---- plumbing ----

	private snapshot(): Campaign | null {
		const c = this.active;
		return c ? $state.snapshot(c) : null;
	}

	private async call<T>(f: () => Promise<T>): Promise<T | null> {
		try {
			return await f();
		} catch (e) {
			this.error = message(e);
			return null;
		}
	}

	/** Saves run one after another, so a later edit never lands before an earlier one. */
	private queue: Promise<unknown> = Promise.resolve();

	private write(c: Campaign): Promise<Campaign | null> {
		// Show the edit at once; the plan follows when the backend answers.
		this.list = [c, ...(this.list ?? []).filter((x) => x.id !== c.id || !c.id)];
		const run = this.queue.then(async () => {
			const saved = await this.call(() => api.saveCampaign(c));
			if (saved) this.list = [saved, ...(this.list ?? []).filter((x) => x.id !== saved.id && x.id)];
			return saved;
		});
		this.queue = run;
		return run;
	}

	private planToken = 0;

	/** Refetch the plan of the active campaign (called by the effect below). */
	async refresh(c: Campaign | null, lang: string) {
		const token = ++this.planToken;
		if (!c || !c.id) {
			if (!c) this.plan = null;
			return;
		}
		this.loading = true;
		try {
			const plan = await api.getPlan(lang, c);
			if (token === this.planToken) {
				this.plan = plan;
				this.error = null;
			}
		} catch (e) {
			if (token === this.planToken) this.error = message(e);
		} finally {
			if (token === this.planToken) this.loading = false;
		}
	}
}

export const campaign = new CampaignState();

// The plan depends on the research state and language, not the name or save time.
let planInput = '';
$effect.root(() => {
	// The plan page shows the data in the active campaign's scenario and faction.
	$effect(() => {
		const c = campaign.active;
		const scenario = c?.scenario;
		const faction = c?.faction;
		const current = untrack(() => app.planScope);
		if (current?.scenario === scenario && current?.faction === faction) return;
		app.planScope = scenario && faction ? { scenario, faction } : null;
	});
	$effect(() => {
		const c = campaign.active;
		const snapshot = c ? $state.snapshot(c) : null;
		const lang = app.scope.lang;
		const input = JSON.stringify(
			snapshot && [
				snapshot.id,
				snapshot.scenario,
				snapshot.faction,
				snapshot.completed,
				snapshot.goals,
				snapshot.picks,
				lang
			]
		);
		if (input === planInput) return;
		planInput = input;
		void campaign.refresh(snapshot, lang);
	});
});
