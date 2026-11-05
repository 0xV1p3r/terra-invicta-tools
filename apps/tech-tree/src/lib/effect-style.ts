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
		tile: 'bg-success-subtle text-success',
		ring: 'ring-success-border',
		badge: 'border-success-border bg-success-subtle text-success',
		text: 'text-success',
		label: 'Beneficial'
	},
	bad: {
		tile: 'bg-detrimental-subtle text-detrimental',
		ring: 'ring-detrimental-border',
		badge: 'border-detrimental-border bg-detrimental-subtle text-detrimental',
		text: 'text-detrimental',
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
