// Small display formatters shared by tree and detail views.
import type { EntityKey, EntityKind, RequirementGroup, UnlockKind } from '$lib/api';

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

/**
 * Requirement groups are all required; any one entry of a group satisfies it (prereqs give
 * single-entry groups, altPrereqN adds alternatives).
 */
export const REQUIREMENT_LABELS = { all: 'All required', any: 'Any one of' };

/** Entries in a requirement group, including ones this scope can't complete. */
export function requirementChoices(group: RequirementGroup): number {
	return group.options.length + group.hidden.length + group.unresolved.length;
}

/** "FusionRockets" → "Fusion Rockets". */
function humanize(name: string): string {
	return name.replace(/_/g, ' ').replace(/([a-z0-9])([A-Z])/g, '$1 $2');
}

/** Readable stand-in for an entity name when only its key is known: "TITechTemplate:AdAstra" → "Ad Astra". */
export function readableKey(key: EntityKey): string {
	return humanize(key.slice(key.indexOf(':') + 1).replace(/^Project_/, ''));
}

/** Player-facing names for templates the wiki names technically or has no section for. */
const TEMPLATE_LABELS: Record<string, string> = {
	TINarrativeEventTemplate: 'Story Events',
	TITimeEventTemplate: 'Timed Events',
	TIStartTimeTemplate: 'Scenarios',
	TIMetaTemplate: 'Game Rules',
	TIGlobalConfig: 'Game Rules',
	TIPlayerTemplate: 'Players',
	TIBilateralTemplate: 'Bilateral Relations',
	TIPriorityPresetTemplate: 'Priority Presets',
	TICouncilorAppearanceTemplate: 'Councilor Appearances',
	TIMapGroupVisualizerTemplate: 'Map Visuals'
};

/** Template name for players, like the wiki's: overrides, the section label, else "TIFooBarTemplate" → "Foo Bars". */
export function templateLabel(template: string, sectionLabel?: string): string {
	if (TEMPLATE_LABELS[template]) return TEMPLATE_LABELS[template];
	if (sectionLabel) return sectionLabel;
	const words = humanize(template.replace(/^TI/, '').replace(/Template$/, ''));
	return words.endsWith('s') ? words : `${words}s`;
}
