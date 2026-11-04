// Display metadata for effects shared by the detail panel and the effect filter.
import type { Component } from 'svelte';
import type { EffectDomain, EffectSentiment } from '$lib/api';
import Award from '@lucide/svelte/icons/award';
import Bug from '@lucide/svelte/icons/bug';
import Building2 from '@lucide/svelte/icons/building-2';
import Coins from '@lucide/svelte/icons/coins';
import Crosshair from '@lucide/svelte/icons/crosshair';
import Flag from '@lucide/svelte/icons/flag';
import FlaskConical from '@lucide/svelte/icons/flask-conical';
import Landmark from '@lucide/svelte/icons/landmark';
import Leaf from '@lucide/svelte/icons/leaf';
import Megaphone from '@lucide/svelte/icons/megaphone';
import Orbit from '@lucide/svelte/icons/orbit';
import Pickaxe from '@lucide/svelte/icons/pickaxe';
import Rocket from '@lucide/svelte/icons/rocket';
import SatelliteDish from '@lucide/svelte/icons/satellite-dish';
import Sparkles from '@lucide/svelte/icons/sparkles';
import Swords from '@lucide/svelte/icons/swords';
import Telescope from '@lucide/svelte/icons/telescope';
import UserRound from '@lucide/svelte/icons/user-round';

export const DOMAIN_ICONS: Record<EffectDomain, Component> = {
	research: FlaskConical,
	priority: Landmark,
	mission: Crosshair,
	control: Flag,
	councilor: UserRound,
	army: Swords,
	ship: Rocket,
	mining: Pickaxe,
	missionControl: SatelliteDish,
	exploration: Telescope,
	transfer: Orbit,
	hab: Building2,
	alien: Bug,
	environment: Leaf,
	economy: Coins,
	politics: Megaphone,
	ability: Award,
	other: Sparkles
};

// `ring` frames game icons, which are drawn for a dark background.
export const SENTIMENT: Record<
	EffectSentiment,
	{ tile: string; ring: string; badge: string; text: string; label: string }
> = {
	good: {
		tile: 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-400',
		ring: 'ring-emerald-500/60',
		badge: 'border-emerald-500/40 bg-emerald-500/15 text-emerald-700 dark:text-emerald-300',
		text: 'text-emerald-600 dark:text-emerald-400',
		label: 'Beneficial'
	},
	bad: {
		tile: 'bg-rose-500/15 text-rose-700 dark:text-rose-400',
		ring: 'ring-rose-500/60',
		badge: 'border-rose-500/40 bg-rose-500/15 text-rose-700 dark:text-rose-300',
		text: 'text-rose-600 dark:text-rose-400',
		label: 'Detrimental'
	},
	neutral: {
		tile: 'bg-muted text-muted-foreground',
		ring: 'ring-border',
		badge: 'border-border bg-muted text-foreground',
		text: 'text-muted-foreground',
		label: ''
	}
};
