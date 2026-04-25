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

//! Atomic monotonic counter for ordering blocks within a workflow.
//!
//! Replaces a previous "compute max + 1" approach that was fragile when
//! blocks were emitted from concurrent paths or when execution was interrupted
//! mid-stream. The tracker hands out contiguous, strictly increasing sequence
//! numbers and is safe to share across threads / async tasks via
//! `Arc<SequenceTracker>`.

use std::sync::atomic::{AtomicU32, Ordering};

/// Monotonic counter that allocates strictly increasing `u32` sequence
/// numbers via `allocate()`.
///
/// Internally backed by an `AtomicU32` — cheap to clone via `Arc`, safe
/// across threads. Wraps at `u32::MAX`; no realistic workflow approaches
/// that bound (~4 billion allocations).
#[derive(Debug, Default)]
pub struct SequenceTracker {
    next: AtomicU32,
}

impl SequenceTracker {
    /// Creates a tracker that will hand out sequences starting at `start`.
    pub fn new(start: u32) -> Self {
        Self {
            next: AtomicU32::new(start),
        }
    }

    /// Atomically returns the current sequence and increments the counter.
    ///
    /// `SeqCst` ordering is used so callers across threads observe a strict
    /// global order (sequence numbers are persisted and used for chronological
    /// rendering — strict ordering matters more than performance here).
    pub fn allocate(&self) -> u32 {
        self.next.fetch_add(1, Ordering::SeqCst)
    }

    /// Returns the next sequence that would be allocated, without consuming it.
    /// Mostly useful for diagnostics or assertions in tests.
    #[cfg(test)]
    pub fn peek(&self) -> u32 {
        self.next.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn allocate_returns_contiguous_sequence() {
        let t = SequenceTracker::new(0);
        let allocated: Vec<u32> = (0..5).map(|_| t.allocate()).collect();
        assert_eq!(allocated, vec![0, 1, 2, 3, 4]);
        assert_eq!(t.peek(), 5);
    }

    #[test]
    fn allocate_is_thread_safe_and_contiguous_under_concurrency() {
        let tracker = Arc::new(SequenceTracker::new(0));
        let n_threads = 8usize;
        let n_per_thread = 256usize;

        let handles: Vec<_> = (0..n_threads)
            .map(|_| {
                let t = tracker.clone();
                std::thread::spawn(move || {
                    let mut local = Vec::with_capacity(n_per_thread);
                    for _ in 0..n_per_thread {
                        local.push(t.allocate());
                    }
                    local
                })
            })
            .collect();

        let mut all: Vec<u32> = handles
            .into_iter()
            .flat_map(|h| h.join().expect("thread join"))
            .collect();
        all.sort_unstable();

        let expected: Vec<u32> = (0..(n_threads * n_per_thread) as u32).collect();
        assert_eq!(all, expected, "sequences must be unique and contiguous");
        assert_eq!(tracker.peek(), (n_threads * n_per_thread) as u32);
    }
}
