// Game detection and the running extraction, shared by onboarding, the update notice and
// settings (Svelte 5 runes). The backend runs one extraction at a time.
import {
	api,
	EXTRACTION_CANCELLED,
	type ExtractArgs,
	type ExtractEvent,
	type ExtractResult,
	type ExtractStage,
	type GameInfo
} from '$lib/api';
import { settings } from '$lib/settings.svelte';

export { gameLabel, gameUpdate } from '$lib/game';

/** Extraction phases in order, as shown in the progress list. */
export const STAGES: { id: ExtractStage; label: string }[] = [
	{ id: 'templates', label: 'Templates' },
	{ id: 'locale', label: 'Localization' },
	{ id: 'schema', label: 'Schema' },
	{ id: 'images', label: 'Images' },
	{ id: 'finish', label: 'Finish' }
];

/** Extraction arguments from the saved options. */
export function extractArgs(game: GameInfo, force = false): ExtractArgs {
	return {
		gameDir: game.root,
		largeArt: settings.extractLargeArt,
		imageFormat: settings.extractImageFormat,
		force
	};
}

class Extraction {
	/** The detected (or chosen) install; null when there is none or before `detect`. */
	game = $state<GameInfo | null>(null);
	/** Detection has finished at least once. */
	detected = $state(false);
	detecting = $state(false);

	running = $state(false);
	cancelling = $state(false);
	/** Current phase; null before the first report. */
	stage = $state<ExtractStage | null>(null);
	done = $state(0);
	total = $state(0);
	/** Label of the current step or the latest log line. */
	label = $state<string | null>(null);
	/** Why the last extraction failed (`EXTRACTION_CANCELLED` if cancelled). */
	error = $state<string | null>(null);
	/** Arguments of the last extraction, for Retry. */
	lastArgs = $state.raw<ExtractArgs | null>(null);

	private detection: Promise<GameInfo | null> | null = null;

	get cancelled() {
		return this.error === EXTRACTION_CANCELLED;
	}

	/** Look for the install once (the saved folder first); `again` repeats the lookup. */
	detect(again = false): Promise<GameInfo | null> {
		if (this.detection && !again) return this.detection;
		this.detecting = true;
		this.detection = api
			.detectGame(settings.gameDir || null)
			.catch(() => null)
			.then((game) => {
				this.game = game;
				this.detected = true;
				this.detecting = false;
				return game;
			});
		return this.detection;
	}

	/** Use the install chosen by the user from now on (and remember its folder). */
	choose(game: GameInfo) {
		this.game = game;
		this.detected = true;
		settings.gameDir = game.root;
	}

	/**
	 * Extract with `args`; resolves with the result once the new build is loaded in the backend,
	 * or null on failure (see `error`).
	 */
	async run(args: ExtractArgs): Promise<ExtractResult | null> {
		if (this.running) return null;
		this.running = true;
		this.cancelling = false;
		this.error = null;
		this.stage = null;
		this.done = this.total = 0;
		this.label = null;
		this.lastArgs = args;
		try {
			return await api.extractGameData(args, (event) => this.report(event));
		} catch (cause) {
			this.error = this.cancelling ? EXTRACTION_CANCELLED : String(cause);
			return null;
		} finally {
			this.running = false;
			this.cancelling = false;
		}
	}

	private report(event: ExtractEvent) {
		switch (event.kind) {
			case 'stage':
				this.stage = event.stage;
				this.done = this.total = 0;
				this.label = null;
				break;
			case 'step':
				this.done = event.done;
				this.total = event.total;
				if (event.label) this.label = event.label;
				break;
			case 'log':
				this.label = event.message;
				break;
		}
	}

	cancel() {
		if (!this.running || this.cancelling) return;
		this.cancelling = true;
		void api.cancelExtraction().catch(() => {});
	}

	/** Forget a failed run (back to the options). */
	dismissError() {
		this.error = null;
	}
}

export const extraction = new Extraction();
