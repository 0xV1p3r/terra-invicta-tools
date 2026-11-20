// Campaign marks for the tree: the active campaign's statuses, plan steps and goals as
// lookups for the renderer, built once per plan rather than looked up per frame.
// Glyph paths from Lucide (https://lucide.dev), ISC license.
import type { EntityKey, PlanResult, PlanStatus } from '#lib/api.ts';

export interface PlanMarks {
	status: Partial<Record<EntityKey, PlanStatus>>;
	done: ReadonlySet<EntityKey>;
	/** Prerequisites met (projects may still need an unlock roll). */
	available: ReadonlySet<EntityKey>;
	/** Plan step badge label ("1", "2", …) by key. */
	steps: ReadonlyMap<EntityKey, string>;
	goals: ReadonlySet<EntityKey>;
}

export function planMarks(plan: PlanResult, goals: readonly EntityKey[]): PlanMarks {
	const done = new Set<EntityKey>();
	const available = new Set<EntityKey>();
	for (const [key, status] of Object.entries(plan.status)) {
		if (status === 'done') done.add(key);
		else if (status === 'available') available.add(key);
	}
	return {
		status: plan.status,
		done,
		available,
		steps: new Map(plan.steps.map((s, i) => [s.entity.key, String(i + 1)])),
		goals: new Set(goals)
	};
}

/** Status of a node in words ("Researched ×3", "Can be researched now", "Plan step 4", "Goal"). */
export function statusLabels(
	status: PlanStatus | undefined,
	step: number | undefined,
	goal: boolean,
	completions: number
): string[] {
	const out: string[] = [];
	if (status === 'done') out.push(completions > 1 ? `Researched ×${completions}` : 'Researched');
	else if (status === 'available') out.push('Can be researched now');
	if (step !== undefined) out.push(`Plan step ${step + 1}`);
	if (goal) out.push('Goal');
	return out;
}

/** Canvas glyphs (24×24 viewBox, stroked): Lucide `check` and `target`. */
export const CHECK_GLYPH = 'M20 6 9 17l-5-5';
export const GOAL_GLYPH =
	'M2 12a10 10 0 1 0 20 0a10 10 0 1 0-20 0M6 12a6 6 0 1 0 12 0a6 6 0 1 0-12 0M10 12a2 2 0 1 0 4 0a2 2 0 1 0-4 0';
