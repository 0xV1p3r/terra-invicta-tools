<script lang="ts" module>
	import type { Component } from 'svelte';
	import type { EntityKind } from '$lib/api';
	import Award from '@lucide/svelte/icons/award';
	import BookOpen from '@lucide/svelte/icons/book-open';
	import Building2 from '@lucide/svelte/icons/building-2';
	import Crosshair from '@lucide/svelte/icons/crosshair';
	import FlaskConical from '@lucide/svelte/icons/flask-conical';
	import Rocket from '@lucide/svelte/icons/rocket';
	import Sparkles from '@lucide/svelte/icons/sparkles';
	import Target from '@lucide/svelte/icons/target';
	import Box from '@lucide/svelte/icons/box';
	import Flag from '@lucide/svelte/icons/flag';
	import UserRound from '@lucide/svelte/icons/user-round';
	import Wrench from '@lucide/svelte/icons/wrench';

	export const KIND_ICONS: Record<EntityKind, Component> = {
		tech: FlaskConical,
		project: Wrench,
		effect: Sparkles,
		habModule: Building2,
		shipPart: Rocket,
		org: Award,
		trait: UserRound,
		faction: Flag,
		objective: Target,
		mission: Crosshair,
		codex: BookOpen,
		other: Box
	};
</script>

<script lang="ts">
	import type { EntityRef } from '$lib/api';
	import { assetUrl } from '$lib/api';
	import CategoryIcon from '$lib/components/tree/CategoryIcon.svelte';

	let {
		entity,
		size = 20,
		fallbackIcon
	}: { entity: EntityRef; size?: number; fallbackIcon?: Component } = $props();

	const url = $derived(assetUrl(entity.icon));
	let failed = $state(false);
	const KindIcon = $derived(
		(entity.kind === 'other' ? fallbackIcon : undefined) ?? KIND_ICONS[entity.kind] ?? Box
	);

	$effect(() => {
		void url;
		failed = false;
	});
</script>

{#if url && !failed}
	<img
		src={url}
		alt=""
		width={size}
		height={size}
		class="shrink-0 object-contain"
		onerror={() => (failed = true)}
	/>
{:else if entity.category && (entity.kind === 'tech' || entity.kind === 'project')}
	<CategoryIcon category={entity.category} {size} />
{:else}
	<span
		class="inline-flex shrink-0 items-center justify-center rounded bg-muted text-muted-foreground"
		style:width="{size}px"
		style:height="{size}px"
		aria-hidden="true"
	>
		<KindIcon style="width: {size * 0.65}px; height: {size * 0.65}px" />
	</span>
{/if}
