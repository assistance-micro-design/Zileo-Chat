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
  UserQuestionModal Component
  Displays interactive questions from the LLM agent to the user.
  Supports checkbox options, text input, and mixed mode.
  User must respond or skip - modal is not closeable.

  @example
  <UserQuestionModal />
-->
<script lang="ts">
	import { Button, Textarea } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import {
		userQuestionStore,
		currentQuestion,
		isSubmitting,
		isModalOpen
	} from '$lib/stores/userQuestion';
	import type { UserQuestionResponse } from '$types/user-question';

	let selectedOptions = $state<string[]>([]);
	let textResponse = $state('');

	// Reset state when question changes
	$effect(() => {
		if ($currentQuestion) {
			selectedOptions = [];
			textResponse = '';
		}
	});

	/**
	 * Toggle checkbox option selection
	 */
	function toggleOption(optionId: string): void {
		if (selectedOptions.includes(optionId)) {
			selectedOptions = selectedOptions.filter((id) => id !== optionId);
		} else {
			selectedOptions = [...selectedOptions, optionId];
		}
	}

	/**
	 * Check if the current form is valid for submission
	 */
	function isValid(): boolean {
		const q = $currentQuestion;
		if (!q) return false;

		if (q.questionType === 'checkbox') {
			return selectedOptions.length > 0;
		}

		if (q.questionType === 'text') {
			return textResponse.trim().length > 0;
		}

		if (q.questionType === 'mixed') {
			const hasSelection = selectedOptions.length > 0;
			const hasText = !q.textRequired || textResponse.trim().length > 0;
			return hasSelection && hasText;
		}

		return false;
	}

	/**
	 * Handle form submission
	 */
	async function handleSubmit(): Promise<void> {
		console.log('[UserQuestionModal] handleSubmit called', {
			currentQuestion: $currentQuestion,
			isValid: isValid(),
			selectedOptions,
			textResponse
		});

		if (!$currentQuestion || !isValid()) {
			console.warn('[UserQuestionModal] Submit blocked - invalid state');
			return;
		}

		const response: UserQuestionResponse = {
			questionId: $currentQuestion.id,
			selectedOptions,
			textResponse: textResponse.trim() || undefined
		};

		console.log('[UserQuestionModal] Submitting response:', response);
		await userQuestionStore.submitResponse(response);
		console.log('[UserQuestionModal] Response submitted');
	}

	/**
	 * Handle skipping the question
	 */
	async function handleSkip(): Promise<void> {
		console.log('[UserQuestionModal] handleSkip called', { currentQuestion: $currentQuestion });

		if (!$currentQuestion) {
			console.warn('[UserQuestionModal] Skip blocked - no current question');
			return;
		}

		console.log('[UserQuestionModal] Skipping question:', $currentQuestion.id);
		await userQuestionStore.skipQuestion($currentQuestion.id);
		console.log('[UserQuestionModal] Question skipped');
	}

</script>

{#if $isModalOpen}
	<div class="modal-backdrop" role="presentation">
		<div
			class="modal"
			role="dialog"
			aria-modal="true"
			aria-labelledby="modal-title"
			tabindex="-1"
			onclick={(e) => e.stopPropagation()}
			onkeydown={(e) => e.stopPropagation()}
		>
			<div class="modal-header">
				<h3 id="modal-title" class="modal-title">{$i18n('user_question_modal_title')}</h3>
			</div>

			<div class="modal-body">
				{#if $currentQuestion}
					<div class="user-question-content">
						<p class="question-text">{$currentQuestion.question}</p>

						{#if $currentQuestion.context}
							<div class="context-box">
								<span class="context-label">{$i18n('user_question_context')}:</span>
								<p>{$currentQuestion.context}</p>
							</div>
						{/if}

						{#if ($currentQuestion.questionType === 'checkbox' || $currentQuestion.questionType === 'mixed') && $currentQuestion.options}
							<div class="options-container">
								{#each $currentQuestion.options as option}
									<label class="option-item">
										<input
											type="checkbox"
											checked={selectedOptions.includes(option.id)}
											onchange={() => toggleOption(option.id)}
										/>
										<span>{option.label}</span>
									</label>
								{/each}
							</div>
						{/if}

						{#if $currentQuestion.questionType === 'text' || $currentQuestion.questionType === 'mixed'}
							<Textarea
								label={$i18n('user_question_text_label')}
								placeholder={$currentQuestion.textPlaceholder ?? $i18n('user_question_text_placeholder')}
								value={textResponse}
								oninput={(e) => (textResponse = e.currentTarget.value)}
								rows={4}
							/>
							{#if $currentQuestion.textRequired}
								<span class="required-hint">{$i18n('user_question_text_required')}</span>
							{/if}
						{/if}
					</div>
				{/if}
			</div>

			<div class="modal-footer">
				<div class="modal-actions">
					<Button variant="ghost" onclick={handleSkip} disabled={$isSubmitting}>
						{$i18n('user_question_skip')}
					</Button>
					<Button variant="primary" onclick={handleSubmit} disabled={!isValid() || $isSubmitting}>
						{#if $isSubmitting}
							{$i18n('common_saving')}
						{:else}
							{$i18n('user_question_submit')}
						{/if}
					</Button>
				</div>
			</div>
		</div>
	</div>
{/if}

<style>
	.modal-backdrop {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: rgba(0, 0, 0, 0.5);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
		padding: var(--spacing-lg);
	}

	.modal {
		background: var(--color-bg-primary);
		border-radius: var(--border-radius-lg);
		box-shadow: var(--shadow-xl);
		max-width: 600px;
		width: 100%;
		max-height: 90vh;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--spacing-lg);
		border-bottom: 1px solid var(--color-border);
	}

	.modal-title {
		font-size: var(--font-size-xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.modal-body {
		flex: 1;
		overflow-y: auto;
		padding: var(--spacing-lg);
	}

	.modal-footer {
		padding: var(--spacing-lg);
		border-top: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
	}

	.user-question-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.question-text {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
		margin: 0;
		line-height: 1.5;
	}

	.context-box {
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
		padding: var(--spacing-md);
		font-size: var(--font-size-sm);
	}

	.context-box p {
		margin: var(--spacing-xs) 0 0 0;
		color: var(--color-text-secondary);
		line-height: 1.5;
	}

	.context-label {
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
	}

	.options-container {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.option-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: background 0.15s;
		user-select: none;
	}

	.option-item:hover {
		background: var(--color-bg-hover);
	}

	.option-item input[type='checkbox'] {
		width: 18px;
		height: 18px;
		accent-color: var(--color-primary);
		cursor: pointer;
	}

	.option-item span {
		font-size: var(--font-size-base);
		color: var(--color-text-primary);
		line-height: 1.5;
	}

	.required-hint {
		font-size: var(--font-size-xs);
		color: var(--color-warning);
		margin-top: calc(var(--spacing-xs) * -1);
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-md);
	}
</style>
