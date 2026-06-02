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
  GlobalValidationModal — hosts the human-in-the-loop ValidationModal at the app
  root so a pending validation surfaces on EVERY route, not only /agent. Without
  this, an ATTENDED Kanban workflow (e.g. a card supervisor chat) that requests a
  tool validation would emit the event into the void: nobody renders the modal,
  the backend poll times out and applies the default behavior (reject) silently.

  Self-contained, mirroring UserQuestionModal: it owns the validationStore
  lifecycle (init on mount / cleanup on destroy) and is driven entirely by the
  workflow-aware store — no page context required. Mounted ONCE in
  routes/+layout.svelte next to <UserQuestionModal />.

  `open` is piloted one-way by `hasPendingValidation`. ValidationModal is
  fail-safe: backdrop, Escape and the header close button are disabled and the
  footer has no Cancel, so the ONLY ways to close it are Approve / Reject (which
  send a decision to the backend) or the backend resolving the request itself
  (timeout / resolved → the store clears the pending entry → `open` goes false).
  No user path closes the modal while leaving the backend waiting without a
  decision (which would let the timeout_behavior fire as a misleading default).
-->
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import {
		validationStore,
		pendingValidation,
		hasPendingValidation,
		isValidating,
		validationError,
		pendingValidationCount
	} from '$lib/stores/validation';
	import ValidationModal from './ValidationModal.svelte';
	import type { ValidationRequest } from '$types/validation';

	onMount(() => {
		// The store guards against double-init (isInitialized), so this is safe
		// even though the store is a singleton. Listener failures are swallowed
		// inside the store.
		void validationStore.init().catch(() => {});
	});

	onDestroy(() => {
		void validationStore.cleanup();
	});

	/** Approve the pending validation (store invokes `approve_validation`). */
	function handleApprove(): void {
		void validationStore.approve();
	}

	/** Reject the pending validation (store invokes `reject_validation`). */
	function handleReject(_request: ValidationRequest, reason?: string): void {
		void validationStore.reject(reason);
	}
</script>

<ValidationModal
	request={$pendingValidation}
	open={$hasPendingValidation}
	processing={$isValidating}
	error={$validationError}
	queueCount={$pendingValidationCount}
	onapprove={handleApprove}
	onreject={handleReject}
/>
