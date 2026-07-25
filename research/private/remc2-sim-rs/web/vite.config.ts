import { defineConfig } from "vite";

export default defineConfig({
  server: {
    host: "127.0.0.1",
    port: 8878,
    strictPort: true
  },
  preview: {
    host: "127.0.0.1",
    port: 8878,
    strictPort: true
  },
  build: {
    target: "es2023",
    sourcemap: true
  }
});
