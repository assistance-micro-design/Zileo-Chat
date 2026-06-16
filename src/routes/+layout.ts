// Copyright 2025 Assistance Micro Design
// SPDX-License-Identifier: Apache-2.0

/**
 * Root layout config: the whole app is a client-only SPA rendered inside the
 * Tauri webview. Disabling SSR and prerendering here applies to every route by
 * inheritance, so a newly added page cannot accidentally opt into SSR/prerender
 * (which would break `adapter-static` with `strict: true`). Individual routes no
 * longer need to repeat these flags.
 */

export const ssr = false;
export const prerender = false;
