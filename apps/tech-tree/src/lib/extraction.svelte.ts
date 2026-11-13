// Game detection and the running extraction, shared by onboarding, the update notice and
// settings (Svelte 5 runes). The backend runs one extraction at a time.
import {
	api,
	EXTRACTION_CANCELLED,
	type ExtractArgs,
	type ExtractEvent,
	type ExtractResult,
	type ExtractStage,
	type ExtractStatus,
	type GameInfo
} from '$lib/api';
import { advanceProgress, initialProgress } from './extraction-progress';
import { settings } from '$lib/settings.svelte';

export { gameLabel, gameUpdate } from '$lib/game';

/** How often `resume` asks the backend for the progress of an extraction it cannot follow. */
const STATUS_POLL_MS = 500;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

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
	percent = $state(0);
	private progress = initialProgress();
	private pendingLabel: string | null = null;
	private progressTimer: ReturnType<typeof setTimeout> | null = null;
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
		this.pendingLabel = null;
		this.progress = initialProgress();
		this.flushProgress();
		this.label = null;
		this.lastArgs = args;
		try {
			const result = await api.extractGameData(args, (event) => this.report(event));
			this.progress = { ...this.progress, percent: 100 };
			return result;
		} catch (cause) {
			this.error = this.cancelling ? EXTRACTION_CANCELLED : String(cause);
			return null;
		} finally {
			this.flushProgress();
			this.running = false;
			this.cancelling = false;
		}
	}

	/**
	 * Follow an extraction the backend is already running (one started before a window reload,
	 * whose progress channel is gone) until it ends, so no conflicting action is offered
	 * meanwhile. Resolves true when one ran and was not cancelled: its build may be loaded now.
	 */
	async resume(): Promise<boolean> {
		if (this.running) return false;
		let status = await api.extractionStatus().catch(() => null);
		if (!status || this.running) return false;
		this.running = true;
		this.cancelling = false;
		this.error = null;
		try {
			while (status) {
				this.show(status);
				await sleep(STATUS_POLL_MS);
				status = await api.extractionStatus().catch(() => null);
			}
			// The outcome is unknown here; a cancel is the one failure we can tell.
			if (this.cancelling) this.error = EXTRACTION_CANCELLED;
			return !this.cancelling;
		} finally {
			this.flushProgress();
			this.running = false;
			this.cancelling = false;
		}
	}

	private flushProgress() {
		if (this.progressTimer !== null) clearTimeout(this.progressTimer);
		this.progressTimer = null;
		this.stage = this.progress.stage;
		this.done = this.progress.done;
		this.total = this.progress.total;
		this.percent = this.progress.percent;
		this.label = this.pendingLabel;
	}

	private updateProgress(stage: ExtractStage | null, done = 0, total = 0) {
		const changed = stage !== this.progress.stage;
		this.progress = advanceProgress(this.progress, stage, done, total);
		if (changed) this.flushProgress();
		else if (this.progressTimer === null)
			this.progressTimer = setTimeout(() => this.flushProgress(), 100);
	}

	private show(status: ExtractStatus) {
		this.pendingLabel = status.label;
		this.updateProgress(status.stage, status.done, status.total);
	}

	private report(event: ExtractEvent) {
		switch (event.kind) {
			case 'stage':
				this.pendingLabel = null;
				this.updateProgress(event.stage);
				break;
			case 'step':
				if (event.label) this.pendingLabel = event.label;
				this.updateProgress(this.progress.stage, event.done, event.total);
				break;
			case 'log':
				this.pendingLabel = event.message;
				this.updateProgress(this.progress.stage, this.progress.done, this.progress.total);
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
