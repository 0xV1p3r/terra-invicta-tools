// Small display formatters shared by tree and detail views.
import type { EntityKind, UnlockKind } from '$lib/api';

/** Compact research cost: 950 → "950", 12500 → "12.5k", 2_000_000 → "2M". */
export function formatCost(cost: number | null | undefined): string {
	if (cost == null) return '–';
	const abs = Math.abs(cost);
	const compact = (n: number, suffix: string) =>
		`${n >= 100 ? Math.round(n) : Number(n.toFixed(1))}${suffix}`;
	if (abs >= 1e6) return compact(cost / 1e6, 'M');
	if (abs >= 1e3) return compact(cost / 1e3, 'k');
	return String(Math.round(cost));
}

/** Full cost with thousands separators, e.g. "12,500". */
export function formatCostFull(cost: number | null | undefined): string {
	return cost == null ? '–' : Math.round(cost).toLocaleString('en-US');
}

export const KIND_LABELS: Record<EntityKind, string> = {
	tech: 'Tech',
	project: 'Project',
	effect: 'Effect',
	habModule: 'Hab module',
	shipPart: 'Ship part',
	org: 'Org',
	trait: 'Trait',
	faction: 'Faction',
	objective: 'Objective',
	mission: 'Mission',
	codex: 'Codex',
	other: 'Other'
};

export const UNLOCK_LABELS: Record<UnlockKind, string> = {
	research: 'Research',
	researchAlternative: 'Research (alternative prerequisite)',
	shipPart: 'Ship parts',
	habModule: 'Hab modules',
	org: 'Orgs',
	grantedOrg: 'Granted orgs',
	claim: 'Claims',
	trait: 'Traits',
	codex: 'Codex entries',
	mission: 'Missions',
	objective: 'Objectives'
};

/** Techs and projects live in the tree; everything else only in the detail panel. */
export function isResearch(kind: EntityKind): boolean {
	return kind === 'tech' || kind === 'project';
}
