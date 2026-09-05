import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  base: "./",
  server: { proxy: { "/api": { target: process.env.STELLATUNE_NETEASE_UI_API || "http://127.0.0.1:19000", changeOrigin: true } } },
  build: {
    outDir: "../ui",
    emptyOutDir: true
  }
});
