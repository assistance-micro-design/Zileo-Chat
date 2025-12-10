// Copyright 2025 Assistance Micro Design
// SPDX-License-Identifier: Apache-2.0

/**
 * Settings layout load function
 * Provides URL pathname to child components for route-based navigation
 */
export function load({ url }: { url: URL }) {
	return {
		pathname: url.pathname
	};
}
