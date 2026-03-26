// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Import/Export Settings Commands
//!
//! Tauri commands for exporting and importing configuration entities.
//!
//! ## Export Commands
//! - `prepare_export_preview` - Get preview data for selected entities
//! - `generate_export_file` - Generate export JSON with sanitization applied
//! - `save_export_to_file` - Save export content to a file
//!
//! ## Import Commands
//! - `validate_import` - Validate import file and detect conflicts
//! - `execute_import` - Execute import with conflict resolutions

pub mod export;
mod helpers;
pub mod import;
mod import_ops;
