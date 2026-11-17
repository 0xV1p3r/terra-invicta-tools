// Entity details for the expanded hover card, cached across hovers; dropped when the scope changes.
import { api, type EntityDetail, type EntityKey, type ScopeArgs } from '#lib/api.ts';

let cacheScope = '';
/** Bumped on every scope change, so requests from a scope left and re-entered stay stale. */
let generation = 0;
const cache = new Map<EntityKey, EntityDetail>();
const pending = new Map<EntityKey, Promise<EntityDetail>>();

function scoped(scope: ScopeArgs) {
	const id = JSON.stringify(scope);
	if (id !== cacheScope) {
		cacheScope = id;
		generation++;
		cache.clear();
		pending.clear();
	}
	return generation;
}

export function loadDetail(scope: ScopeArgs, key: EntityKey): Promise<EntityDetail> {
	const gen = scoped(scope);
	const existing = pending.get(key);
	if (existing) return existing;
	const request: Promise<EntityDetail> = api.getEntity(scope, key).then(
		(d) => {
			if (generation === gen) cache.set(key, d);
			if (pending.get(key) === request) pending.delete(key);
			return d;
		},
		(e) => {
			// Not cached: hovering again retries.
			if (pending.get(key) === request) pending.delete(key);
			throw e;
		}
	);
	pending.set(key, request);
	return request;
}

/** Already fetched detail, if any. */
export function cachedDetail(scope: ScopeArgs, key: EntityKey): EntityDetail | undefined {
	scoped(scope);
	return cache.get(key);
}
