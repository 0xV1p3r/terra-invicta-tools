<script lang="ts">
	import { assetUrl, type EntityStats } from '#lib/api.ts';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { SENTIMENT } from '#lib/effect-style.ts';
	import { statLabel, statTile, statValue, subHeading } from '#lib/components/styles.ts';
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
			<h4 class="mb-1 {subHeading}">{group.label}</h4>
			<dl
				class="grid gap-2 {wide ? 'grid-cols-[repeat(auto-fill,minmax(9rem,1fr))]' : 'grid-cols-2'}"
			>
				{#each group.stats as stat (stat.field)}
					{@const url = assetUrl(stat.icon)}
					<div class={statTile} title="{stat.label} · {stat.field}">
						<dt class="flex items-center gap-1 {statLabel}">
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
							class="{statValue} {stat.sentiment === 'neutral'
								? ''
								: SENTIMENT[stat.sentiment].text}"
						>
							{stat.value}
						</dd>
						{#if stat.detail}
							<dd class={statLabel} title={stat.detail}>
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
			<h4 class="mb-0.5 {subHeading}">{link.label}</h4>
			<div class="flex flex-wrap gap-x-1">
				{#each link.entities as entity (entity.key)}
					<EntityLink {entity} />
				{/each}
			</div>
		</div>
	{/each}
</div>
