<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { ModeWatcher } from 'mode-watcher';

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Terra Invicta Tech Tree</title>
</svelte:head>

<ModeWatcher defaultMode="dark" />
{@render children?.()}
