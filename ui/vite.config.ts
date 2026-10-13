import { defineConfig } from "vite";

// Tauri serves the built files from ui/dist (see crates/x4sd/tauri.conf.json).
export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  // The bundle (mostly Three.js) is loaded from inside the exe, not over a network.
  build: { target: "es2022", outDir: "dist", emptyOutDir: true, chunkSizeWarningLimit: 1000 },
});
