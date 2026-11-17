import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

import pkg from './package.json' with { type: 'json' };

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			preprocess: vitePreprocess(),
			// Tauri serves a static single-page app; see src/routes/+layout.ts
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	// package.json holds the app version (tauri.conf.json reads it too)
	define: { __APP_VERSION__: JSON.stringify(pkg.version) },
	// Tauri expects a fixed port and its own console output
	clearScreen: false,
	server: {
		port: 1420,
		strictPort: true,
		host: host || false,
		hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
		watch: { ignored: ['**/src-tauri/**'] }
	}
});
