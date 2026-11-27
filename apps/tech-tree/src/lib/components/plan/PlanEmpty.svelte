<script lang="ts">
	// The plan page without a campaign: what planning does, and ways to start (a new campaign,
	// a save import, or one of the saved campaigns).
	import FileInput from '@lucide/svelte/icons/file-input';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import Plus from '@lucide/svelte/icons/plus';
	import Route from '@lucide/svelte/icons/route';
	import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
	import { campaign } from '#lib/campaign.svelte.ts';
	import { Button } from '#lib/components/ui/button/index.js';
	import { app } from '#lib/state.svelte.ts';
	import { planUi } from './plan-ui.svelte.ts';

	let importing = $state(false);

	async function importSave() {
		importing = true;
		try {
			await campaign.importSaveFile();
		} finally {
			importing = false;
		}
	}

	const scopeLabel = (scenario: string, faction: string) =>
		[
			app.meta?.scenarios.find((s) => s.name === scenario)?.label ?? scenario,
			app.meta?.factions.find((f) => f.name === faction)?.label ?? faction
		].join(' · ');
	const RECENT = 5;
</script>

<div class="grid h-full place-items-center overflow-auto bg-background p-6">
	<div class="grid w-full max-w-md gap-5">
		<div class="grid justify-items-center gap-3 text-center">
			<div class="grid size-11 place-items-center rounded-xl border bg-card text-muted-foreground">
				<Route class="size-5" />
			</div>
			<h2 class="text-lg font-semibold">Plan research for a campaign</h2>
			<p class="text-sm text-muted-foreground">
				Mark what you have researched, pick goals, and see what to research next and the path there.
				Import a save to fill in your progress, or start from scratch.
			</p>
		</div>
		<div class="flex flex-wrap justify-center gap-2">
			<Button onclick={importSave} disabled={importing}>
				{#if importing}<LoaderCircle class="animate-spin" />{:else}<FileInput />{/if}
				Import from save…
			</Button>
			<Button variant="outline" onclick={() => (planUi.creating = true)}>
				<Plus /> New campaign
			</Button>
		</div>
		{#if campaign.error}
			<p class="flex items-start gap-2 text-sm text-destructive" role="alert">
				<TriangleAlert class="mt-0.5 size-4 shrink-0" />
				<span class="min-w-0 break-words">{campaign.error}</span>
			</p>
		{/if}
		{#if campaign.list?.length}
			<section class="grid gap-1.5" aria-labelledby="plan-empty-recent">
				<h3 id="plan-empty-recent" class="text-xs font-medium text-muted-foreground">
					Open a campaign
				</h3>
				<ul class="grid overflow-hidden rounded-lg border bg-card">
					{#each campaign.list.slice(0, RECENT) as c (c.id || 'new')}
						<li class="border-b last:border-b-0">
							<button
								type="button"
								class="grid w-full min-w-0 px-3 py-2 text-left hover:bg-accent focus-visible:bg-accent focus-visible:outline-none"
								disabled={!c.id}
								onclick={() => campaign.activate(c.id)}
							>
								<span class="truncate text-sm font-medium">{c.name}</span>
								<span class="truncate text-xs text-muted-foreground"
									>{scopeLabel(c.scenario, c.faction)}{#if c.save}
										· from save, {c.save.gameDate}{/if}</span
								>
							</button>
						</li>
					{/each}
				</ul>
				{#if campaign.list.length > RECENT}
					<p class="text-xs text-muted-foreground">More in the campaign menu in the toolbar.</p>
				{/if}
			</section>
		{/if}
	</div>
</div>
