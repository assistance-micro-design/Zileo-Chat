import type { ExportFormat } from '$types/embedding';
import type { MemoryType } from '$types/memory';

export type BadgeVariant = 'primary' | 'success' | 'warning' | 'error';

export interface MemoryTypeOptionLabels {
	all: string;
	userPref: string;
	context: string;
	knowledge: string;
	decision: string;
}

export interface MemoryTypeOption {
	value: MemoryType | '';
	label: string;
}

export interface ExportMetadata {
	exportFormat: ExportFormat;
	extension: 'json' | 'csv';
	mimeType: 'application/json' | 'text/csv';
	filterName: 'JSON' | 'CSV';
	defaultFilename: string;
}

export function truncate(text: string, maxLength: number): string {
	if (text.length <= maxLength) return text;
	return text.slice(0, maxLength) + '...';
}

export function formatDate(dateStr: string, locale?: string | string[]): string {
	const date = new Date(dateStr);
	return date.toLocaleDateString(locale, {
		year: 'numeric',
		month: 'short',
		day: 'numeric',
		hour: '2-digit',
		minute: '2-digit'
	});
}

export function getTypeVariant(type: MemoryType): BadgeVariant {
	const variants: Record<MemoryType, BadgeVariant> = {
		user_pref: 'primary',
		context: 'success',
		knowledge: 'warning',
		decision: 'error'
	};
	return variants[type] || 'primary';
}

export function formatScope(workflowId: string | undefined | null, generalLabel: string): string {
	if (!workflowId) return generalLabel;
	return workflowId.length > 12 ? workflowId.slice(0, 12) + '...' : workflowId;
}

export function buildMemoryTypeOptions(labels: MemoryTypeOptionLabels): MemoryTypeOption[] {
	return [
		{ value: '', label: labels.all },
		{ value: 'user_pref', label: labels.userPref },
		{ value: 'context', label: labels.context },
		{ value: 'knowledge', label: labels.knowledge },
		{ value: 'decision', label: labels.decision }
	];
}

export function getExportMetadata(format: 'json' | 'csv', date = new Date()): ExportMetadata {
	return {
		exportFormat: format === 'json' ? 'json' : 'csv',
		extension: format,
		mimeType: format === 'json' ? 'application/json' : 'text/csv',
		filterName: format === 'json' ? 'JSON' : 'CSV',
		defaultFilename: `memories-${date.toISOString().slice(0, 10)}.${format}`
	};
}

export function formatImportFailureMessage(
	template: string,
	failed: number,
	errors: string[]
): string {
	return template
		.replace('{count}', String(failed))
		.replace('{errors}', errors.slice(0, 3).join(', '));
}
