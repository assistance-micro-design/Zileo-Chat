use super::*;
use std::time::Duration;

#[test]
fn test_initial_state_is_closed() {
    let cb = SubAgentCircuitBreaker::with_defaults();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 0);
}

#[test]
fn test_default_impl() {
    let cb = SubAgentCircuitBreaker::default();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_threshold(), CIRCUIT_FAILURE_THRESHOLD);
    assert_eq!(cb.cooldown(), Duration::from_secs(CIRCUIT_COOLDOWN_SECS));
}

#[test]
fn test_allow_request_when_closed() {
    let mut cb = SubAgentCircuitBreaker::with_defaults();
    assert!(cb.allow_request());
}

#[test]
fn test_opens_after_threshold_failures() {
    let mut cb = SubAgentCircuitBreaker::new(3, Duration::from_secs(60));

    // First two failures - still closed
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 1);

    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 2);

    // Third failure - opens
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
    assert_eq!(cb.failure_count(), 3);
}

#[test]
fn test_rejects_when_open() {
    let mut cb = SubAgentCircuitBreaker::new(1, Duration::from_secs(60));

    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    // Should reject requests
    assert!(!cb.allow_request());
}

#[test]
fn test_transitions_to_half_open_after_cooldown() {
    let mut cb = SubAgentCircuitBreaker::new(
        1,
        Duration::from_millis(10), // Very short cooldown for testing
    );

    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    // Wait for cooldown
    std::thread::sleep(Duration::from_millis(20));

    // Should transition to half-open
    assert!(cb.allow_request());
    assert_eq!(cb.state(), CircuitState::HalfOpen);
}

#[test]
fn test_closes_on_success_after_half_open() {
    let mut cb = SubAgentCircuitBreaker::new(1, Duration::from_millis(10));

    cb.record_failure();
    std::thread::sleep(Duration::from_millis(20));
    cb.allow_request(); // Transitions to half-open

    cb.record_success();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 0);
}

#[test]
fn test_reopens_on_failure_in_half_open() {
    let mut cb = SubAgentCircuitBreaker::new(1, Duration::from_millis(10));

    cb.record_failure();
    std::thread::sleep(Duration::from_millis(20));
    cb.allow_request(); // Transitions to half-open

    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
}

#[test]
fn test_success_resets_failure_count() {
    let mut cb = SubAgentCircuitBreaker::with_defaults();

    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.failure_count(), 2);

    cb.record_success();
    assert_eq!(cb.failure_count(), 0);
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[test]
fn test_remaining_cooldown() {
    let mut cb = SubAgentCircuitBreaker::new(1, Duration::from_secs(60));

    // No cooldown when closed
    assert!(cb.remaining_cooldown().is_none());
    assert_eq!(cb.remaining_cooldown_secs(), 0);

    cb.record_failure();

    // Should have remaining cooldown when open
    let remaining = cb.remaining_cooldown();
    assert!(remaining.is_some());
    assert!(remaining.unwrap() > Duration::from_secs(50));
    assert!(cb.remaining_cooldown_secs() > 50);
}

#[test]
fn test_reset() {
    let mut cb = SubAgentCircuitBreaker::new(1, Duration::from_secs(60));

    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    cb.reset();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 0);
    assert!(cb.remaining_cooldown().is_none());
}

#[test]
fn test_time_since_last_failure() {
    let mut cb = SubAgentCircuitBreaker::with_defaults();

    // No failure yet
    assert!(cb.time_since_last_failure().is_none());

    cb.record_failure();

    // Should have a recent failure
    let elapsed = cb.time_since_last_failure();
    assert!(elapsed.is_some());
    assert!(elapsed.unwrap() < Duration::from_secs(1));
}

#[test]
fn test_circuit_state_display() {
    assert_eq!(format!("{}", CircuitState::Closed), "Closed");
    assert_eq!(format!("{}", CircuitState::Open), "Open");
    assert_eq!(format!("{}", CircuitState::HalfOpen), "HalfOpen");
}

#[test]
fn test_circuit_state_default() {
    let state = CircuitState::default();
    assert_eq!(state, CircuitState::Closed);
}

#[test]
fn test_multiple_success_after_failures() {
    let mut cb = SubAgentCircuitBreaker::with_defaults();

    // Two failures
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.failure_count(), 2);
    assert_eq!(cb.state(), CircuitState::Closed);

    // Success resets everything
    cb.record_success();
    assert_eq!(cb.failure_count(), 0);

    // Can fail again
    cb.record_failure();
    assert_eq!(cb.failure_count(), 1);
}

#[test]
fn test_custom_threshold() {
    let mut cb = SubAgentCircuitBreaker::new(5, Duration::from_secs(30));

    for _ in 0..4 {
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    // Fifth failure opens
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
    assert_eq!(cb.failure_count(), 5);
}
