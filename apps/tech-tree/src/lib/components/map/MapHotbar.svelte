<script lang="ts">
	// Hotbar over the map: the Sun and each planet, one click to focus it (like the game's).
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import BodyIcon from './BodyIcon.svelte';
	import { majors } from './majors.svelte';
</script>

{#if majors.planets.length}
	<div
		class="flex items-center gap-0.5 rounded-xl border bg-card p-1 shadow"
		role="group"
		aria-label="Planets"
	>
		{#each majors.planets as body, i (body.key)}
			{#if i === 1}<span class="mx-0.5 h-6 w-px bg-border" aria-hidden="true"></span>{/if}
			<Tooltip.Root>
				<Tooltip.Trigger>
					{#snippet child({ props })}
						<Button
							{...props}
							variant="ghost"
							size="icon"
							class="size-9 aria-[current]:bg-accent aria-[current]:ring-1 aria-[current]:ring-ring"
							aria-label="Focus {body.info.entity.name}"
							aria-current={majors.focused === body.key ? 'true' : undefined}
							onclick={() => majors.focus(body.key)}
						>
							<BodyIcon info={body.info} size={24} />
						</Button>
					{/snippet}
				</Tooltip.Trigger>
				<Tooltip.Content side="bottom">{body.info.entity.name}</Tooltip.Content>
			</Tooltip.Root>
		{/each}
	</div>
{/if}
