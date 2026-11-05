<script lang="ts">
	import { assetUrl, type EntityStats } from '$lib/api';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { SENTIMENT } from '$lib/effect-style';
	import EntityLink from './EntityLink.svelte';

	// `tags` is off where the caller already shows them (unlock card headers).
	let {
		stats,
		wide = false,
		tags = true
	}: { stats: EntityStats; wide?: boolean; tags?: boolean } = $props();
</script>

<div class="space-y-3">
	{#if tags && stats.tags.length}
		<div class="flex flex-wrap gap-1">
			{#each stats.tags as tag (tag)}
				<Badge variant="secondary" class="h-5 px-1.5">{tag}</Badge>
			{/each}
		</div>
	{/if}

	{#each stats.groups as group (group.label)}
		<section>
			<h5 class="mb-1 text-xs font-medium tracking-wide text-muted-foreground uppercase">
				{group.label}
			</h5>
			<dl
				class="grid gap-2 {wide ? 'grid-cols-[repeat(auto-fill,minmax(9rem,1fr))]' : 'grid-cols-2'}"
			>
				{#each group.stats as stat (stat.field)}
					{@const url = assetUrl(stat.icon)}
					<div
						class="min-w-0 rounded-md bg-muted/50 px-2 py-1.5"
						title="{stat.label} · {stat.field}"
					>
						<dt class="flex items-center gap-1 truncate text-xs text-muted-foreground">
							{#if url}
								<img
									src={url}
									alt=""
									class="size-4 shrink-0 rounded-sm bg-icon-bg object-contain p-px"
									draggable="false"
								/>
							{/if}
							<span class="truncate">{stat.label}</span>
						</dt>
						<dd
							class="truncate text-sm font-semibold tabular-nums {stat.sentiment === 'neutral'
								? ''
								: SENTIMENT[stat.sentiment].text}"
						>
							{stat.value}
						</dd>
						{#if stat.detail}
							<dd class="truncate text-xs text-muted-foreground" title={stat.detail}>
								{stat.detail}
							</dd>
						{/if}
					</div>
				{/each}
			</dl>
		</section>
	{/each}

	{#if stats.notes.length}
		<ul class="list-disc space-y-0.5 pl-4 text-xs/relaxed text-muted-foreground">
			{#each stats.notes as note, i (i)}
				<li class="whitespace-pre-line">{note}</li>
			{/each}
		</ul>
	{/if}

	{#each stats.links as link (link.label)}
		<div class="text-sm">
			<h5 class="mb-0.5 text-xs font-medium tracking-wide text-muted-foreground uppercase">
				{link.label}
			</h5>
			<div class="flex flex-wrap gap-x-1">
				{#each link.entities as entity (entity.key)}
					<EntityLink {entity} />
				{/each}
			</div>
		</div>
	{/each}
</div>
