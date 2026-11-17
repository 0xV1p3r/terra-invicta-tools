<script lang="ts">
	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';

	let { children } = $props();

	// The webview's default menu (Reload, Inspect, ...) makes no sense in the app;
	// text fields keep it for copy and paste.
	function suppressContextMenu(event: MouseEvent) {
		const target = event.target as HTMLElement | null;
		if (target?.closest('input, textarea, [contenteditable="true"]')) return;
		event.preventDefault();
	}
</script>

<svelte:window oncontextmenu={suppressContextMenu} />

<svelte:head>
	<link rel="icon" href={favicon} />
	<title>Terra Invicta Tools</title>
</svelte:head>

{@render children?.()}
