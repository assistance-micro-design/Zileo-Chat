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
