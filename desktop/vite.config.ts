import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  base: './',
  plugins: [svelte()],
  server: {
    port: 5173,
    strictPort: true,
  },
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "esnext",
    minify: "esbuild",
    sourcemap: false,
  },
});