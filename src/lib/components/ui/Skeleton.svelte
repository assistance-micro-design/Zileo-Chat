<!--
  Copyright 2025 Assistance Micro Design

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

<!--
  Skeleton Component
  A loading placeholder that shows animated skeleton shapes.
  Used for skeleton loading states during data fetching.

  @example
  <Skeleton variant="text" width="200px" />
  <Skeleton variant="circular" size="48px" />
  <Skeleton variant="rectangular" width="100%" height="120px" />
-->
<script lang="ts">
	/**
	 * Skeleton shape variants
	 */
	export type SkeletonVariant = 'text' | 'circular' | 'rectangular';

	/**
	 * Skeleton props
	 */
	interface Props {
		/** Shape variant */
		variant?: SkeletonVariant;
		/** Width (CSS value) */
		width?: string;
		/** Height (CSS value, required for rectangular) */
		height?: string;
		/** Size for circular variant (CSS value) */
		size?: string;
		/** Whether to animate the skeleton */
		animate?: boolean;
		/** Additional CSS classes */
		class?: string;
	}

	let {
		variant = 'text',
		width,
		height,
		size,
		animate = true,
		class: className = ''
	}: Props = $props();

	/**
	 * Compute inline styles based on props
	 */
	const style = $derived.by(() => {
		const styles: string[] = [];

		if (variant === 'circular') {
			const circleSize = size ?? '40px';
			styles.push(`width: ${circleSize}`);
			styles.push(`height: ${circleSize}`);
		} else {
			if (width) styles.push(`width: ${width}`);
			if (height) styles.push(`height: ${height}`);
		}

		return styles.join('; ');
	});
</script>

<div
	class="skeleton skeleton-{variant} {className}"
	class:skeleton-animate={animate}
	style={style}
	aria-hidden="true"
	role="presentation"
></div>

<style>
	.skeleton {
		background: var(--color-bg-tertiary);
		position: relative;
		overflow: hidden;
	}

	.skeleton-text {
		height: 1em;
		border-radius: var(--radius-sm, 4px);
		width: 100%;
	}

	.skeleton-circular {
		border-radius: var(--radius-full, 9999px);
	}

	.skeleton-rectangular {
		border-radius: var(--radius-md, 8px);
		min-height: 1rem;
	}

	.skeleton-animate::after {
		content: '';
		position: absolute;
		inset: 0;
		background: linear-gradient(
			90deg,
			transparent 0%,
			rgba(255, 255, 255, 0.15) 50%,
			transparent 100%
		);
		animation: shimmer 1.5s infinite;
	}

	@keyframes shimmer {
		0% {
			transform: translateX(-100%);
		}
		100% {
			transform: translateX(100%);
		}
	}

	/* Respect reduced motion preference */
	@media (prefers-reduced-motion: reduce) {
		.skeleton-animate::after {
			animation: none;
		}
	}
</style>
