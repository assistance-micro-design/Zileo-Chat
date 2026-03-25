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

//! Activity monitoring for sub-agent inactivity timeout detection.
//!
//! Provides heartbeat-based monitoring that allows long-running but active
//! executions while catching genuine hangs.

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;

/// Callback type for activity notification.
///
/// This callback is invoked whenever the agent shows activity (LLM response,
/// tool call start/end, MCP response). It should be lightweight and thread-safe.
pub type ActivityCallback = Arc<dyn Fn() + Send + Sync>;

/// Monitors agent activity to detect hangs.
///
/// The ActivityMonitor tracks the timestamp of the last activity and provides
/// methods to:
/// - Record new activity (resetting the inactivity timer)
/// - Check how long since the last activity
///
/// This enables intelligent timeout detection that allows long-running but active
/// executions while catching genuine hangs (no activity for extended periods).
///
/// # Thread Safety
///
/// All operations are thread-safe via `RwLock`. The `record_activity()` method
/// uses `try_write()` to avoid blocking if a read is in progress.
///
/// # Example
///
/// ```ignore
/// let monitor = ActivityMonitor::new();
///
/// // In the execution loop:
/// monitor.record_activity(); // Called on each LLM token, tool call, etc.
///
/// // In the monitoring loop:
/// if monitor.seconds_since_last_activity() > INACTIVITY_TIMEOUT_SECS {
///     // Abort - agent is hung
/// }
/// ```
#[derive(Clone)]
pub struct ActivityMonitor {
    /// Timestamp of the last recorded activity
    last_activity: Arc<RwLock<Instant>>,
}

impl ActivityMonitor {
    /// Creates a new ActivityMonitor with the current time as initial activity.
    pub fn new() -> Self {
        Self {
            last_activity: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Records a new activity, resetting the inactivity timer.
    ///
    /// This should be called whenever the agent shows signs of progress:
    /// - LLM returns tokens
    /// - Tool call starts
    /// - Tool call completes
    /// - MCP server responds
    ///
    /// Uses `try_write()` to avoid blocking. If the lock is held, the activity
    /// is skipped (this is acceptable as another activity will be recorded soon).
    pub fn record_activity(&self) {
        if let Ok(mut last) = self.last_activity.try_write() {
            *last = Instant::now();
        }
        // If try_write fails, another thread is writing - that's fine,
        // activity is being recorded anyway
    }

    /// Returns the number of seconds since the last recorded activity.
    ///
    /// Returns 0 if the lock cannot be acquired (conservative - assume active).
    pub fn seconds_since_last_activity(&self) -> u64 {
        self.last_activity
            .try_read()
            .map(|last| last.elapsed().as_secs())
            .unwrap_or(0)
    }

    /// Creates a callback closure that records activity when called.
    ///
    /// This callback can be passed to the orchestrator/agent for automatic
    /// activity tracking during execution.
    pub fn create_callback(self: &Arc<Self>) -> ActivityCallback {
        let monitor = Arc::clone(self);
        Arc::new(move || {
            monitor.record_activity();
        })
    }
}

impl Default for ActivityMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_monitor_new() {
        let monitor = ActivityMonitor::new();
        // Should start with recent activity (just created)
        assert!(monitor.seconds_since_last_activity() < 2);
    }

    #[test]
    fn test_activity_monitor_default() {
        let monitor = ActivityMonitor::default();
        assert!(monitor.seconds_since_last_activity() < 2);
    }

    #[test]
    fn test_activity_monitor_record_activity() {
        let monitor = ActivityMonitor::new();

        // Small delay to ensure time has passed
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Record new activity
        monitor.record_activity();

        // Should be very recent
        assert!(monitor.seconds_since_last_activity() < 1);
    }

    #[test]
    fn test_activity_monitor_clone() {
        let monitor = ActivityMonitor::new();
        let cloned = monitor.clone();

        // Both should show same initial time
        let time1 = monitor.seconds_since_last_activity();
        let time2 = cloned.seconds_since_last_activity();

        // Due to Arc, they should be identical (pointing to same data)
        assert_eq!(time1, time2);

        // Recording on one should affect the other (shared state)
        std::thread::sleep(std::time::Duration::from_millis(50));
        monitor.record_activity();

        // Both should now show recent activity
        assert!(cloned.seconds_since_last_activity() < 1);
    }

    #[test]
    fn test_activity_monitor_callback() {
        let monitor = Arc::new(ActivityMonitor::new());

        // Wait a bit
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Create callback and invoke it
        let callback = monitor.create_callback();
        callback();

        // Activity should be recorded
        assert!(monitor.seconds_since_last_activity() < 1);
    }
}
