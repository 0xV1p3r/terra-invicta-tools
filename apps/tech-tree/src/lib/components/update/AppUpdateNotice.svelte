<script lang="ts">
	import Download from '@lucide/svelte/icons/download';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import X from '@lucide/svelte/icons/x';
	import { appUpdate } from '$lib/app-update.svelte';
	import Tip from '$lib/components/toolbar/Tip.svelte';
	import { Button } from '$lib/components/ui/button/index.js';

	// After the first paint: the check is a network request.
	$effect(() => {
		const timer = setTimeout(() => appUpdate.checkAtStartup(), 1500);
		return () => clearTimeout(timer);
	});

	const update = $derived(appUpdate.notice);
	const progress = $derived(
		appUpdate.status === 'installing'
			? 'Installing…'
			: appUpdate.percent === null
				? 'Downloading…'
				: `Downloading… ${appUpdate.percent}%`
	);
</script>

{#if update}
	<div
		class="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1.5 border-b bg-card px-3 py-1.5 text-sm"
		role="region"
		aria-label="App update"
	>
		<Download class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
		<p role="status">
			<span class="font-medium">Terra Invicta Tools {update.version} is available</span>
			<span class="text-muted-foreground"
				>{#if appUpdate.busy}{progress}{:else if appUpdate.status === 'error'}Update failed: {appUpdate.error}{:else}(you
					have {__APP_VERSION__}){/if}</span
			>
		</p>
		<div class="ml-auto flex items-center gap-1">
			<Button size="sm" disabled={appUpdate.busy} onclick={() => void appUpdate.install()}>
				{#if appUpdate.busy}<LoaderCircle class="animate-spin" />{/if}
				{appUpdate.status === 'error' ? 'Try again' : 'Install and restart'}
			</Button>
			<Tip text="Dismiss until the next version">
				<Button
					variant="ghost"
					size="icon"
					class="size-8"
					aria-label="Dismiss app update notice"
					disabled={appUpdate.busy}
					onclick={() => appUpdate.dismiss()}
				>
					<X />
				</Button>
			</Tip>
		</div>
	</div>
{/if}
