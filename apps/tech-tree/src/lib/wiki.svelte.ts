import type { EntityKey, WikiSection } from '#lib/api.ts';
import { templateLabel } from '#lib/format.ts';
class WikiState {
	reset() {
		this.open = false;
		this.section = null;
		this.selected = null;
		this.back = [];
		this.forward = [];
		this.sections = [];
		this.spoilers = {};
	}
	open = $state(false);
	section = $state<{ template: string; spoiler: boolean } | null>(null);
	get template() {
		return this.section?.template ?? null;
	}
	set template(template: string | null) {
		this.section = template ? { template, spoiler: false } : null;
	}
	spoilers = $state<Record<string, boolean>>({});
	hideSpoilers() {
		this.back = this.back.filter((k) => !this.spoilers[k]);
		this.forward = this.forward.filter((k) => !this.spoilers[k]);
		if (this.section?.spoiler || (this.selected && this.spoilers[this.selected])) {
			this.section = null;
			this.selected = null;
		}
	}
	remember(key: EntityKey, spoiler: boolean) {
		this.spoilers[key] = spoiler;
	}
	selected = $state<EntityKey | null>(null);
	back = $state<EntityKey[]>([]);
	forward = $state<EntityKey[]>([]);
	sections = $state.raw<WikiSection[]>([]);
	label(template: string) {
		return templateLabel(template, this.sections.find((s) => s.template === template)?.label);
	}
	show(template?: string, spoiler = false) {
		this.open = true;
		if (template && (template !== this.template || spoiler !== this.section?.spoiler)) {
			// Leaving the current page is a navigation step: Back returns to it.
			if (this.selected) this.back = [...this.back.slice(-99), this.selected];
			this.forward = [];
			this.section = { template, spoiler };
			this.selected = null;
		}
	}
	/** Narrow layout: back to the entry list (the page stays reachable via Back). */
	closeEntity() {
		if (this.selected) this.back = [...this.back.slice(-99), this.selected];
		this.forward = [];
		this.selected = null;
	}
	openEntity(key: EntityKey, spoiler?: boolean) {
		if (spoiler !== undefined) this.remember(key, spoiler);
		if (this.selected !== key) {
			if (this.selected) this.back = [...this.back.slice(-99), this.selected];
			this.forward = [];
			this.select(key);
		}
		this.open = true;
	}
	private select(key: EntityKey) {
		this.selected = key;
		const template = key.split(':')[0];
		const spoiler = this.spoilers[key] ?? false;
		// A new section object would reload the entry list (and reset its scroll position).
		if (this.section?.template !== template || this.section.spoiler !== spoiler) {
			this.section = { template, spoiler };
		}
	}
	goBack() {
		const key = this.back.at(-1);
		if (!key) return;
		if (this.selected) this.forward = [...this.forward, this.selected];
		this.back = this.back.slice(0, -1);
		this.select(key);
	}
	goForward() {
		const key = this.forward.at(-1);
		if (!key) return;
		if (this.selected) this.back = [...this.back, this.selected];
		this.forward = this.forward.slice(0, -1);
		this.select(key);
	}
}
export const wiki = new WikiState();
