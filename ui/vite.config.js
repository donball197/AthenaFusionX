import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		proxy: {
			// Redirect all API calls to the Rust backend
			'/api': 'http://localhost:3000',
			// Secure the WebSocket bridge for real-time telemetry
			'/ws/nexus': {
				target: 'ws://localhost:3000',
				ws: true
			}
		},
		fs: {
			// Allow Vite to serve files from the workspace DNA
			allow: ['..']
		}
	},
	build: {
		chunkSizeWarningLimit: 1000,
		rollupOptions: {
			output: {
				manualChunks(id) {
					if (id.includes('codemirror') || id.includes('@codemirror')) {
						return 'vendor-editor';
					}
					if (id.includes('node_modules')) {
						return 'vendor';
					}
				}
			}
		}
	}
});
