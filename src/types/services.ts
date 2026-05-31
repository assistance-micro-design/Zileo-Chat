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
 * @fileoverview Service layer types for agent page workflows.
 *
 * @module types/services
 */

/**
 * Modal state for agent page dialogs.
 *
 * Validation is NOT a variant here: the human-in-the-loop ValidationModal is
 * mounted globally (GlobalValidationModal in the root layout) and driven by the
 * validationStore, so it is not part of the agent page's local modal machine.
 */
export type ModalState =
	| { type: 'none' }
	| { type: 'new-workflow' }
	| { type: 'delete-workflow'; workflowId: string };
