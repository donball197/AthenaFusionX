// vite.config.js
import { sveltekit } from "file:///home/donball197/nexus_singularity-dev/ui/node_modules/@sveltejs/kit/src/exports/vite/index.js";
import { defineConfig } from "file:///home/donball197/nexus_singularity-dev/ui/node_modules/vite/dist/node/index.js";
var vite_config_default = defineConfig({
  plugins: [sveltekit()],
  server: {
    proxy: {
      "/api": "http://localhost:3000",
      "/ws/telemetry": { target: "ws://localhost:3000", ws: true }
    },
    fs: { allow: [".."] }
  },
  build: {
    chunkSizeWarningLimit: 1e3,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes("codemirror") || id.includes("@codemirror")) return "vendor-editor";
          if (id.includes("node_modules")) return "vendor";
        }
      }
    }
  }
});
export {
  vite_config_default as default
};
//# sourceMappingURL=data:application/json;base64,ewogICJ2ZXJzaW9uIjogMywKICAic291cmNlcyI6IFsidml0ZS5jb25maWcuanMiXSwKICAic291cmNlc0NvbnRlbnQiOiBbImNvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9kaXJuYW1lID0gXCIvaG9tZS9kb25iYWxsMTk3L25leHVzX3Npbmd1bGFyaXR5LWRldi91aVwiO2NvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9maWxlbmFtZSA9IFwiL2hvbWUvZG9uYmFsbDE5Ny9uZXh1c19zaW5ndWxhcml0eS1kZXYvdWkvdml0ZS5jb25maWcuanNcIjtjb25zdCBfX3ZpdGVfaW5qZWN0ZWRfb3JpZ2luYWxfaW1wb3J0X21ldGFfdXJsID0gXCJmaWxlOi8vL2hvbWUvZG9uYmFsbDE5Ny9uZXh1c19zaW5ndWxhcml0eS1kZXYvdWkvdml0ZS5jb25maWcuanNcIjtpbXBvcnQgeyBzdmVsdGVraXQgfSBmcm9tICdAc3ZlbHRlanMva2l0L3ZpdGUnO1xuaW1wb3J0IHsgZGVmaW5lQ29uZmlnIH0gZnJvbSAndml0ZSc7XG5cbmV4cG9ydCBkZWZhdWx0IGRlZmluZUNvbmZpZyh7XG4gICAgcGx1Z2luczogW3N2ZWx0ZWtpdCgpXSxcbiAgICBzZXJ2ZXI6IHtcbiAgICAgICAgcHJveHk6IHtcbiAgICAgICAgICAgICcvYXBpJzogJ2h0dHA6Ly9sb2NhbGhvc3Q6MzAwMCcsXG4gICAgICAgICAgICAnL3dzL3RlbGVtZXRyeSc6IHsgdGFyZ2V0OiAnd3M6Ly9sb2NhbGhvc3Q6MzAwMCcsIHdzOiB0cnVlIH1cbiAgICAgICAgfSxcbiAgICAgICAgZnM6IHsgYWxsb3c6IFsnLi4nXSB9XG4gICAgfSxcbiAgICBidWlsZDoge1xuICAgICAgICBjaHVua1NpemVXYXJuaW5nTGltaXQ6IDEwMDAsXG4gICAgICAgIHJvbGx1cE9wdGlvbnM6IHtcbiAgICAgICAgICAgIG91dHB1dDoge1xuICAgICAgICAgICAgICAgIG1hbnVhbENodW5rcyhpZCkge1xuICAgICAgICAgICAgICAgICAgICBpZiAoaWQuaW5jbHVkZXMoJ2NvZGVtaXJyb3InKSB8fCBpZC5pbmNsdWRlcygnQGNvZGVtaXJyb3InKSkgcmV0dXJuICd2ZW5kb3ItZWRpdG9yJztcbiAgICAgICAgICAgICAgICAgICAgaWYgKGlkLmluY2x1ZGVzKCdub2RlX21vZHVsZXMnKSkgcmV0dXJuICd2ZW5kb3InO1xuICAgICAgICAgICAgICAgIH1cbiAgICAgICAgICAgIH1cbiAgICAgICAgfVxuICAgIH1cbn0pO1xuIl0sCiAgIm1hcHBpbmdzIjogIjtBQUE2UyxTQUFTLGlCQUFpQjtBQUN2VSxTQUFTLG9CQUFvQjtBQUU3QixJQUFPLHNCQUFRLGFBQWE7QUFBQSxFQUN4QixTQUFTLENBQUMsVUFBVSxDQUFDO0FBQUEsRUFDckIsUUFBUTtBQUFBLElBQ0osT0FBTztBQUFBLE1BQ0gsUUFBUTtBQUFBLE1BQ1IsaUJBQWlCLEVBQUUsUUFBUSx1QkFBdUIsSUFBSSxLQUFLO0FBQUEsSUFDL0Q7QUFBQSxJQUNBLElBQUksRUFBRSxPQUFPLENBQUMsSUFBSSxFQUFFO0FBQUEsRUFDeEI7QUFBQSxFQUNBLE9BQU87QUFBQSxJQUNILHVCQUF1QjtBQUFBLElBQ3ZCLGVBQWU7QUFBQSxNQUNYLFFBQVE7QUFBQSxRQUNKLGFBQWEsSUFBSTtBQUNiLGNBQUksR0FBRyxTQUFTLFlBQVksS0FBSyxHQUFHLFNBQVMsYUFBYSxFQUFHLFFBQU87QUFDcEUsY0FBSSxHQUFHLFNBQVMsY0FBYyxFQUFHLFFBQU87QUFBQSxRQUM1QztBQUFBLE1BQ0o7QUFBQSxJQUNKO0FBQUEsRUFDSjtBQUNKLENBQUM7IiwKICAibmFtZXMiOiBbXQp9Cg==
