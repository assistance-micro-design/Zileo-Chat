import type {
	AvailableToolInfo,
	RiskThresholdConfig,
	TimeoutBehavior,
	UpdateValidationSettingsRequest,
	ValidationMode
} from '$types/validation';

export const TIMEOUT_MIN = 5;
export const TIMEOUT_MAX = 600;
export const RETENTION_MIN = 7;
export const RETENTION_MAX = 90;

export const timeoutBehaviorOptions: Array<{ value: TimeoutBehavior; labelKey: string }> = [
	{ value: 'reject', labelKey: 'validation_timeout_behavior_reject' },
	{ value: 'approve', labelKey: 'validation_timeout_behavior_approve' },
	{ value: 'skip', labelKey: 'validation_timeout_behavior_skip' }
];

export const modeOptions: Array<{ value: ValidationMode; labelKey: string; descKey: string }> = [
	{
		value: 'auto',
		labelKey: 'validation_mode_auto',
		descKey: 'validation_mode_auto_desc'
	},
	{
		value: 'manual',
		labelKey: 'validation_mode_manual',
		descKey: 'validation_mode_manual_desc'
	},
	{
		value: 'selective',
		labelKey: 'validation_mode_selective',
		descKey: 'validation_mode_selective_desc'
	}
];

export interface ValidationSettingsFormState {
	mode: ValidationMode;
	subAgentsValidation: boolean;
	toolsValidation: boolean;
	mcpValidation: boolean;
	riskThresholds: RiskThresholdConfig;
	timeoutSeconds: number;
	timeoutBehavior: TimeoutBehavior;
	enableLogging: boolean;
	retentionDays: number;
}

export interface SplitAvailableToolsResult {
	basicTools: AvailableToolInfo[];
	subAgentTools: AvailableToolInfo[];
}

export interface AutoManualModeDisplay {
	variant: 'approved' | 'validation-required';
	icon: string;
	statusKey: string;
	sectionTitleKey: string;
	sectionHelpKey: string;
}

export function clampTimeout(value: number): number {
	if (Number.isNaN(value)) return TIMEOUT_MIN;
	return Math.min(TIMEOUT_MAX, Math.max(TIMEOUT_MIN, Math.round(value)));
}

export function clampRetention(value: number): number {
	if (Number.isNaN(value)) return RETENTION_MIN;
	return Math.min(RETENTION_MAX, Math.max(RETENTION_MIN, Math.round(value)));
}

export function createValidationSettingsUpdateRequest(
	input: ValidationSettingsFormState
): UpdateValidationSettingsRequest {
	return {
		mode: input.mode,
		selectiveConfig: {
			subAgents: input.subAgentsValidation,
			tools: input.toolsValidation,
			mcp: input.mcpValidation,
			fileOps: false,
			dbOps: false
		},
		riskThresholds: input.riskThresholds,
		timeoutSeconds: clampTimeout(input.timeoutSeconds),
		timeoutBehavior: input.timeoutBehavior,
		audit: {
			enableLogging: input.enableLogging,
			retentionDays: clampRetention(input.retentionDays)
		}
	};
}

export function splitAvailableTools(tools: AvailableToolInfo[]): SplitAvailableToolsResult {
	return {
		basicTools: tools.filter((tool) => tool.category === 'basic'),
		subAgentTools: tools.filter((tool) => tool.category === 'sub_agent')
	};
}

export function getAutoManualModeDisplay(mode: 'auto' | 'manual'): AutoManualModeDisplay {
	if (mode === 'auto') {
		return {
			variant: 'approved',
			icon: '✓',
			statusKey: 'validation_auto_approved',
			sectionTitleKey: 'validation_auto_title',
			sectionHelpKey: 'validation_auto_help'
		};
	}

	return {
		variant: 'validation-required',
		icon: '⚠',
		statusKey: 'validation_requires_approval',
		sectionTitleKey: 'validation_manual_title',
		sectionHelpKey: 'validation_manual_help'
	};
}
