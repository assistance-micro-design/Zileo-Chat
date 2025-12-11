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

export type QuestionType = 'checkbox' | 'text' | 'mixed';
export type QuestionStatus = 'pending' | 'answered' | 'skipped' | 'timeout';

export interface QuestionOption {
  id: string;
  label: string;
}

export interface UserQuestion {
  id: string;
  workflowId: string;
  agentId: string;
  question: string;
  questionType: QuestionType;
  options?: QuestionOption[];
  textPlaceholder?: string;
  textRequired?: boolean;
  context?: string;
  status: QuestionStatus;
  selectedOptions?: string[];
  textResponse?: string;
  createdAt: string;
  answeredAt?: string;
}

export interface UserQuestionResponse {
  questionId: string;
  selectedOptions: string[];
  textResponse?: string;
}

export interface UserQuestionStreamPayload {
  questionId: string;
  question: string;
  questionType: QuestionType;
  options?: QuestionOption[];
  textPlaceholder?: string;
  textRequired?: boolean;
  context?: string;
}
