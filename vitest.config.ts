// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath, URL } from "node:url";

// Single source of truth for Vitest configuration. vite.config.ts no longer
// declares a `test` block to avoid the two configs drifting apart.
export default defineConfig({
  plugins: [svelte({ hot: !process.env.VITEST })],
  test: {
    include: ["src/**/*.{test,spec}.{js,ts}"],
    environment: "jsdom",
    globals: true,
    setupFiles: ["src/tests/setup.ts"],
  },
  resolve: {
    alias: {
      $lib: fileURLToPath(new URL("./src/lib", import.meta.url)),
      $app: fileURLToPath(new URL("./src/app", import.meta.url)),
      $types: fileURLToPath(new URL("./src/types", import.meta.url)),
      $messages: fileURLToPath(new URL("./src/messages", import.meta.url)),
    },
  },
});
