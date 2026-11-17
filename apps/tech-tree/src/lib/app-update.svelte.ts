// Updates of the app itself (not the game data), from the GitHub releases (Svelte 5 runes).
// The updater plugin verifies each download against the public key in tauri.conf.json.
import type { Update } from '@tauri-apps/plugin-updater';
import { mockMode } from '#lib/api.ts';
import { settings } from '#lib/settings.svelte.ts';

type Status = 'idle' | 'checking' | 'downloading' | 'installing' | 'error';

// The plugin has no timeouts of its own: a stalled connection would block retries until restart.
const CHECK_TIMEOUT_MS = 30_000;
/** For the whole download (the AppImage is about 90 MB), so it allows a slow connection. */
const DOWNLOAD_TIMEOUT_MS = 30 * 60_000;

class AppUpdate {
	/** The newer release, if the last check found one. */
	available = $state.raw<Update | null>(null);
	status = $state<Status>('idle');
	/** Download progress 0–100, or null while the size is unknown. */
	percent = $state<number | null>(null);
	error = $state('');
	/** A check has finished; with `available` null, the app is up to date. */
	checked = $state(false);
	private startupDone = false;

	/** The notice shows a found update unless this version was dismissed. */
	get notice(): Update | null {
		const update = this.available;
		return update && !settings.dismissedAppUpdates.includes(update.version) ? update : null;
	}

	/** Once per app run, when enabled in settings. */
	checkAtStartup() {
		if (this.startupDone || !settings.checkAppUpdates) return;
		this.startupDone = true;
		void this.check();
	}

	async check() {
		if (mockMode || this.status === 'checking' || this.busy) return;
		this.status = 'checking';
		this.error = '';
		try {
			const { check } = await import('@tauri-apps/plugin-updater');
			this.available = await check({ timeout: CHECK_TIMEOUT_MS });
			this.status = 'idle';
		} catch (cause) {
			this.error = String(cause);
			this.status = 'error';
		} finally {
			this.checked = true;
		}
	}

	get busy() {
		return this.status === 'downloading' || this.status === 'installing';
	}

	/** Download, install and restart. On Windows the installer exits the app itself. */
	async install() {
		const update = this.available;
		if (!update || this.busy) return;
		this.status = 'downloading';
		this.percent = null;
		this.error = '';
		let total = 0;
		let received = 0;
		try {
			await update.downloadAndInstall(
				(event) => {
					if (event.event === 'Started') total = event.data.contentLength ?? 0;
					else if (event.event === 'Progress') {
						received += event.data.chunkLength;
						if (total > 0) this.percent = Math.min(100, Math.round((received / total) * 100));
					} else this.status = 'installing';
				},
				{ timeout: DOWNLOAD_TIMEOUT_MS }
			);
			const { relaunch } = await import('@tauri-apps/plugin-process');
			await relaunch();
		} catch (cause) {
			this.error = String(cause);
			this.status = 'error';
		}
	}

	dismiss() {
		const update = this.available;
		if (update) settings.dismissedAppUpdates = [...settings.dismissedAppUpdates, update.version];
	}
}

export const appUpdate = new AppUpdate();
