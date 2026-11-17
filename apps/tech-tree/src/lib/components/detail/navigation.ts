import { getContext, setContext } from 'svelte';
import type { EntityRef } from '#lib/api.ts';
import { app } from '#lib/state.svelte.ts';
const key = Symbol('entity-navigation');
type Navigator = (entity: Pick<EntityRef, 'key' | 'kind' | 'spoiler'>) => void;
export function setEntityNavigator(fn: Navigator) {
	setContext(key, fn);
}
export function getEntityNavigator(): Navigator {
	return getContext<Navigator>(key) ?? ((entity) => app.navigate(entity));
}

const spoilerKey = Symbol('spoiler-policy');
export function setSpoilerPolicy(policy: () => boolean) {
	setContext(spoilerKey, policy);
}
export function getSpoilerPolicy(): () => boolean {
	return getContext<() => boolean>(spoilerKey) ?? (() => true);
}
