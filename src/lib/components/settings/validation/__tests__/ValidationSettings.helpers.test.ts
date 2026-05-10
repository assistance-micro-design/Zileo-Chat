import { describe, expect, it } from 'vitest';
import type { AvailableToolInfo } from '$types/validation';
import {
	RETENTION_MAX,
	RETENTION_MIN,
	TIMEOUT_MAX,
	TIMEOUT_MIN,
	clampRetention,
	clampTimeout,
	createValidationSettingsUpdateRequest,
	getAutoManualModeDisplay,
	splitAvailableTools
} from '../ValidationSettings.helpers';

describe('ValidationSettings helpers', () => {
	it('clamps timeout values to backend-compatible bounds', () => {
		expect(clampTimeout(TIMEOUT_MIN - 1)).toBe(TIMEOUT_MIN);
		expect(clampTimeout(TIMEOUT_MAX + 1)).toBe(TIMEOUT_MAX);
		expect(clampTimeout(42.6)).toBe(43);
		expect(clampTimeout(Number.NaN)).toBe(TIMEOUT_MIN);
	});

	it('clamps retention values to backend-compatible bounds', () => {
		expect(clampRetention(RETENTION_MIN - 1)).toBe(RETENTION_MIN);
		expect(clampRetention(RETENTION_MAX + 1)).toBe(RETENTION_MAX);
		expect(clampRetention(31.4)).toBe(31);
		expect(clampRetention(Number.NaN)).toBe(RETENTION_MIN);
	});

	it('creates the validation settings update request with current payload shape', () => {
		expect(
			createValidationSettingsUpdateRequest({
				mode: 'selective',
				subAgentsValidation: true,
				toolsValidation: false,
				mcpValidation: true,
				riskThresholds: {
					autoApproveLow: false,
					alwaysConfirmHigh: true
				},
				timeoutSeconds: TIMEOUT_MAX + 10,
				timeoutBehavior: 'approve',
				enableLogging: false,
				retentionDays: RETENTION_MIN - 10
			})
		).toEqual({
			mode: 'selective',
			selectiveConfig: {
				subAgents: true,
				tools: false,
				mcp: true,
				fileOps: false,
				dbOps: false
			},
			riskThresholds: {
				autoApproveLow: false,
				alwaysConfirmHigh: true
			},
			timeoutSeconds: TIMEOUT_MAX,
			timeoutBehavior: 'approve',
			audit: {
				enableLogging: false,
				retentionDays: RETENTION_MIN
			}
		});
	});

	it('splits available tools into basic and sub-agent groups without mutating input', () => {
		const tools = [
			{ name: 'MemoryTool', category: 'basic', requiresContext: false },
			{ name: 'SpawnAgentTool', category: 'sub_agent', requiresContext: true }
		] satisfies AvailableToolInfo[];
		const result = splitAvailableTools(tools);

		expect(result.basicTools).toEqual([tools[0]]);
		expect(result.subAgentTools).toEqual([tools[1]]);
		expect(tools).toHaveLength(2);
	});

	it('returns auto mode display metadata', () => {
		expect(getAutoManualModeDisplay('auto')).toEqual({
			variant: 'approved',
			icon: '✓',
			statusKey: 'validation_auto_approved',
			sectionTitleKey: 'validation_auto_title',
			sectionHelpKey: 'validation_auto_help'
		});
	});

	it('returns manual mode display metadata', () => {
		expect(getAutoManualModeDisplay('manual')).toEqual({
			variant: 'validation-required',
			icon: '⚠',
			statusKey: 'validation_requires_approval',
			sectionTitleKey: 'validation_manual_title',
			sectionHelpKey: 'validation_manual_help'
		});
	});
});
