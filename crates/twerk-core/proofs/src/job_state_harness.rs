use twerk_core::job::JobState;

// ---------------------------------------------------------------------------
// Terminal states cannot transition out (except Failed/Cancelled -> Restart)
// ---------------------------------------------------------------------------

#[kani::proof]
fn job_state_completed_cannot_transition() {
    let all_states = [
        JobState::Pending,
        JobState::Scheduled,
        JobState::Running,
        JobState::Cancelled,
        JobState::Completed,
        JobState::Failed,
        JobState::Restart,
    ];
    for target in &all_states {
        assert!(
            !JobState::Completed.can_transition_to(target),
            "Completed should not be able to transition to any state"
        );
    }
}

#[kani::proof]
fn job_state_terminal_cannot_transition() {
    // Completed is a terminal state — it cannot transition to anything
    let all_states = [
        JobState::Pending,
        JobState::Scheduled,
        JobState::Running,
        JobState::Cancelled,
        JobState::Completed,
        JobState::Failed,
        JobState::Restart,
    ];

    // Completed: no transitions out at all
    for target in &all_states {
        assert!(
            !JobState::Completed.can_transition_to(target),
            "Completed should not transition to anything"
        );
    }
}

// ---------------------------------------------------------------------------
// Linear chain transitions
// ---------------------------------------------------------------------------

#[kani::proof]
fn job_state_pending_can_transition_to_scheduled() {
    assert!(
        JobState::Pending.can_transition_to(&JobState::Scheduled),
        "Pending -> Scheduled should be valid"
    );
}

#[kani::proof]
fn job_state_scheduled_can_transition_to_running() {
    assert!(
        JobState::Scheduled.can_transition_to(&JobState::Running),
        "Scheduled -> Running should be valid"
    );
}

// ---------------------------------------------------------------------------
// Running -> terminal transitions
// ---------------------------------------------------------------------------

#[kani::proof]
fn job_state_running_can_transition_to_completed() {
    assert!(
        JobState::Running.can_transition_to(&JobState::Completed),
        "Running -> Completed should be valid"
    );
}

#[kani::proof]
fn job_state_running_can_transition_to_failed() {
    assert!(
        JobState::Running.can_transition_to(&JobState::Failed),
        "Running -> Failed should be valid"
    );
}

#[kani::proof]
fn job_state_running_can_transition_to_cancelled() {
    assert!(
        JobState::Running.can_transition_to(&JobState::Cancelled),
        "Running -> Cancelled should be valid"
    );
}

// ---------------------------------------------------------------------------
// Restart transitions
// ---------------------------------------------------------------------------

#[kani::proof]
fn job_state_failed_can_transition_to_restart() {
    assert!(
        JobState::Failed.can_transition_to(&JobState::Restart),
        "Failed -> Restart should be valid"
    );
}

#[kani::proof]
fn job_state_cancelled_can_transition_to_restart() {
    assert!(
        JobState::Cancelled.can_transition_to(&JobState::Restart),
        "Cancelled -> Restart should be valid"
    );
}

#[kani::proof]
fn job_state_restart_can_transition_to_pending() {
    assert!(
        JobState::Restart.can_transition_to(&JobState::Pending),
        "Restart -> Pending should be valid"
    );
}

// ---------------------------------------------------------------------------
// Consistency: can_cancel agrees with can_transition_to(Cancelled)
// ---------------------------------------------------------------------------

#[kani::proof]
fn job_state_can_cancel_consistent() {
    let all_states = [
        JobState::Pending,
        JobState::Scheduled,
        JobState::Running,
        JobState::Cancelled,
        JobState::Completed,
        JobState::Failed,
        JobState::Restart,
    ];

    for state in &all_states {
        let can_cancel_method = state.can_cancel();
        let can_transition = state.can_transition_to(&JobState::Cancelled);
        assert_eq!(
            can_cancel_method, can_transition,
            "can_cancel() must agree with can_transition_to(Cancelled) for {:?}",
            state
        );
    }
}

// ---------------------------------------------------------------------------
// Consistency: can_restart agrees with transition to Restart
// ---------------------------------------------------------------------------

#[kani::proof]
fn job_state_can_restart_consistent() {
    let all_states = [
        JobState::Pending,
        JobState::Scheduled,
        JobState::Running,
        JobState::Cancelled,
        JobState::Completed,
        JobState::Failed,
        JobState::Restart,
    ];

    for state in &all_states {
        let can_restart_method = state.can_restart();
        let can_transition = state.can_transition_to(&JobState::Restart);
        assert_eq!(
            can_restart_method, can_transition,
            "can_restart() must agree with can_transition_to(Restart) for {:?}",
            state
        );
    }
}

// ---------------------------------------------------------------------------
// Invalid transitions are rejected
// ---------------------------------------------------------------------------

#[kani::proof]
fn job_state_pending_cannot_skip_to_running() {
    assert!(
        !JobState::Pending.can_transition_to(&JobState::Running),
        "Pending -> Running should be invalid (must go through Scheduled)"
    );
}

#[kani::proof]
fn job_state_pending_cannot_go_to_terminal() {
    assert!(
        !JobState::Pending.can_transition_to(&JobState::Completed),
        "Pending -> Completed should be invalid"
    );
}
