import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		adapter: adapter({
			fallback: 'index.html',
			pages: 'dist',
			assets: 'dist',
			precompress: false,
			strict: true
		}),
		// SUTURE: Force-disable service worker to bypass the rename race condition
		serviceWorker: {
			register: false,
		}
	}
};

export default config;
