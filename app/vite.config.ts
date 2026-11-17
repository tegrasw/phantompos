import process from "node:process";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

// Set by the Tauri CLI when developing on a physical mobile device.
const host = process.env.TAURI_DEV_HOST;

// Setup follows https://v2.tauri.app/start/frontend/vite/
// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte()],

  // Keep Rust compiler output visible in the `tauri dev` terminal.
  clearScreen: false,

  server: {
    // Must match `build.devUrl` in src-tauri/tauri.conf.json.
    port: 1420,
    // Tauri expects a fixed port; fail instead of picking another one.
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Rust sources are rebuilt by the Tauri CLI, not by Vite.
      ignored: ["**/src-tauri/**"],
    },
  },

  // Expose VITE_* and TAURI_ENV_* variables to the frontend.
  envPrefix: ["VITE_", "TAURI_ENV_*"],

  build: {
    // WebView2 on Windows is Chromium-based; macOS and Linux use WebKit.
    target:
      process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    // Readable output and sourcemaps for debug builds only.
    // (The Tauri guide says "esbuild"; Vite 8 defaults to Oxc and no longer
    // bundles esbuild, so `true` selects the built-in minifier.)
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    outDir: "dist",
  },
});
