// Copyright 2025 Assistance Micro Design
// SPDX-License-Identifier: Apache-2.0

/**
 * Settings root redirect: `/settings` sends the user to the default
 * providers sub-page without rendering an intermediate shell.
 */

import { redirect } from '@sveltejs/kit';

export const ssr = false;
export const prerender = false;

export const load = (): never => {
	throw redirect(307, '/settings/providers');
};
