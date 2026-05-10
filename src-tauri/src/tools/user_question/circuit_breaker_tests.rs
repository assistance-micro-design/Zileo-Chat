use super::*;
use crate::tools::constants::user_question as uq_const;

fn new_cb_with_defaults() -> UserQuestionCircuitBreaker {
    UserQuestionCircuitBreaker::new(
        "test_wf".to_string(),
        uq_const::CIRCUIT_FAILURE_THRESHOLD,
        Duration::from_secs(uq_const::CIRCUIT_COOLDOWN_SECS),
    )
}

#[test]
fn test_initial_state_is_closed() {
    let cb = new_cb_with_defaults();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.timeout_count(), 0);
}

#[test]
fn test_allow_question_when_closed() {
    let mut cb = new_cb_with_defaults();
    assert!(cb.allow_question());
}

#[test]
fn test_opens_after_threshold_timeouts() {
    let mut cb = UserQuestionCircuitBreaker::new("test_wf".to_string(), 3, Duration::from_secs(60));

    // First two timeouts - still closed
    cb.record_timeout();
    assert_eq!(cb.state(), CircuitState::Closed);
    cb.record_timeout();
    assert_eq!(cb.state(), CircuitState::Closed);

    // Third timeout - opens
    cb.record_timeout();
    assert_eq!(cb.state(), CircuitState::Open);
    assert_eq!(cb.timeout_count(), 3);
}

#[test]
fn test_rejects_when_open() {
    let mut cb = UserQuestionCircuitBreaker::new("test_wf".to_string(), 1, Duration::from_secs(60));

    cb.record_timeout();
    assert_eq!(cb.state(), CircuitState::Open);

    // Should reject questions
    assert!(!cb.allow_question());
}

#[test]
fn test_transitions_to_half_open_after_cooldown() {
    let mut cb = UserQuestionCircuitBreaker::new(
        "test_wf".to_string(),
        1,
        Duration::from_millis(10), // Very short cooldown for testing
    );

    cb.record_timeout();
    assert_eq!(cb.state(), CircuitState::Open);

    // Wait for cooldown
    std::thread::sleep(Duration::from_millis(20));

    // Should transition to half-open
    assert!(cb.allow_question());
    assert_eq!(cb.state(), CircuitState::HalfOpen);
}

#[test]
fn test_closes_on_success_after_half_open() {
    let mut cb =
        UserQuestionCircuitBreaker::new("test_wf".to_string(), 1, Duration::from_millis(10));

    cb.record_timeout();
    std::thread::sleep(Duration::from_millis(20));
    cb.allow_question(); // Transitions to half-open

    cb.record_success();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.timeout_count(), 0);
}

#[test]
fn test_reopens_on_timeout_in_half_open() {
    let mut cb =
        UserQuestionCircuitBreaker::new("test_wf".to_string(), 1, Duration::from_millis(10));

    cb.record_timeout();
    std::thread::sleep(Duration::from_millis(20));
    cb.allow_question(); // Transitions to half-open

    cb.record_timeout();
    assert_eq!(cb.state(), CircuitState::Open);
}

#[test]
fn test_success_resets_timeout_count() {
    let mut cb = new_cb_with_defaults();

    cb.record_timeout();
    cb.record_timeout();
    assert_eq!(cb.timeout_count(), 2);

    cb.record_success();
    assert_eq!(cb.timeout_count(), 0);
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[test]
fn test_skip_resets_like_success() {
    let mut cb = new_cb_with_defaults();

    cb.record_timeout();
    cb.record_timeout();
    assert_eq!(cb.timeout_count(), 2);

    cb.record_skip();
    assert_eq!(cb.timeout_count(), 0);
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[test]
fn test_remaining_cooldown() {
    let mut cb = UserQuestionCircuitBreaker::new("test_wf".to_string(), 1, Duration::from_secs(60));

    // No cooldown when closed
    assert!(cb.remaining_cooldown().is_none());

    cb.record_timeout();

    // Should have remaining cooldown when open
    let remaining = cb.remaining_cooldown();
    assert!(remaining.is_some());
    assert!(remaining.unwrap() > Duration::from_secs(50));
}

#[test]
fn test_reset() {
    let mut cb = UserQuestionCircuitBreaker::new("test_wf".to_string(), 1, Duration::from_secs(60));

    cb.record_timeout();
    assert_eq!(cb.state(), CircuitState::Open);

    cb.reset();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.timeout_count(), 0);
    assert!(cb.remaining_cooldown().is_none());
}

#[test]
fn test_default_constants() {
    assert_eq!(uq_const::CIRCUIT_FAILURE_THRESHOLD, 3);
    assert_eq!(uq_const::CIRCUIT_COOLDOWN_SECS, 60);
}
