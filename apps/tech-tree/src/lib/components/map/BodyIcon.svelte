<script lang="ts">
	// A space body's game icon; without one (mock mode, missing asset) a shaded disc in its map color.
	import { assetUrl, type SpaceBodyInfo } from '$lib/api';
	import { bodyColor } from './scene';

	let { info, size = 20 }: { info: SpaceBodyInfo; size?: number } = $props();

	const url = $derived(assetUrl(info.entity.icon));
	let failed = $state(false);
	$effect(() => {
		void url;
		failed = false;
	});
	const color = $derived(bodyColor(info));
	const ringed = $derived(info.entity.dataName === 'Saturn');
	const gradient = $props.id();
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
{:else}
	<!-- A `size-*` class keeps buttons and menu items from shrinking it to their icon size. -->
	<svg
		viewBox="0 0 24 24"
		class="size-(--body-icon) shrink-0"
		style:--body-icon="{size}px"
		aria-hidden="true"
	>
		<defs>
			<radialGradient id={gradient} cx="35%" cy="35%" r="70%">
				<stop offset="0%" stop-color="white" stop-opacity="0.45" />
				<stop offset="60%" stop-color="white" stop-opacity="0" />
				<stop offset="100%" stop-color="black" stop-opacity="0.35" />
			</radialGradient>
		</defs>
		{#if ringed}
			<!-- Back half of the ring, then the planet, then the front half. -->
			<path
				d="M2.34 14.59 A10 3.5 -15 0 1 21.66 9.41"
				fill="none"
				stroke={color}
				stroke-width="1.5"
			/>
		{/if}
		<circle cx="12" cy="12" r={ringed ? 6.5 : 9} fill={color} />
		<circle cx="12" cy="12" r={ringed ? 6.5 : 9} fill="url(#{gradient})" />
		{#if ringed}
			<path
				d="M21.66 9.41 A10 3.5 -15 0 1 2.34 14.59"
				fill="none"
				stroke={color}
				stroke-width="1.5"
			/>
		{/if}
	</svg>
{/if}
