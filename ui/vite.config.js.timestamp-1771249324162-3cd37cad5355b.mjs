// vite.config.js
import { sveltekit } from "file:///home/donball197/nexus_singularity/ui/node_modules/@sveltejs/kit/src/exports/vite/index.js";
import { defineConfig } from "file:///home/donball197/nexus_singularity/ui/node_modules/vite/dist/node/index.js";
var vite_config_default = defineConfig({
  plugins: [sveltekit()],
  build: {
    chunkSizeWarningLimit: 1e3,
    // Bump limit to 1MB since we use heavy IDE libs
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
//# sourceMappingURL=data:application/json;base64,ewogICJ2ZXJzaW9uIjogMywKICAic291cmNlcyI6IFsidml0ZS5jb25maWcuanMiXSwKICAic291cmNlc0NvbnRlbnQiOiBbImNvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9kaXJuYW1lID0gXCIvaG9tZS9kb25iYWxsMTk3L25leHVzX3Npbmd1bGFyaXR5L3VpXCI7Y29uc3QgX192aXRlX2luamVjdGVkX29yaWdpbmFsX2ZpbGVuYW1lID0gXCIvaG9tZS9kb25iYWxsMTk3L25leHVzX3Npbmd1bGFyaXR5L3VpL3ZpdGUuY29uZmlnLmpzXCI7Y29uc3QgX192aXRlX2luamVjdGVkX29yaWdpbmFsX2ltcG9ydF9tZXRhX3VybCA9IFwiZmlsZTovLy9ob21lL2RvbmJhbGwxOTcvbmV4dXNfc2luZ3VsYXJpdHkvdWkvdml0ZS5jb25maWcuanNcIjtpbXBvcnQgeyBzdmVsdGVraXQgfSBmcm9tICdAc3ZlbHRlanMva2l0L3ZpdGUnO1xuaW1wb3J0IHsgZGVmaW5lQ29uZmlnIH0gZnJvbSAndml0ZSc7XG5cbmV4cG9ydCBkZWZhdWx0IGRlZmluZUNvbmZpZyh7XG5cdHBsdWdpbnM6IFtzdmVsdGVraXQoKV0sXG5cdGJ1aWxkOiB7XG5cdFx0Y2h1bmtTaXplV2FybmluZ0xpbWl0OiAxMDAwLCAvLyBCdW1wIGxpbWl0IHRvIDFNQiBzaW5jZSB3ZSB1c2UgaGVhdnkgSURFIGxpYnNcblx0XHRyb2xsdXBPcHRpb25zOiB7XG5cdFx0XHRvdXRwdXQ6IHtcblx0XHRcdFx0bWFudWFsQ2h1bmtzKGlkKSB7XG5cdFx0XHRcdFx0Ly8gUHV0IENvZGVNaXJyb3IgYW5kIGl0cyBsYW5ndWFnZXMgaW50byBhIHNlcGFyYXRlICd2ZW5kb3ItZWRpdG9yJyBjaHVua1xuXHRcdFx0XHRcdGlmIChpZC5pbmNsdWRlcygnY29kZW1pcnJvcicpIHx8IGlkLmluY2x1ZGVzKCdAY29kZW1pcnJvcicpKSB7XG5cdFx0XHRcdFx0XHRyZXR1cm4gJ3ZlbmRvci1lZGl0b3InO1xuXHRcdFx0XHRcdH1cblx0XHRcdFx0XHQvLyBQdXQgb3RoZXIgaGVhdnkgZGVwZW5kZW5jaWVzIGludG8gYSBnZW5lcmFsIHZlbmRvciBjaHVua1xuXHRcdFx0XHRcdGlmIChpZC5pbmNsdWRlcygnbm9kZV9tb2R1bGVzJykpIHtcblx0XHRcdFx0XHRcdHJldHVybiAndmVuZG9yJztcblx0XHRcdFx0XHR9XG5cdFx0XHRcdH1cblx0XHRcdH1cblx0XHR9XG5cdH1cbn0pO1xuIl0sCiAgIm1hcHBpbmdzIjogIjtBQUFpUyxTQUFTLGlCQUFpQjtBQUMzVCxTQUFTLG9CQUFvQjtBQUU3QixJQUFPLHNCQUFRLGFBQWE7QUFBQSxFQUMzQixTQUFTLENBQUMsVUFBVSxDQUFDO0FBQUEsRUFDckIsT0FBTztBQUFBLElBQ04sdUJBQXVCO0FBQUE7QUFBQSxJQUN2QixlQUFlO0FBQUEsTUFDZCxRQUFRO0FBQUEsUUFDUCxhQUFhLElBQUk7QUFFaEIsY0FBSSxHQUFHLFNBQVMsWUFBWSxLQUFLLEdBQUcsU0FBUyxhQUFhLEdBQUc7QUFDNUQsbUJBQU87QUFBQSxVQUNSO0FBRUEsY0FBSSxHQUFHLFNBQVMsY0FBYyxHQUFHO0FBQ2hDLG1CQUFPO0FBQUEsVUFDUjtBQUFBLFFBQ0Q7QUFBQSxNQUNEO0FBQUEsSUFDRDtBQUFBLEVBQ0Q7QUFDRCxDQUFDOyIsCiAgIm5hbWVzIjogW10KfQo=
