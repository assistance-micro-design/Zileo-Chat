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
 * @fileoverview Typed Tauri API clients, by domain.
 *
 * UI code (components, routes, stores, services) must call these functions
 * instead of invoking Tauri command names directly via `tauriInvoke`.
 *
 * @module lib/api
 */

export * from './agents.api';
export * from './workflows.api';
export * from './kanban.api';
export * from './scheduler.api';
export * from './memory.api';
export * from './mcp.api';
export * from './settings.api';
