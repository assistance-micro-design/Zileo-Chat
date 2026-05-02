/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/**
 * Currency-formatting helpers shared by chat metrics surfaces.
 *
 * Single source of truth: previously each component (MetricsBar, TokenDisplay)
 * had a slightly different `formatCost` definition, so the same workflow could
 * show two different strings for the same number.
 *
 * Pure module — no Svelte / store imports — so it can be unit-tested without
 * pulling the whole i18n boot. Callers pass their localized "free" label.
 */

/** Default label when callers don't pass one (English). */
const DEFAULT_FREE_LABEL = 'Free';

/**
 * Formats a USD cost with adaptive precision.
 *
 * - `0` (or non-finite values) → `freeLabel` (caller-provided, defaults to "Free")
 * - `< 0.0001` → `"<$0.0001"` to avoid showing `$0.00`
 * - `< 0.01` → 4 decimals (`$0.0042`)
 * - otherwise → 2 decimals (`$1.23`)
 *
 * Designed to be called from any component without per-callsite tweaks.
 */
export function formatCost(usd: number, freeLabel: string = DEFAULT_FREE_LABEL): string {
	if (!Number.isFinite(usd) || usd === 0) return freeLabel;
	if (usd < 0.0001) return '<$0.0001';
	if (usd < 0.01) return `$${usd.toFixed(4)}`;
	return `$${usd.toFixed(2)}`;
}

/**
 * Formats an optional cost. When `null`/`undefined` (backend has not yet
 * provided a cost), returns the placeholder used by Phase 7 metrics surfaces.
 *
 * The placeholder is a literal `—` (em-dash) so it doesn't compete visually
 * with real numeric values, and it doesn't say "Free" — the absence of cost
 * is not the same as a free request.
 */
export function formatCostOrPlaceholder(
	usd: number | null | undefined,
	freeLabel: string = DEFAULT_FREE_LABEL
): string {
	if (usd == null) return '—';
	return formatCost(usd, freeLabel);
}
