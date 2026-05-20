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

//! Speech-to-text providers.
//!
//! Only the Mistral Voxtral batch endpoint is implemented for v1. The
//! module is structured to make a second provider drop-in (extract the
//! adapter trait) without rewriting the command surface.

pub mod mistral_batch;

pub use mistral_batch::transcribe_audio_core;
