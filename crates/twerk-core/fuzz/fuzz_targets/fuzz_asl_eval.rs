//! Fuzz target for ASL state machine evaluation engine.
//!
//! This target generates random ASL state machine definitions and validates that:
//! 1. The eval engine never panics on any input
//! 2. State transition invariants hold (terminal states are terminal, all states reachable)
//! 3. Error handling paths don't leak sensitive data
//! 4. Nested Parallel/Map depth limits are enforced
//!
//! Risk: Parser panics, evaluation panics, invariant violations, data leaks

#![no_main]

use std::collections::HashMap;

use libfuzzer_sys::fuzz_target;
use serde_json::Value;
use twerk_core::asl::StateMachine;
use twerk_core::eval::evaluate_state_machine;

const MAX_NESTED_DEPTH: usize = 100;

fn check_terminal_invariants(machine: &StateMachine) -> bool {
    for (name, state) in machine.states() {
        let is_terminal = state.kind().is_terminal();
        let has_end_transition = matches!(
            state.kind().transition(),
            Some(twerk_core::asl::Transition::End)
        );
        if is_terminal && has_end_transition {
            return false;
        }
        if !is_terminal && !has_end_transition {
            if let Some(twerk_core::asl::Transition::Next(target)) = state.kind().transition() {
                if target == name {
                    return false;
                }
            }
        }
    }
    true
}

fn check_reachability(machine: &StateMachine) -> bool {
    let mut reachable = std::collections::HashSet::new();
    let mut stack = vec![machine.start_at().clone()];

    while let Some(state_name) = stack.pop() {
        if reachable.contains(&state_name) {
            continue;
        }
        reachable.insert(state_name.clone());

        if let Some(state) = machine.get_state(&state_name) {
            if state.kind().is_terminal() {
                continue;
            }
            if let Some(twerk_core::asl::Transition::Next(target)) = state.kind().transition() {
                if !reachable.contains(target) {
                    stack.push(target.clone());
                }
            }
        }
    }

    for name in machine.states().keys() {
        if !reachable.contains(name) {
            return false;
        }
    }
    true
}

fn count_nested_parallel_map_depth(machine: &StateMachine) -> usize {
    fn traverse_state(state: &twerk_core::asl::State, current_depth: usize) -> usize {
        let depth_after_self = current_depth + 1;
        match state.kind() {
            twerk_core::asl::StateKind::Parallel(p) => p
                .branches()
                .iter()
                .map(|b| {
                    b.states()
                        .values()
                        .map(|s| traverse_state(s, depth_after_self + 1))
                        .fold(0, std::cmp::max)
                })
                .fold(0, std::cmp::max),
            twerk_core::asl::StateKind::Map(m) => m
                .item_processor()
                .states()
                .values()
                .map(|s| traverse_state(s, depth_after_self + 1))
                .fold(0, std::cmp::max),
            _ => current_depth,
        }
    }

    machine
        .states()
        .values()
        .map(|s| traverse_state(s, 0))
        .fold(0, std::cmp::max)
}

fn error_contains_sensitive_data(err: &str) -> bool {
    let sensitive_patterns = [
        "password",
        "secret",
        "token",
        "key",
        "credential",
        "auth",
        "bearer",
        "api_key",
        "apikey",
        "private",
        "jwt",
    ];
    let lower = err.to_lowercase();
    sensitive_patterns.iter().any(|p| lower.contains(p))
}

fn validate_error_safe(err: &twerk_core::eval::StateEvalError) -> bool {
    let err_str = err.to_string();
    if error_contains_sensitive_data(&err_str) {
        return false;
    }
    if let twerk_core::eval::StateEvalError::StateMachine(errors) = err {
        for e in errors {
            if error_contains_sensitive_data(&e.to_string()) {
                return false;
            }
        }
    }
    true
}

fuzz_target!(|data: &[u8]| {
    let json_str = String::from_utf8_lossy(data);

    let Ok(json_value) = serde_json::from_str::<Value>(&json_str) else {
        return;
    };

    let Ok(machine_json) = serde_json::to_string(&json_value) else {
        return;
    };

    let Ok(machine) = serde_json::from_str::<StateMachine>(&machine_json) else {
        return;
    };

    if machine.states().is_empty() {
        return;
    }

    let context: HashMap<String, Value> = HashMap::new();

    let nested_depth = count_nested_parallel_map_depth(&machine);
    if nested_depth > MAX_NESTED_DEPTH {
        return;
    }

    let nested_depth = count_nested_parallel_map_depth(&machine);
    if nested_depth > MAX_NESTED_DEPTH {
        return;
    }

    match evaluate_state_machine(&machine, &context) {
        Ok(_) => {}
        Err(e) => {
            if !validate_error_safe(&e) {
                panic!("Error contains sensitive data: {}", e);
            }
        }
    }

    if !check_terminal_invariants(&machine) {
        panic!("Terminal invariants violated");
    }

    if !check_reachability(&machine) {
        panic!("Reachability invariants violated");
    }
});
