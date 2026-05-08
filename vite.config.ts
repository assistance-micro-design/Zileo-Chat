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
    target: ['es2021', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG
  }
});
