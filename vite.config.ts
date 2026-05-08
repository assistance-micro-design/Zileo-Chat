import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

// Note: Vitest configuration lives in vitest.config.ts (source of truth).
// Keeping it out of this file avoids drift between dev/build aliases
// (resolved by SvelteKit) and test aliases.

export default defineConfig({
  plugins: [sveltekit()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: 'localhost',
    watch: {
      ignored: ['**/src-tauri/target/**']
    }
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    // Tauri 2 ships modern WebView (WebKit on macOS/Linux, Edge WebView2 on
    // Windows). chrome105 / safari15 covers all platforms — older targets
    // emit unnecessary polyfills.
    target: ['es2022', 'chrome105', 'safari15'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG
  }
});
