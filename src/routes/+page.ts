// Copyright 2025 Assistance Micro Design
// SPDX-License-Identifier: Apache-2.0

/**
 * Root redirect: entering `/` sends the user to the Kanban task board — the
 * default landing surface — without rendering an intermediate shell (no meta
 * refresh flash). The agent workspace stays reachable from the nav bar.
 */

import { redirect } from '@sveltejs/kit';

export const ssr = false;
export const prerender = false;

export const load = (): never => {
	throw redirect(307, '/kanban');
};
