// Node flag metadata: labels and lucide icons for DOM use, plus the same glyphs as SVG
// path data (24×24 viewBox, stroked) for the canvas renderer.
// Glyph paths from Lucide (https://lucide.dev), ISC license.
import type { Component } from 'svelte';
import Ban from '@lucide/svelte/icons/ban';
import Flag from '@lucide/svelte/icons/flag';
import Globe from '@lucide/svelte/icons/globe';
import Package from '@lucide/svelte/icons/package';
import Repeat from '@lucide/svelte/icons/repeat';
import Shuffle from '@lucide/svelte/icons/shuffle';
import Users from '@lucide/svelte/icons/users';
import type { NodeFlags } from '#lib/api.ts';

export type FlagKey = keyof NodeFlags;

export interface FlagInfo {
	key: FlagKey;
	icon: Component;
	label: string;
	/** SVG path data of the glyph. */
	glyph: string;
	/** Shown when the flag is false (`obtainable`) instead of true. */
	inverted?: boolean;
}

const circle = (cx: number, cy: number, r: number) =>
	`M${cx - r} ${cy}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0`;

export const FLAGS: FlagInfo[] = [
	{
		key: 'scenarioSpecific',
		icon: Globe,
		label: 'Only available in some scenarios',
		glyph: `${circle(12, 12, 10)}M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20M2 12h20`
	},
	{
		key: 'variesByScenario',
		icon: Shuffle,
		label: 'Prerequisites or cost vary by scenario',
		glyph:
			'm18 14 4 4-4 4M18 2l4 4-4 4M2 18h1.973a4 4 0 0 0 3.3-1.7l5.454-8.6a4 4 0 0 1 3.3-1.7H22' +
			'M2 6h1.972a4 4 0 0 1 3.6 2.2M22 18h-6.041a4 4 0 0 1-3.3-1.8l-.359-.45'
	},
	{
		key: 'dlc',
		icon: Package,
		label: 'Requires DLC',
		glyph:
			'M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4' +
			'A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73zM12 22V12M3.29 7 12 12l8.71-5M7.5 4.27l9 5.15'
	},
	{
		key: 'factionLocked',
		icon: Users,
		label: 'Restricted to some factions',
		glyph: `M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2M16 3.128a4 4 0 0 1 0 7.744M22 21v-2a4 4 0 0 0-3-3.87${circle(9, 7, 4)}`
	},
	{
		key: 'endGame',
		icon: Flag,
		label: 'End-game research',
		glyph:
			'M4 22V4a1 1 0 0 1 .4-.8A6 6 0 0 1 8 2c3 0 5 2 7.333 2q2 0 3.067-.8A1 1 0 0 1 20 4v10' +
			'a1 1 0 0 1-.4.8A6 6 0 0 1 16 16c-3 0-5-2-8-2a6 6 0 0 0-4 1.528'
	},
	{
		key: 'repeatable',
		icon: Repeat,
		label: 'Repeatable',
		glyph: 'm17 2 4 4-4 4M3 11v-1a4 4 0 0 1 4-4h14M7 22l-4-4 4-4M21 13v1a4 4 0 0 1-4 4H3'
	},
	{
		key: 'obtainable',
		icon: Ban,
		label: 'Not obtainable in the current scope',
		glyph: `${circle(12, 12, 10)}m4.9 4.9 14.2 14.2`,
		inverted: true
	}
];

/** Flags to show for a node, in display order. */
export function activeFlags(flags: NodeFlags): FlagInfo[] {
	return FLAGS.filter((f) => flags[f.key] !== !!f.inverted);
}
