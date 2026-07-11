import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],

  server: {
    host: "0.0.0.0",
    port: 5173,
    strictPort: true,

    proxy: {
      "/api": {
        target: "http://localhost:3000",
        changeOrigin: true,
        secure: false,
      },

      "/ws": {
        target: "ws://localhost:3000",
        ws: true,
        changeOrigin: true,
      },
    },
  },
});
