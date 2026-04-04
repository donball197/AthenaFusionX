import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
    plugins: [sveltekit()],

    optimizeDeps: {
        exclude: ['codemirror', '@codemirror/*', '@replit/codemirror-lang-svelte']
    },

    build: {
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
    },

    // Help with SSR/client boundary issues
    ssr: {
        noExternal: ['@codemirror/*', '@replit/codemirror-lang-svelte']
    }
});
