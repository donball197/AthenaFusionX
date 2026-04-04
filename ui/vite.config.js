import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	build: {
		chunkSizeWarningLimit: 1000, // Bump limit to 1MB since we use heavy IDE libs
		rollupOptions: {
			output: {
				manualChunks(id) {
					// Put CodeMirror and its languages into a separate 'vendor-editor' chunk
					if (id.includes('codemirror') || id.includes('@codemirror')) {
						return 'vendor-editor';
					}
					// Put other heavy dependencies into a general vendor chunk
					if (id.includes('node_modules')) {
						return 'vendor';
					}
				}
			}
		}
	}
});
