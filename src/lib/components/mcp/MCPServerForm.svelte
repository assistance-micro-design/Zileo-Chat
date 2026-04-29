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
  MCPServerForm Component (v1.2 - HTTP auth)

  Form for creating and editing MCP server configurations. For HTTP
  deployments, exposes the new authentication fields (Bearer / API Key /
  Basic) and an "Extra HTTP headers" editor; secrets are sent to the
  backend through `MCPServerConfigWithSecret.authSecret` and persisted in
  the OS keychain — never returned on read.

  @example
  <MCPServerForm
    mode="create"
    onsave={handleSave}
    oncancel={handleCancel}
  />

  <MCPServerForm
    mode="edit"
    server={existingServer}
    onsave={handleSave}
    oncancel={handleCancel}
  />
-->
<script lang="ts">
	import type {
		MCPAuthMetadata,
		MCPAuthSecret,
		MCPAuthType,
		MCPDeploymentMethod,
		MCPServerConfig,
		MCPServerConfigWithSecret
	} from '$types/mcp';
	import { Button, HelpButton, Input, PasswordInput, Select, Textarea } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import { Plus, X } from '@lucide/svelte';
	import { i18n, t } from '$lib/i18n';
	import {
		MAX_EXTRA_HEADERS,
		validateApiKeyHeaderName,
		validateApiKeyValue,
		validateBasicAuth,
		validateBearerToken,
		validateExtraHeaders
	} from '$lib/utils/mcp-auth-validation';

	/**
	 * MCPServerForm props.
	 */
	interface Props {
		/** Form mode: create or edit */
		mode: 'create' | 'edit';
		/** Existing server data for edit mode */
		server?: MCPServerConfig;
		/** Handler when form is saved */
		onsave: (config: MCPServerConfigWithSecret) => void;
		/** Handler when form is cancelled */
		oncancel: () => void;
		/** Whether save is in progress */
		saving?: boolean;
	}

	let {
		mode,
		server,
		onsave,
		oncancel,
		saving = false
	}: Props = $props();

	/**
	 * Generates a unique ID for new servers.
	 */
	function generateId(): string {
		return `mcp-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
	}

	/** Form state initialised from props. */
	let formData = $state<{
		id: string;
		name: string;
		enabled: boolean;
		command: MCPDeploymentMethod;
		args: string;
		env: Array<{ key: string; value: string }>;
		description: string;
		authType: MCPAuthType;
		bearerToken: string;
		apiKeyHeaderName: string;
		apiKeyValue: string;
		basicUser: string;
		basicPass: string;
		extraHeaders: Array<{ key: string; value: string }>;
	}>({
		id: generateId(),
		name: '',
		enabled: true,
		command: 'docker',
		args: '',
		env: [],
		description: '',
		authType: 'none',
		bearerToken: '',
		apiKeyHeaderName: '',
		apiKeyValue: '',
		basicUser: '',
		basicPass: '',
		extraHeaders: []
	});

	/** Validation errors state (each entry is an i18n key or already-resolved msg). */
	let errors = $state<{
		name?: string;
		args?: string;
		env?: string;
		authBearer?: string;
		authApiKeyHeader?: string;
		authApiKeyValue?: string;
		authBasic?: string;
		extraHeaders?: string;
	}>({});

	/** Whether the user has dismissed the legacy migration banner for this session. */
	let migrationDismissed = $state(false);

	/** Sync form data when server prop changes (switching between edit targets). */
	$effect(() => {
		const initialAuthType = server?.authType ?? 'none';
		const meta = server?.authMetadata;
		formData = {
			id: server?.id ?? generateId(),
			name: server?.name ?? '',
			enabled: server?.enabled ?? true,
			command: server?.command ?? 'docker',
			args: server?.args?.join('\n') ?? '',
			env: server?.env
				? Object.entries(server.env).map(([key, value]) => ({ key, value }))
				: [],
			description: server?.description ?? '',
			authType: initialAuthType,
			bearerToken: '',
			apiKeyHeaderName: meta?.headerName ?? '',
			apiKeyValue: '',
			basicUser: meta?.username ?? '',
			basicPass: '',
			extraHeaders: server?.extraHeaders
				? Object.entries(server.extraHeaders).map(([key, value]) => ({ key, value }))
				: []
		};
		errors = {};
		migrationDismissed = false;
	});

	/** True when the deployment method is HTTP. */
	const isHttp = $derived(formData.command === 'http');

	/** True when an authentication method other than `none` is selected. */
	const authActive = $derived(formData.authType !== 'none');

	/**
	 * In create mode, a secret is mandatory whenever auth is active. In edit
	 * mode, it is optional (empty -> keep the existing keychain value).
	 */
	const secretRequired = $derived(mode === 'create' && authActive);

	/** Placeholder used for secret fields in edit mode. */
	const secretPlaceholder = $derived(
		secretRequired
			? $i18n('mcp_auth_secret_placeholder_required')
			: $i18n('mcp_auth_secret_placeholder_kept')
	);

	/** Returns the value of the legacy `API_KEY` env var, or null. */
	const legacyApiKeyValue = $derived.by((): string | null => {
		if (!isHttp) return null;
		if (formData.authType !== 'none') return null;
		const entry = formData.env.find((e) => e.key.trim() === 'API_KEY');
		return entry && entry.value ? entry.value : null;
	});

	/**
	 * Returns the legacy `HEADER_*` env entries (without the `HEADER_` prefix
	 * stripped — kept verbatim for the migration helper). Mirrors the
	 * backend `detect_legacy_http_auth_keys` so the in-form banner stays in
	 * sync with the page-level one.
	 */
	const legacyHeaderEntries = $derived.by((): Array<{ key: string; value: string }> => {
		if (!isHttp) return [];
		if (formData.authType !== 'none') return [];
		return formData.env.filter(
			(e) => e.key.trim().startsWith('HEADER_') && e.value.length > 0
		);
	});

	/** Whether the legacy migration banner should be shown right now. */
	const showLegacyMigration = $derived(
		!migrationDismissed
			&& (legacyApiKeyValue !== null || legacyHeaderEntries.length > 0)
	);

	/** Whether the current Basic auth selection runs over plain HTTP. */
	const basicOverPlainHttp = $derived.by(() => {
		if (formData.authType !== 'basic') return false;
		const url = formData.args.split('\n').find((l) => l.trim().length > 0);
		return Boolean(url && /^http:\/\//i.test(url.trim()));
	});

	/** Command options for select - reactive to locale changes. */
	const commandOptions: SelectOption[] = $derived([
		{ value: 'docker', label: t('mcp_form_deployment_docker') },
		{ value: 'npx', label: t('mcp_form_deployment_npx') },
		{ value: 'uvx', label: t('mcp_form_deployment_uvx') },
		{ value: 'http', label: t('mcp_form_deployment_http') }
	]);

	/** Auth method options. */
	const authOptions: SelectOption[] = $derived([
		{ value: 'none', label: t('mcp_auth_method_none') },
		{ value: 'bearer', label: t('mcp_auth_method_bearer') },
		{ value: 'apikey', label: t('mcp_auth_method_apikey') },
		{ value: 'basic', label: t('mcp_auth_method_basic') }
	]);

	/**
	 * Builds the extra-headers map from the form rows, dropping rows whose
	 * key is empty (used both for validation and for submit).
	 */
	function buildExtraHeadersMap(): Record<string, string> {
		const map: Record<string, string> = {};
		for (const row of formData.extraHeaders) {
			const k = row.key.trim();
			if (!k) continue;
			map[k] = row.value;
		}
		return map;
	}

	/**
	 * Validates the form. Populates `errors` and returns true on success.
	 */
	function validate(): boolean {
		const newErrors: typeof errors = {};

		// Name validation
		if (!formData.name.trim()) {
			newErrors.name = t('mcp_form_name_required');
		} else if (!/^[a-zA-Z0-9_-]+$/.test(formData.name)) {
			newErrors.name = t('mcp_form_name_format');
		} else if (formData.name.length > 64) {
			newErrors.name = t('mcp_form_name_length');
		}

		// Args validation
		if (!formData.args.trim()) {
			if (formData.command === 'http') {
				newErrors.args = t('mcp_form_args_url_required');
			} else if (formData.command !== 'docker') {
				newErrors.args = t('mcp_form_args_required');
			}
		} else if (formData.command === 'http') {
			const url = formData.args.trim().split('\n')[0];
			if (!/^https?:\/\/.+/.test(url)) {
				newErrors.args = t('mcp_form_args_invalid_url');
			}
		}

		// Env vars: duplicate keys
		const envKeys = formData.env.map((e) => e.key).filter((k) => k.trim());
		const uniqueKeys = new Set(envKeys);
		if (envKeys.length !== uniqueKeys.size) {
			newErrors.env = t('mcp_form_env_duplicate');
		}

		// Auth validation (HTTP only)
		if (isHttp && formData.authType === 'bearer') {
			const needSecret = secretRequired || formData.bearerToken.length > 0;
			if (needSecret) {
				const r = validateBearerToken(formData.bearerToken);
				if (!r.valid && r.error) newErrors.authBearer = t(r.error);
			}
		}
		if (isHttp && formData.authType === 'apikey') {
			const r1 = validateApiKeyHeaderName(formData.apiKeyHeaderName);
			if (!r1.valid && r1.error) newErrors.authApiKeyHeader = t(r1.error);

			const needSecret = secretRequired || formData.apiKeyValue.length > 0;
			if (needSecret) {
				const r2 = validateApiKeyValue(formData.apiKeyValue);
				if (!r2.valid && r2.error) newErrors.authApiKeyValue = t(r2.error);
			}
		}
		if (isHttp && formData.authType === 'basic') {
			const needSecret = secretRequired || formData.basicPass.length > 0;
			if (needSecret) {
				const r = validateBasicAuth(formData.basicUser, formData.basicPass);
				if (!r.valid && r.error) newErrors.authBasic = t(r.error);
			} else if (!formData.basicUser.trim()) {
				newErrors.authBasic = t('mcp_auth_invalid_basic');
			}
		}

		// Extra headers
		if (isHttp) {
			const headers = buildExtraHeadersMap();
			const r = validateExtraHeaders(headers, authActive);
			if (!r.valid && r.error) newErrors.extraHeaders = t(r.error);
		}

		errors = newErrors;
		return Object.keys(newErrors).length === 0;
	}

	/**
	 * Builds the optional `authMetadata` payload from the form state.
	 */
	function buildAuthMetadata(): MCPAuthMetadata | undefined {
		switch (formData.authType) {
			case 'apikey': {
				const headerName = formData.apiKeyHeaderName.trim() || 'X-API-Key';
				return { headerName };
			}
			case 'basic':
				return { username: formData.basicUser.trim() };
			default:
				return undefined;
		}
	}

	/**
	 * Builds the optional `authSecret` payload, respecting the edit-mode
	 * "leave blank to keep" semantics.
	 */
	function buildAuthSecret(): MCPAuthSecret | undefined {
		switch (formData.authType) {
			case 'bearer':
				return formData.bearerToken
					? { token: formData.bearerToken }
					: undefined;
			case 'apikey':
				return formData.apiKeyValue
					? { value: formData.apiKeyValue }
					: undefined;
			case 'basic':
				return formData.basicPass
					? { password: formData.basicPass }
					: undefined;
			default:
				return undefined;
		}
	}

	/**
	 * Handles form submission.
	 */
	function handleSubmit(event: Event): void {
		event.preventDefault();
		if (!validate()) return;

		const args = formData.args
			.split('\n')
			.map((arg) => arg.trim())
			.filter((arg) => arg.length > 0);

		const env: Record<string, string> = {};
		for (const item of formData.env) {
			if (item.key.trim()) env[item.key.trim()] = item.value;
		}

		const extraHeadersMap = isHttp ? buildExtraHeadersMap() : {};

		const config: MCPServerConfigWithSecret = {
			id: formData.id,
			name: formData.name.trim(),
			enabled: formData.enabled,
			command: formData.command,
			args,
			env,
			description: formData.description.trim() || undefined,
			authType: isHttp ? formData.authType : undefined,
			authMetadata: isHttp ? buildAuthMetadata() : undefined,
			extraHeaders:
				isHttp && Object.keys(extraHeadersMap).length > 0
					? extraHeadersMap
					: undefined,
			authSecret: isHttp ? buildAuthSecret() : undefined
		};

		onsave(config);
	}

	function addEnvVar(): void {
		formData.env = [...formData.env, { key: '', value: '' }];
	}

	function removeEnvVar(index: number): void {
		formData.env = formData.env.filter((_, i) => i !== index);
	}

	function addExtraHeader(): void {
		if (formData.extraHeaders.length >= MAX_EXTRA_HEADERS) return;
		formData.extraHeaders = [...formData.extraHeaders, { key: '', value: '' }];
	}

	function removeExtraHeader(index: number): void {
		formData.extraHeaders = formData.extraHeaders.filter((_, i) => i !== index);
	}

	function handleCommandChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		formData.command = event.currentTarget.value as MCPDeploymentMethod;
		// Reset auth-related fields when leaving HTTP
		if (formData.command !== 'http') {
			formData.authType = 'none';
		}
	}

	function handleAuthTypeChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		formData.authType = event.currentTarget.value as MCPAuthType;
	}

	/**
	 * Moves any legacy `HEADER_*` env entries into the typed `extraHeaders`
	 * editor, stripping the prefix. Used by every migration action so the
	 * legacy env-var slate is fully cleared in one click. Existing entries
	 * with the same name are not overwritten (user-edited values win).
	 */
	function migrateLegacyHeadersIntoExtra(): void {
		if (legacyHeaderEntries.length === 0) return;
		const existing = new Set(
			formData.extraHeaders.map((h) => h.key.trim()).filter((k) => k.length > 0)
		);
		const moved = legacyHeaderEntries
			.map((e) => ({
				key: e.key.trim().slice('HEADER_'.length),
				value: e.value
			}))
			.filter((h) => h.key.length > 0 && !existing.has(h.key));
		if (moved.length > 0) {
			formData.extraHeaders = [...formData.extraHeaders, ...moved];
		}
		formData.env = formData.env.filter((e) => !e.key.trim().startsWith('HEADER_'));
	}

	/**
	 * Migration helper actions: pre-fill the new auth fields from the legacy
	 * `API_KEY` env var, move any `HEADER_*` entries to the extra-headers
	 * editor, then drop the legacy entries.
	 */
	function migrateLegacyAsBearer(): void {
		const value = legacyApiKeyValue;
		if (!value && legacyHeaderEntries.length === 0) return;
		if (value) {
			formData.authType = 'bearer';
			formData.bearerToken = value;
		}
		migrateLegacyHeadersIntoExtra();
		formData.env = formData.env.filter((e) => e.key.trim() !== 'API_KEY');
		migrationDismissed = true;
	}

	function migrateLegacyAsApiKey(): void {
		const value = legacyApiKeyValue;
		if (!value && legacyHeaderEntries.length === 0) return;
		if (value) {
			formData.authType = 'apikey';
			formData.apiKeyHeaderName = 'X-API-Key';
			formData.apiKeyValue = value;
		}
		migrateLegacyHeadersIntoExtra();
		formData.env = formData.env.filter((e) => e.key.trim() !== 'API_KEY');
		migrationDismissed = true;
	}

	/**
	 * For configs that only have `HEADER_*` entries (no `API_KEY`): move the
	 * headers into the extra-headers editor without touching `authType`.
	 */
	function migrateLegacyHeadersOnly(): void {
		if (legacyHeaderEntries.length === 0) return;
		migrateLegacyHeadersIntoExtra();
		migrationDismissed = true;
	}

	function dismissLegacyMigration(): void {
		migrationDismissed = true;
	}
</script>

<form class="mcp-form" onsubmit={handleSubmit}>
	<div class="form-section">
		<Input
			label={$i18n('mcp_form_name_label')}
			value={formData.name}
			oninput={(e) => { formData.name = e.currentTarget.value; }}
			placeholder={$i18n('mcp_form_name_placeholder')}
			required
			help={errors.name ?? $i18n('mcp_form_name_help')}
		/>
		{#if errors.name}
			<span class="error-text">{errors.name}</span>
		{/if}
	</div>

	<div class="form-section">
		<Select
			label={$i18n('mcp_form_deployment_label')}
			options={commandOptions}
			value={formData.command}
			onchange={handleCommandChange}
			required
			help={$i18n('mcp_form_deployment_help')}
		/>
	</div>

	<div class="form-section">
		<Textarea
			label={$i18n('mcp_form_args_label')}
			value={formData.args}
			oninput={(e) => { formData.args = e.currentTarget.value; }}
			placeholder={$i18n('mcp_form_args_placeholder')}
			rows={4}
			help={errors.args ?? $i18n('mcp_form_args_help')}
		/>
		{#if errors.args}
			<span class="error-text">{errors.args}</span>
		{/if}
	</div>

	{#if isHttp}
		<div class="form-section auth-section">
			<div class="section-title-row">
				<h3 class="section-title">{$i18n('mcp_auth_section_title')}</h3>
				<HelpButton
					titleKey="mcp_auth_help_title"
					descriptionKey="mcp_auth_help_description"
					tutorialKey="mcp_auth_help_tutorial"
				/>
			</div>

			{#if showLegacyMigration}
				<div class="legacy-banner" role="region" aria-label={$i18n('mcp_auth_legacy_migration_title')}>
					<div class="legacy-banner-text">
						<strong>{$i18n('mcp_auth_legacy_migration_title')}</strong>
						<p>{$i18n('mcp_auth_legacy_migration_body')}</p>
					</div>
					<div class="legacy-banner-actions">
						{#if legacyApiKeyValue !== null}
							<Button
								type="button"
								variant="primary"
								size="sm"
								onclick={migrateLegacyAsBearer}
							>
								{$i18n('mcp_auth_legacy_migration_use_bearer')}
							</Button>
							<Button
								type="button"
								variant="secondary"
								size="sm"
								onclick={migrateLegacyAsApiKey}
							>
								{$i18n('mcp_auth_legacy_migration_use_apikey')}
							</Button>
						{:else}
							<Button
								type="button"
								variant="primary"
								size="sm"
								onclick={migrateLegacyHeadersOnly}
							>
								{$i18n('mcp_auth_legacy_migration_move_headers')}
							</Button>
						{/if}
						<Button
							type="button"
							variant="ghost"
							size="sm"
							onclick={dismissLegacyMigration}
						>
							{$i18n('mcp_auth_legacy_migration_dismiss')}
						</Button>
					</div>
				</div>
			{/if}

			<Select
				label={$i18n('mcp_auth_method_label')}
				options={authOptions}
				value={formData.authType}
				onchange={handleAuthTypeChange}
				help={$i18n('mcp_auth_method_help')}
			/>

			{#if formData.authType === 'bearer'}
				<PasswordInput
					label={$i18n('mcp_auth_bearer_token_label')}
					bind:value={formData.bearerToken}
					placeholder={secretPlaceholder}
					help={errors.authBearer ?? $i18n('mcp_auth_bearer_token_help')}
					error={errors.authBearer}
					required={secretRequired}
				/>
			{:else if formData.authType === 'apikey'}
				<Input
					label={$i18n('mcp_auth_apikey_header_label')}
					value={formData.apiKeyHeaderName}
					oninput={(e) => { formData.apiKeyHeaderName = e.currentTarget.value; }}
					placeholder="X-API-Key"
					help={errors.authApiKeyHeader ?? $i18n('mcp_auth_apikey_header_help')}
				/>
				{#if errors.authApiKeyHeader}
					<span class="error-text">{errors.authApiKeyHeader}</span>
				{/if}

				<PasswordInput
					label={$i18n('mcp_auth_apikey_value_label')}
					bind:value={formData.apiKeyValue}
					placeholder={secretPlaceholder}
					help={errors.authApiKeyValue ?? $i18n('mcp_auth_bearer_token_help')}
					error={errors.authApiKeyValue}
					required={secretRequired}
				/>
			{:else if formData.authType === 'basic'}
				<Input
					label={$i18n('mcp_auth_basic_user_label')}
					value={formData.basicUser}
					oninput={(e) => { formData.basicUser = e.currentTarget.value; }}
					placeholder=""
					help={errors.authBasic}
				/>
				<PasswordInput
					label={$i18n('mcp_auth_basic_pass_label')}
					bind:value={formData.basicPass}
					placeholder={secretPlaceholder}
					help={errors.authBasic ?? $i18n('mcp_auth_bearer_token_help')}
					error={errors.authBasic}
					required={secretRequired}
				/>
				{#if basicOverPlainHttp}
					<span class="warning-text">{$i18n('mcp_auth_basic_http_warning')}</span>
				{/if}
			{/if}
		</div>

		<div class="form-section auth-section">
			<div class="env-header">
				<span class="env-label">{$i18n('mcp_auth_extra_headers_title')}</span>
				<Button
					type="button"
					variant="ghost"
					size="sm"
					onclick={addExtraHeader}
					ariaLabel={$i18n('mcp_auth_extra_headers_add')}
					disabled={formData.extraHeaders.length >= MAX_EXTRA_HEADERS}
				>
					<Plus size={16} />
					<span>{$i18n('mcp_auth_extra_headers_add')}</span>
				</Button>
			</div>

			{#if formData.extraHeaders.length > 0}
				<div class="env-list">
					{#each formData.extraHeaders as header, index (`${index}-${header.key}`)}
						<div class="env-row">
							<input
								type="text"
								class="env-input env-key"
								value={header.key}
								oninput={(e) => { formData.extraHeaders[index].key = e.currentTarget.value; }}
								placeholder={$i18n('mcp_auth_extra_headers_key_placeholder')}
								aria-label={$i18n('mcp_auth_extra_headers_key_placeholder')}
							/>
							<span class="env-equals">:</span>
							<input
								type="text"
								class="env-input env-value"
								value={header.value}
								oninput={(e) => { formData.extraHeaders[index].value = e.currentTarget.value; }}
								placeholder={$i18n('mcp_auth_extra_headers_value_placeholder')}
								aria-label={$i18n('mcp_auth_extra_headers_value_placeholder')}
							/>
							<Button
								type="button"
								variant="ghost"
								size="icon"
								onclick={() => removeExtraHeader(index)}
								ariaLabel={$i18n('mcp_auth_extra_headers_remove')}
							>
								<X size={16} />
							</Button>
						</div>
					{/each}
				</div>
			{/if}
			{#if errors.extraHeaders}
				<span class="error-text">{errors.extraHeaders}</span>
			{/if}
		</div>
	{/if}

	<div class="form-section" class:section-disabled={isHttp}>
		<div class="env-header">
			<span class="env-label" id="env-vars-label">{$i18n('mcp_form_env_label')}</span>
			<Button
				type="button"
				variant="ghost"
				size="sm"
				onclick={addEnvVar}
				ariaLabel={$i18n('mcp_form_env_add')}
				disabled={isHttp}
			>
				<Plus size={16} />
				<span>{$i18n('mcp_form_env_add')}</span>
			</Button>
		</div>

		{#if isHttp}
			<p class="env-disabled-banner">{$i18n('mcp_auth_env_disabled_in_http')}</p>
		{/if}

		{#if formData.env.length === 0}
			<p class="env-empty">{$i18n('mcp_form_env_empty')}</p>
		{:else}
			<div class="env-list">
				{#each formData.env as envVar, index (`${index}-${envVar.key}`)}
					<div class="env-row">
						<input
							type="text"
							class="env-input env-key"
							value={envVar.key}
							oninput={(e) => { formData.env[index].key = e.currentTarget.value; }}
							placeholder={$i18n('mcp_form_env_key_placeholder')}
							aria-label={$i18n('mcp_form_env_key_arialabel')}
							disabled={isHttp}
						/>
						<span class="env-equals">=</span>
						<input
							type="text"
							class="env-input env-value"
							value={envVar.value}
							oninput={(e) => { formData.env[index].value = e.currentTarget.value; }}
							placeholder={$i18n('mcp_form_env_value_placeholder')}
							aria-label={$i18n('mcp_form_env_value_arialabel')}
							disabled={isHttp}
						/>
						<Button
							type="button"
							variant="ghost"
							size="icon"
							onclick={() => removeEnvVar(index)}
							ariaLabel={$i18n('mcp_form_env_remove_arialabel')}
							disabled={isHttp}
						>
							<X size={16} />
						</Button>
					</div>
				{/each}
			</div>
		{/if}
		{#if errors.env}
			<span class="error-text">{errors.env}</span>
		{/if}
	</div>

	<div class="form-section">
		<Textarea
			label={$i18n('mcp_form_description_label')}
			value={formData.description}
			oninput={(e) => { formData.description = e.currentTarget.value; }}
			placeholder={$i18n('mcp_form_description_placeholder')}
			rows={2}
			help={$i18n('mcp_form_description_help')}
		/>
	</div>

	<div class="form-section">
		<div class="checkbox-wrapper">
			<input
				type="checkbox"
				id="mcp-server-enabled"
				checked={formData.enabled}
				onchange={(e) => { formData.enabled = e.currentTarget.checked; }}
			/>
			<label for="mcp-server-enabled" class="checkbox-label">
				{$i18n('mcp_form_enabled_label')}
			</label>
		</div>
	</div>

	<div class="form-actions">
		<Button
			type="button"
			variant="ghost"
			onclick={oncancel}
			disabled={saving}
		>
			{$i18n('common_cancel')}
		</Button>
		<Button
			type="submit"
			variant="primary"
			disabled={saving}
		>
			{#if saving}
				{mode === 'create' ? $i18n('mcp_form_creating') : $i18n('mcp_form_saving')}
			{:else}
				{mode === 'create' ? $i18n('mcp_form_create_server') : $i18n('mcp_form_save_changes')}
			{/if}
		</Button>
	</div>
</form>

<style>
	.mcp-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.form-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.auth-section {
		padding: var(--spacing-md);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		background: var(--color-bg-secondary);
		gap: var(--spacing-md);
	}

	.section-title-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.section-title {
		margin: 0;
		font-size: var(--font-size-md);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.legacy-banner {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		background: var(--color-warning-bg);
		border: 1px solid var(--color-warning);
		border-radius: var(--border-radius-sm);
	}

	.legacy-banner-text strong {
		display: block;
		color: var(--color-warning);
		margin-bottom: var(--spacing-xs);
	}

	.legacy-banner-text p {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
	}

	.legacy-banner-actions {
		display: flex;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}

	.section-disabled {
		opacity: 0.6;
	}

	.env-disabled-banner {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
		margin: 0;
		padding: var(--spacing-sm);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-sm);
	}

	.env-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.env-input {
		padding: var(--spacing-sm) var(--spacing-md);
		font-size: var(--font-size-sm);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-sm);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		outline: none;
		transition: border-color 0.2s;
	}

	.env-input:focus {
		border-color: var(--color-primary);
	}

	.env-input:disabled {
		cursor: not-allowed;
		background: var(--color-bg-secondary);
	}

	.env-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.env-header :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.env-empty {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
		padding: var(--spacing-md);
		text-align: center;
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
	}

	.env-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.env-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.env-key {
		flex: 1;
		max-width: 180px;
		font-family: var(--font-mono);
	}

	.env-equals {
		color: var(--color-text-secondary);
		font-family: var(--font-mono);
	}

	.env-value {
		flex: 2;
		font-family: var(--font-mono);
	}

	.env-row :global(button:last-child) {
		flex-shrink: 0;
	}

	.checkbox-wrapper {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.checkbox-wrapper input[type='checkbox'] {
		width: 18px;
		height: 18px;
		accent-color: var(--color-accent);
		cursor: pointer;
	}

	.checkbox-label {
		cursor: pointer;
		font-size: var(--font-size-sm);
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-md);
		padding-top: var(--spacing-md);
		border-top: 1px solid var(--color-border);
	}

	.error-text {
		font-size: var(--font-size-sm);
		color: var(--color-error);
	}

	.warning-text {
		font-size: var(--font-size-sm);
		color: var(--color-warning);
	}
</style>
