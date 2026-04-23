use std::collections::HashMap;

use serde_json::Value;

use crate::asl::{
    Catcher, ChoiceRule, ChoiceState, Expression, ImageRef, MapState, ParallelState, Retrier,
    ShellScript, StateKind, StateMachine, StateName, TaskState, Transition, VariableName,
};

use super::{evaluate_state_machine, StateEvalError};

// ---------------------------------------------------------------------------
// Arm-level seam builders — raw-parameter constructors, testable with invalid inputs.
// ---------------------------------------------------------------------------

pub(super) struct TaskArmTiming {
    pub(super) timeout: Option<u64>,
    pub(super) heartbeat: Option<u64>,
}

pub(super) struct TaskArmRecovery {
    pub(super) retry: Vec<Retrier>,
    pub(super) catch: Vec<Catcher>,
}

pub(super) struct TaskArmSpec {
    pub(super) image: ImageRef,
    pub(super) run: ShellScript,
    pub(super) env: HashMap<String, Expression>,
    pub(super) var: Option<VariableName>,
    pub(super) timing: TaskArmTiming,
    pub(super) recovery: TaskArmRecovery,
    pub(super) transition: Transition,
}

impl TaskArmSpec {
    fn from_state(task: &TaskState) -> Self {
        Self {
            image: task.image().clone(),
            run: task.run().clone(),
            env: task.env().clone(),
            var: task.var().cloned(),
            timing: TaskArmTiming {
                timeout: task.timeout(),
                heartbeat: task.heartbeat(),
            },
            recovery: TaskArmRecovery {
                retry: task.retry().to_vec(),
                catch: task.catch().to_vec(),
            },
            transition: task.transition().clone(),
        }
    }
}

pub(super) fn build_task_arm(spec: TaskArmSpec) -> Result<StateKind, StateEvalError> {
    let TaskArmSpec {
        image,
        run,
        env,
        var,
        timing,
        recovery,
        transition,
    } = spec;

    TaskState::new(
        image,
        run,
        env,
        var,
        timing.timeout,
        timing.heartbeat,
        recovery.retry,
        recovery.catch,
        transition,
    )
    .map(StateKind::Task)
    .map_err(StateEvalError::from)
}

pub(super) fn dispatch_task_from_state(task: &TaskState) -> Result<StateKind, StateEvalError> {
    build_task_arm(TaskArmSpec::from_state(task))
}

pub(super) fn build_choice_arm(
    choices: Vec<ChoiceRule>,
    default: Option<StateName>,
) -> Result<StateKind, StateEvalError> {
    ChoiceState::new(choices, default)
        .map(StateKind::Choice)
        .map_err(StateEvalError::from)
}

#[derive(Clone, Copy)]
pub(super) enum ParallelArmFailFast {
    RuntimeDefault,
    StopOnFirstFailure,
    WaitForAllBranches,
}

impl ParallelArmFailFast {
    fn into_option(self) -> Option<bool> {
        match self {
            Self::RuntimeDefault => None,
            Self::StopOnFirstFailure => Some(true),
            Self::WaitForAllBranches => Some(false),
        }
    }
}

impl From<Option<bool>> for ParallelArmFailFast {
    fn from(value: Option<bool>) -> Self {
        match value {
            Some(true) => Self::StopOnFirstFailure,
            Some(false) => Self::WaitForAllBranches,
            None => Self::RuntimeDefault,
        }
    }
}

pub(super) struct ParallelArmSpec {
    pub(super) branches: Vec<StateMachine>,
    pub(super) transition: Transition,
    pub(super) fail_fast: ParallelArmFailFast,
}

impl ParallelArmSpec {
    fn from_state(parallel: &ParallelState, branches: Vec<StateMachine>) -> Self {
        Self {
            branches,
            transition: parallel.transition().clone(),
            fail_fast: parallel.fail_fast().into(),
        }
    }
}

pub(super) fn build_parallel_arm(spec: ParallelArmSpec) -> Result<StateKind, StateEvalError> {
    ParallelState::new(spec.branches, spec.transition, spec.fail_fast.into_option())
        .map(StateKind::Parallel)
        .map_err(StateEvalError::from)
}

pub(super) fn eval_parallel_arm(
    parallel: &ParallelState,
    context: &HashMap<String, Value>,
) -> Result<StateKind, StateEvalError> {
    parallel
        .branches()
        .iter()
        .map(|branch| evaluate_state_machine(branch, context))
        .collect::<Result<Vec<_>, _>>()
        .map(|branches| ParallelArmSpec::from_state(parallel, branches))
        .and_then(build_parallel_arm)
}

pub(super) struct MapArmExecution {
    pub(super) max_concurrency: Option<u32>,
    pub(super) transition: Transition,
    pub(super) retry: Vec<Retrier>,
    pub(super) catch: Vec<Catcher>,
    pub(super) tolerance: Option<f64>,
}

pub(super) struct MapArmSpec {
    pub(super) items_path: Expression,
    pub(super) item_processor: Box<StateMachine>,
    pub(super) execution: MapArmExecution,
}

impl MapArmSpec {
    fn from_state(map: &MapState, item_processor: Box<StateMachine>) -> Self {
        Self {
            items_path: map.items_path().clone(),
            item_processor,
            execution: MapArmExecution {
                max_concurrency: map.max_concurrency(),
                transition: map.transition().clone(),
                retry: map.retry().to_vec(),
                catch: map.catch().to_vec(),
                tolerance: map.tolerated_failure_percentage(),
            },
        }
    }
}

pub(super) fn build_map_arm(spec: MapArmSpec) -> Result<StateKind, StateEvalError> {
    let MapArmSpec {
        items_path,
        item_processor,
        execution,
    } = spec;

    MapState::new(
        items_path,
        item_processor,
        execution.max_concurrency,
        execution.transition,
        execution.retry,
        execution.catch,
        execution.tolerance,
    )
    .map(StateKind::Map)
    .map_err(StateEvalError::from)
}

pub(super) fn eval_map_arm(
    map: &MapState,
    context: &HashMap<String, Value>,
) -> Result<StateKind, StateEvalError> {
    evaluate_state_machine(map.item_processor(), context)
        .map(Box::new)
        .map(|item_processor| MapArmSpec::from_state(map, item_processor))
        .and_then(build_map_arm)
}
