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
  LegalModal Component
  Displays legal notices and privacy policy with GDPR compliance.

  @example
  <LegalModal
    type="legal-notice"
    open={showLegal}
    onclose={() => showLegal = false}
  />
-->
<script lang="ts">
	import Modal from '$lib/components/ui/Modal.svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * LegalModal props
	 */
	interface Props {
		/** Type of modal: 'legal-notice' or 'privacy-policy' */
		type: 'legal-notice' | 'privacy-policy';
		/** Whether the modal is open */
		open: boolean;
		/** Close handler */
		onclose: () => void;
	}

	let { type, open, onclose }: Props = $props();

	let title = $derived(
		type === 'legal-notice' ? $i18n('legal_notice_title') : $i18n('privacy_policy_title')
	);
</script>

<Modal {open} {title} {onclose}>
	{#snippet body()}
		{#if type === 'legal-notice'}
			<div class="legal-content">
				<h3>{$i18n('legal_editor_title')}</h3>
				<p>{$i18n('legal_editor_intro')}</p>
				<ul>
					<li><strong>{$i18n('legal_company_name')}:</strong> Assistance Micro Design</li>
					<li>
						<strong>{$i18n('legal_company_type')}:</strong>
						{$i18n('legal_entrepreneur_individuel')}
					</li>
					<li><strong>{$i18n('legal_headquarters')}:</strong> Guadeloupe</li>
					<li><strong>{$i18n('legal_siret')}:</strong> 47857738000049</li>
				</ul>

				<h3>{$i18n('legal_publication_director')}</h3>
				<p>Sebastien PERRONE</p>

				<h3>{$i18n('legal_contact')}</h3>
				<p>
					<strong>Email:</strong>
					<a href="mailto:assistance-micro-design@pm.me">assistance-micro-design@pm.me</a>
				</p>
			</div>
		{:else}
			<div class="legal-content privacy-policy">
				<!-- Responsable du traitement -->
				<h3>{$i18n('privacy_controller_title')}</h3>
				<p>{$i18n('privacy_controller_content')}</p>

				<!-- Donnees collectees -->
				<h3>{$i18n('privacy_data_collected_title')}</h3>
				<p>{$i18n('privacy_data_collected_content')}</p>

				<!-- Finalites -->
				<h3>{$i18n('privacy_purpose_title')}</h3>
				<p>{$i18n('privacy_purpose_content')}</p>

				<!-- Base legale -->
				<h3>{$i18n('privacy_legal_basis_title')}</h3>
				<p>{$i18n('privacy_legal_basis_content')}</p>

				<!-- Stockage et securite -->
				<h3>{$i18n('privacy_storage_title')}</h3>
				<p>{$i18n('privacy_storage_content')}</p>

				<!-- Duree de conservation -->
				<h3>{$i18n('privacy_retention_title')}</h3>
				<p>{$i18n('privacy_retention_content')}</p>

				<!-- Destinataires -->
				<h3>{$i18n('privacy_recipients_title')}</h3>
				<p>{$i18n('privacy_recipients_content')}</p>

				<!-- Transferts hors UE -->
				<h3>{$i18n('privacy_transfers_title')}</h3>
				<p>{$i18n('privacy_transfers_content')}</p>

				<!-- Droits RGPD -->
				<h3>{$i18n('privacy_rights_title')}</h3>
				<p>{$i18n('privacy_rights_intro')}</p>
				<ul class="rights-list">
					<li>{$i18n('privacy_right_access')}</li>
					<li>{$i18n('privacy_right_rectification')}</li>
					<li>{$i18n('privacy_right_erasure')}</li>
					<li>{$i18n('privacy_right_restriction')}</li>
					<li>{$i18n('privacy_right_portability')}</li>
					<li>{$i18n('privacy_right_opposition')}</li>
				</ul>

				<!-- Decisions automatisees / IA -->
				<h3>{$i18n('privacy_automated_title')}</h3>
				<p>{$i18n('privacy_automated_content')}</p>

				<!-- Reclamation CNIL -->
				<h3>{$i18n('privacy_complaint_title')}</h3>
				<p>{$i18n('privacy_complaint_content')}</p>

				<!-- Contact -->
				<h3>{$i18n('privacy_contact_title')}</h3>
				<p>{$i18n('privacy_contact_content')}</p>

				<!-- Mise a jour -->
				<h3>{$i18n('privacy_update_title')}</h3>
				<p>{$i18n('privacy_update_content')}</p>
				<p class="last-updated">{$i18n('privacy_last_updated')}</p>
			</div>
		{/if}
	{/snippet}

	{#snippet footer()}
		<button class="btn btn-primary" onclick={onclose}>
			{$i18n('common_close')}
		</button>
	{/snippet}
</Modal>

<style>
	.legal-content {
		line-height: 1.6;
	}

	.legal-content h3 {
		margin-top: var(--spacing-lg);
		margin-bottom: var(--spacing-sm);
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.legal-content h3:first-child {
		margin-top: 0;
	}

	.legal-content ul {
		list-style: none;
		padding: 0;
		margin: var(--spacing-md) 0;
	}

	.legal-content li {
		margin-bottom: var(--spacing-sm);
	}

	.legal-content a {
		color: var(--color-accent);
		text-decoration: none;
	}

	.legal-content a:hover {
		text-decoration: underline;
	}

	/* Privacy Policy specific styles */
	.privacy-policy {
		max-height: 60vh;
		overflow-y: auto;
	}

	.rights-list {
		list-style: disc;
		padding-left: var(--spacing-xl);
	}

	.rights-list li {
		margin-bottom: var(--spacing-xs);
	}

	.last-updated {
		font-style: italic;
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm);
	}
</style>
