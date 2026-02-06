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
Settings Index Page - Redirects to default section using meta refresh
-->

<svelte:head>
	<meta http-equiv="refresh" content="0; url=/settings/providers" />
</svelte:head>

<div class="redirect-container">
	<p>Redirecting to providers...</p>
	<a href="/settings/providers">Click here if not redirected</a>
</div>

<style>
	.redirect-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		gap: var(--spacing-md);
		color: var(--color-text-secondary);
	}

	a {
		color: var(--color-accent);
		text-decoration: underline;
	}
</style>
