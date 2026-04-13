#![allow(clippy::unnecessary_wraps, dead_code, unused_imports)]

use std::collections::HashMap;
use std::error::Error as StdError;

use indexmap::IndexMap;
use rstest::rstest;
use serde_json::{json, Value};

use super::arms::{
    build_choice_arm, build_map_arm, build_parallel_arm, build_task_arm, MapArmExecution,
    MapArmSpec, ParallelArmFailFast, ParallelArmSpec, TaskArmRecovery, TaskArmSpec, TaskArmTiming,
};
use super::{evaluate_state, evaluate_state_kind, evaluate_state_machine, StateEvalError};
use crate::asl::{
    ChoiceRule, ChoiceState, ChoiceStateError, Expression, FailState, ImageRef, JsonPath, MapState,
    MapStateError, ParallelState, ParallelStateError, PassState, ShellScript, State, StateKind,
    StateMachine, StateName, SucceedState, TaskState, TaskStateError, Transition, VariableName,
    WaitDuration, WaitState,
};

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

mod builders;
mod eval_kind;
mod eval_machine;
mod eval_state;
mod fixtures;
mod machine_fixtures;
mod static_guards;
