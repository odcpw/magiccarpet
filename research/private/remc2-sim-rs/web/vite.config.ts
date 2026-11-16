import { defineConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig(({ mode }) => ({
  server: {
    host: "127.0.0.1",
    port: 8878,
    strictPort: true,
    proxy: { "/original-api": { target: "http://127.0.0.1:8881", changeOrigin: false } }
  },
  preview: {
    host: "127.0.0.1",
    port: 8878,
    strictPort: true
  },
  build: {
    target: "es2023",
    sourcemap: true,
    rollupOptions: mode === "original"
      ? { input: { original: resolve(import.meta.dirname, "original.html") } }
      : undefined
  }
}));
