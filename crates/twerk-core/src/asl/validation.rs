//! Deep validation for ASL state machines beyond the 6 basic invariants.
//!
//! Provides reachability analysis, cycle detection (DFS coloring), and
//! dead-end identification for [`StateMachine`].

use std::collections::{HashMap, HashSet, VecDeque};

use super::machine::StateMachine;
use super::state::StateKind;
use super::transition::Transition;
use super::types::StateName;

// ---------------------------------------------------------------------------
// ValidationReport
// ---------------------------------------------------------------------------

/// Results of deep state-machine analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    /// States not reachable from `start_at`.
    pub unreachable_states: Vec<StateName>,
    /// Cycles detected via DFS back-edges (each cycle as a path).
    pub cycles: Vec<Vec<StateName>>,
    /// Non-terminal states with no outgoing transition and not Choice.
    pub dead_end_states: Vec<StateName>,
}

impl ValidationReport {
    /// Returns `true` when every field is empty — no issues found.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.unreachable_states.is_empty()
            && self.cycles.is_empty()
            && self.dead_end_states.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Graph helpers
// ---------------------------------------------------------------------------

/// Collect all outgoing edge targets for a state.
fn outgoing_targets(kind: &StateKind) -> Vec<&StateName> {
    let mut targets = Vec::new();

    // Non-choice states: check the transition field
    if let Some(Transition::Next(ref target)) = kind.transition() {
        targets.push(target);
    }

    // Choice states: gather rule targets + default
    if let StateKind::Choice(ref cs) = kind {
        for rule in cs.choices() {
            targets.push(rule.next());
        }
        if let Some(default) = cs.default() {
            targets.push(default);
        }
    }

    targets
}

/// BFS from `start_at` to find all reachable state names.
fn reachable_set(machine: &StateMachine) -> HashSet<StateName> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    visited.insert(machine.start_at().clone());
    queue.push_back(machine.start_at());

    while let Some(current) = queue.pop_front() {
        if let Some(state) = machine.states().get(current) {
            for target in outgoing_targets(state.kind()) {
                if visited.insert(target.clone()) {
                    queue.push_back(target);
                }
            }
        }
    }

    visited
}

// ---------------------------------------------------------------------------
// Cycle detection — DFS coloring (white/gray/black)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    White,
    Gray,
    Black,
}

fn detect_cycles(machine: &StateMachine) -> Vec<Vec<StateName>> {
    let mut color: HashMap<&StateName, Color> = machine
        .states()
        .keys()
        .map(|k| (k, Color::White))
        .collect();

    let mut cycles = Vec::new();
    let mut path: Vec<&StateName> = Vec::new();

    for start in machine.states().keys() {
        if color.get(start).copied() == Some(Color::White) {
            dfs_visit(machine, start, &mut color, &mut path, &mut cycles);
        }
    }

    cycles
}

fn dfs_visit<'a>(
    machine: &'a StateMachine,
    node: &'a StateName,
    color: &mut HashMap<&'a StateName, Color>,
    path: &mut Vec<&'a StateName>,
    cycles: &mut Vec<Vec<StateName>>,
) {
    color.insert(node, Color::Gray);
    path.push(node);

    if let Some(state) = machine.states().get(node) {
        for target in outgoing_targets(state.kind()) {
            // Only consider targets that exist in the machine
            if !machine.states().contains_key(target) {
                continue;
            }
            match color.get(target).copied() {
                Some(Color::Gray) => {
                    // Back edge → extract cycle from path
                    let cycle_start = path.iter().position(|n| *n == target);
                    if let Some(idx) = cycle_start {
                        let cycle: Vec<StateName> =
                            path[idx..].iter().map(|n| (*n).clone()).collect();
                        cycles.push(cycle);
                    }
                }
                Some(Color::White) => {
                    dfs_visit(machine, target, color, path, cycles);
                }
                _ => {} // Black — already fully explored
            }
        }
    }

    path.pop();
    color.insert(node, Color::Black);
}

// ---------------------------------------------------------------------------
// Dead-end detection
// ---------------------------------------------------------------------------

fn find_dead_ends(machine: &StateMachine, reachable: &HashSet<StateName>) -> Vec<StateName> {
    machine
        .states()
        .iter()
        .filter(|(name, state)| {
            reachable.contains(*name)
                && !state.kind().is_terminal()
                && state.kind().transition().is_none()
                && !matches!(state.kind(), StateKind::Choice(_))
        })
        .map(|(name, _)| name.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Perform deep analysis of a state machine's graph structure.
///
/// Builds a directed graph, runs BFS for reachability, DFS coloring for
/// cycles, and checks for dead-end non-terminal states.
#[must_use]
pub fn analyze(machine: &StateMachine) -> ValidationReport {
    let reachable = reachable_set(machine);

    let unreachable_states: Vec<StateName> = machine
        .states()
        .keys()
        .filter(|name| !reachable.contains(*name))
        .cloned()
        .collect();

    let cycles = detect_cycles(machine);
    let dead_end_states = find_dead_ends(machine, &reachable);

    ValidationReport {
        unreachable_states,
        cycles,
        dead_end_states,
    }
}
