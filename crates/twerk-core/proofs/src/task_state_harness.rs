use twerk_core::task::TaskState;

// ---------------------------------------------------------------------------
// Terminal states cannot transition out
// ---------------------------------------------------------------------------

#[kani::proof]
fn task_state_completed_cannot_transition() {
    let targets = [
        TaskState::Created,
        TaskState::Pending,
        TaskState::Scheduled,
        TaskState::Running,
        TaskState::Cancelled,
        TaskState::Stopped,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Skipped,
    ];
    for target in &targets {
        assert!(
            !TaskState::Completed.can_transition_to(target),
            "Completed should not transition to any state, including {:?}",
            target
        );
    }
}

#[kani::proof]
fn task_state_cancelled_cannot_transition() {
    let targets = [
        TaskState::Created,
        TaskState::Pending,
        TaskState::Scheduled,
        TaskState::Running,
        TaskState::Cancelled,
        TaskState::Stopped,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Skipped,
    ];
    for target in &targets {
        assert!(
            !TaskState::Cancelled.can_transition_to(target),
            "Cancelled should not transition to any state, including {:?}",
            target
        );
    }
}

#[kani::proof]
fn task_state_stopped_cannot_transition() {
    let targets = [
        TaskState::Created,
        TaskState::Pending,
        TaskState::Scheduled,
        TaskState::Running,
        TaskState::Cancelled,
        TaskState::Stopped,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Skipped,
    ];
    for target in &targets {
        assert!(
            !TaskState::Stopped.can_transition_to(target),
            "Stopped should not transition to any state, including {:?}",
            target
        );
    }
}

#[kani::proof]
fn task_state_skipped_cannot_transition() {
    let targets = [
        TaskState::Created,
        TaskState::Pending,
        TaskState::Scheduled,
        TaskState::Running,
        TaskState::Cancelled,
        TaskState::Stopped,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Skipped,
    ];
    for target in &targets {
        assert!(
            !TaskState::Skipped.can_transition_to(target),
            "Skipped should not transition to any state, including {:?}",
            target
        );
    }
}

// ---------------------------------------------------------------------------
// Linear chain transitions
// ---------------------------------------------------------------------------

#[kani::proof]
fn task_state_created_can_transition_to_pending() {
    assert!(
        TaskState::Created.can_transition_to(&TaskState::Pending),
        "Created -> Pending should be valid"
    );
}

#[kani::proof]
fn task_state_pending_can_transition_to_scheduled() {
    assert!(
        TaskState::Pending.can_transition_to(&TaskState::Scheduled),
        "Pending -> Scheduled should be valid"
    );
}

#[kani::proof]
fn task_state_scheduled_can_transition_to_running() {
    assert!(
        TaskState::Scheduled.can_transition_to(&TaskState::Running),
        "Scheduled -> Running should be valid"
    );
}

// ---------------------------------------------------------------------------
// Running -> terminal transitions
// ---------------------------------------------------------------------------

#[kani::proof]
fn task_state_running_can_transition_to_completed() {
    assert!(
        TaskState::Running.can_transition_to(&TaskState::Completed),
        "Running -> Completed should be valid"
    );
}

#[kani::proof]
fn task_state_running_can_transition_to_failed() {
    assert!(
        TaskState::Running.can_transition_to(&TaskState::Failed),
        "Running -> Failed should be valid"
    );
}

#[kani::proof]
fn task_state_running_can_transition_to_cancelled() {
    assert!(
        TaskState::Running.can_transition_to(&TaskState::Cancelled),
        "Running -> Cancelled should be valid"
    );
}

#[kani::proof]
fn task_state_running_can_transition_to_stopped() {
    assert!(
        TaskState::Running.can_transition_to(&TaskState::Stopped),
        "Running -> Stopped should be valid"
    );
}

// ---------------------------------------------------------------------------
// Retry path: Failed -> Pending
// ---------------------------------------------------------------------------

#[kani::proof]
fn task_state_failed_can_transition_to_pending() {
    assert!(
        TaskState::Failed.can_transition_to(&TaskState::Pending),
        "Failed -> Pending should be valid (retry path)"
    );
}

// ---------------------------------------------------------------------------
// Active states can transition to Skipped
// ---------------------------------------------------------------------------

#[kani::proof]
fn task_state_created_can_skip() {
    assert!(
        TaskState::Created.can_transition_to(&TaskState::Skipped),
        "Created (active) -> Skipped should be valid"
    );
}

#[kani::proof]
fn task_state_pending_can_skip() {
    assert!(
        TaskState::Pending.can_transition_to(&TaskState::Skipped),
        "Pending (active) -> Skipped should be valid"
    );
}

#[kani::proof]
fn task_state_scheduled_can_skip() {
    assert!(
        TaskState::Scheduled.can_transition_to(&TaskState::Skipped),
        "Scheduled (active) -> Skipped should be valid"
    );
}

#[kani::proof]
fn task_state_running_can_skip() {
    assert!(
        TaskState::Running.can_transition_to(&TaskState::Skipped),
        "Running (active) -> Skipped should be valid"
    );
}

// ---------------------------------------------------------------------------
// is_active consistency
// ---------------------------------------------------------------------------

#[kani::proof]
fn task_state_is_active_matches_active_set() {
    let active_states = [
        TaskState::Created,
        TaskState::Pending,
        TaskState::Scheduled,
        TaskState::Running,
    ];
    for state in &active_states {
        assert!(state.is_active(), "{:?} should be active", state);
    }

    let inactive_states = [
        TaskState::Cancelled,
        TaskState::Stopped,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Skipped,
    ];
    for state in &inactive_states {
        assert!(!state.is_active(), "{:?} should not be active", state);
    }
}
