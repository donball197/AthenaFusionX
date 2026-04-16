// vite.config.js
import { sveltekit } from "file:///home/donball197/nexus_singularity-dev/ui/node_modules/@sveltejs/kit/src/exports/vite/index.js";
import { defineConfig } from "file:///home/donball197/nexus_singularity-dev/ui/node_modules/vite/dist/node/index.js";
var vite_config_default = defineConfig({
  plugins: [sveltekit()],
  server: {
    proxy: {
      // Redirect all API calls to the Rust backend
      "/api": "http://localhost:3000",
      // Secure the WebSocket bridge for real-time telemetry
      "/ws/nexus": {
        target: "ws://localhost:3000",
        ws: true
      }
    },
    fs: {
      // Allow Vite to serve files from the workspace DNA
      allow: [".."]
    }
  },
  build: {
    chunkSizeWarningLimit: 1e3,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes("codemirror") || id.includes("@codemirror")) {
            return "vendor-editor";
          }
          if (id.includes("node_modules")) {
            return "vendor";
          }
        }
      }
    }
  }
});
export {
  vite_config_default as default
};
//# sourceMappingURL=data:application/json;base64,ewogICJ2ZXJzaW9uIjogMywKICAic291cmNlcyI6IFsidml0ZS5jb25maWcuanMiXSwKICAic291cmNlc0NvbnRlbnQiOiBbImNvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9kaXJuYW1lID0gXCIvaG9tZS9kb25iYWxsMTk3L25leHVzX3Npbmd1bGFyaXR5LWRldi91aVwiO2NvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9maWxlbmFtZSA9IFwiL2hvbWUvZG9uYmFsbDE5Ny9uZXh1c19zaW5ndWxhcml0eS1kZXYvdWkvdml0ZS5jb25maWcuanNcIjtjb25zdCBfX3ZpdGVfaW5qZWN0ZWRfb3JpZ2luYWxfaW1wb3J0X21ldGFfdXJsID0gXCJmaWxlOi8vL2hvbWUvZG9uYmFsbDE5Ny9uZXh1c19zaW5ndWxhcml0eS1kZXYvdWkvdml0ZS5jb25maWcuanNcIjtpbXBvcnQgeyBzdmVsdGVraXQgfSBmcm9tICdAc3ZlbHRlanMva2l0L3ZpdGUnO1xuaW1wb3J0IHsgZGVmaW5lQ29uZmlnIH0gZnJvbSAndml0ZSc7XG5cbmV4cG9ydCBkZWZhdWx0IGRlZmluZUNvbmZpZyh7XG5cdHBsdWdpbnM6IFtzdmVsdGVraXQoKV0sXG5cdHNlcnZlcjoge1xuXHRcdHByb3h5OiB7XG5cdFx0XHQvLyBSZWRpcmVjdCBhbGwgQVBJIGNhbGxzIHRvIHRoZSBSdXN0IGJhY2tlbmRcblx0XHRcdCcvYXBpJzogJ2h0dHA6Ly9sb2NhbGhvc3Q6MzAwMCcsXG5cdFx0XHQvLyBTZWN1cmUgdGhlIFdlYlNvY2tldCBicmlkZ2UgZm9yIHJlYWwtdGltZSB0ZWxlbWV0cnlcblx0XHRcdCcvd3MvbmV4dXMnOiB7XG5cdFx0XHRcdHRhcmdldDogJ3dzOi8vbG9jYWxob3N0OjMwMDAnLFxuXHRcdFx0XHR3czogdHJ1ZVxuXHRcdFx0fVxuXHRcdH0sXG5cdFx0ZnM6IHtcblx0XHRcdC8vIEFsbG93IFZpdGUgdG8gc2VydmUgZmlsZXMgZnJvbSB0aGUgd29ya3NwYWNlIEROQVxuXHRcdFx0YWxsb3c6IFsnLi4nXVxuXHRcdH1cblx0fSxcblx0YnVpbGQ6IHtcblx0XHRjaHVua1NpemVXYXJuaW5nTGltaXQ6IDEwMDAsXG5cdFx0cm9sbHVwT3B0aW9uczoge1xuXHRcdFx0b3V0cHV0OiB7XG5cdFx0XHRcdG1hbnVhbENodW5rcyhpZCkge1xuXHRcdFx0XHRcdGlmIChpZC5pbmNsdWRlcygnY29kZW1pcnJvcicpIHx8IGlkLmluY2x1ZGVzKCdAY29kZW1pcnJvcicpKSB7XG5cdFx0XHRcdFx0XHRyZXR1cm4gJ3ZlbmRvci1lZGl0b3InO1xuXHRcdFx0XHRcdH1cblx0XHRcdFx0XHRpZiAoaWQuaW5jbHVkZXMoJ25vZGVfbW9kdWxlcycpKSB7XG5cdFx0XHRcdFx0XHRyZXR1cm4gJ3ZlbmRvcic7XG5cdFx0XHRcdFx0fVxuXHRcdFx0XHR9XG5cdFx0XHR9XG5cdFx0fVxuXHR9XG59KTtcbiJdLAogICJtYXBwaW5ncyI6ICI7QUFBNlMsU0FBUyxpQkFBaUI7QUFDdlUsU0FBUyxvQkFBb0I7QUFFN0IsSUFBTyxzQkFBUSxhQUFhO0FBQUEsRUFDM0IsU0FBUyxDQUFDLFVBQVUsQ0FBQztBQUFBLEVBQ3JCLFFBQVE7QUFBQSxJQUNQLE9BQU87QUFBQTtBQUFBLE1BRU4sUUFBUTtBQUFBO0FBQUEsTUFFUixhQUFhO0FBQUEsUUFDWixRQUFRO0FBQUEsUUFDUixJQUFJO0FBQUEsTUFDTDtBQUFBLElBQ0Q7QUFBQSxJQUNBLElBQUk7QUFBQTtBQUFBLE1BRUgsT0FBTyxDQUFDLElBQUk7QUFBQSxJQUNiO0FBQUEsRUFDRDtBQUFBLEVBQ0EsT0FBTztBQUFBLElBQ04sdUJBQXVCO0FBQUEsSUFDdkIsZUFBZTtBQUFBLE1BQ2QsUUFBUTtBQUFBLFFBQ1AsYUFBYSxJQUFJO0FBQ2hCLGNBQUksR0FBRyxTQUFTLFlBQVksS0FBSyxHQUFHLFNBQVMsYUFBYSxHQUFHO0FBQzVELG1CQUFPO0FBQUEsVUFDUjtBQUNBLGNBQUksR0FBRyxTQUFTLGNBQWMsR0FBRztBQUNoQyxtQkFBTztBQUFBLFVBQ1I7QUFBQSxRQUNEO0FBQUEsTUFDRDtBQUFBLElBQ0Q7QUFBQSxFQUNEO0FBQ0QsQ0FBQzsiLAogICJuYW1lcyI6IFtdCn0K
