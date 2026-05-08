import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
    plugins: [sveltekit()],
    server: {
        proxy: {
            '/api': 'http://localhost:3000',
            '/ws/telemetry': { target: 'ws://localhost:3000', ws: true }
        },
        fs: { allow: ['..'] }
    },
    build: {
        chunkSizeWarningLimit: 1000,
        rollupOptions: {
            output: {
                manualChunks(id) {
                    if (id.includes('codemirror') || id.includes('@codemirror')) return 'vendor-editor';
                    if (id.includes('node_modules')) return 'vendor';
                }
            }
        }
    }
});
