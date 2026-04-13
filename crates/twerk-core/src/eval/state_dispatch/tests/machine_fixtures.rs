use super::fixtures::*;
use super::*;

pub(super) fn machine_missing_start_at() -> TestResult<StateMachine> {
    let mut states = IndexMap::new();
    states.insert(
        sn("Init")?,
        State::new(StateKind::Pass(PassState::new(None, next_t("Done")?))),
    );
    states.insert(
        sn("Done")?,
        State::new(StateKind::Succeed(SucceedState::new())),
    );

    Ok(StateMachine::new(sn("Missing")?, states))
}

pub(super) fn machine_missing_transition_target() -> TestResult<StateMachine> {
    let mut states = IndexMap::new();
    states.insert(
        sn("Init")?,
        State::new(StateKind::Pass(PassState::new(None, next_t("Ghost")?))),
    );
    states.insert(
        sn("Done")?,
        State::new(StateKind::Succeed(SucceedState::new())),
    );

    Ok(StateMachine::new(sn("Init")?, states))
}

pub(super) fn machine_missing_choice_target() -> TestResult<StateMachine> {
    let choice = fixture(ChoiceState::new(
        vec![ChoiceRule::new(
            expr("$.input.value > 10")?,
            sn("Ghost")?,
            None,
        )],
        Some(sn("Done")?),
    ))?;

    machine(
        "Choose",
        [
            ("Choose", State::new(StateKind::Choice(choice))),
            ("Done", State::new(StateKind::Succeed(SucceedState::new()))),
        ],
    )
}

pub(super) fn machine_missing_default_target() -> TestResult<StateMachine> {
    let choice = fixture(ChoiceState::new(
        vec![ChoiceRule::new(
            expr("$.input.value > 10")?,
            sn("Done")?,
            None,
        )],
        Some(sn("Ghost")?),
    ))?;

    machine(
        "Choose",
        [
            ("Choose", State::new(StateKind::Choice(choice))),
            ("Done", State::new(StateKind::Succeed(SucceedState::new()))),
        ],
    )
}

pub(super) fn machine_without_terminal_state() -> TestResult<StateMachine> {
    machine(
        "LoopA",
        [
            (
                "LoopA",
                State::new(StateKind::Pass(PassState::new(None, next_t("LoopB")?))),
            ),
            (
                "LoopB",
                State::new(StateKind::Pass(PassState::new(None, next_t("LoopA")?))),
            ),
        ],
    )
}

pub(super) fn parallel_state_with_invalid_branch(
    transition: Transition,
) -> TestResult<ParallelState> {
    fixture(ParallelState::new(
        vec![machine_missing_start_at()?],
        transition,
        Some(true),
    ))
}

pub(super) fn map_state_with_invalid_item_processor(
    transition: Transition,
) -> TestResult<MapState> {
    fixture(MapState::new(
        expr("$.input.items")?,
        Box::new(machine_missing_transition_target()?),
        Some(2),
        transition,
        vec![],
        vec![],
        Some(25.0),
    ))
}

pub(super) fn dense_choice_state() -> TestResult<ChoiceState> {
    fixture(ChoiceState::new(
        vec![
            ChoiceRule::new(expr("$.input.value > 10")?, sn("WaitStep")?, None),
            ChoiceRule::new(expr("$.input.value <= 10")?, sn("ParallelStep")?, None),
        ],
        Some(sn("FailStep")?),
    ))
}

pub(super) fn dense_task_step() -> TestResult<(&'static str, State)> {
    Ok((
        "TaskStep",
        wrapped_state(StateKind::Task(valid_task_state(
            Some(10),
            Some(9),
            next_t("PassStep")?,
        )?))?,
    ))
}

pub(super) fn dense_pass_step() -> TestResult<(&'static str, State)> {
    Ok((
        "PassStep",
        wrapped_state(StateKind::Pass(PassState::new(
            Some(json!({"kind": "pass"})),
            next_t("ChoiceStep")?,
        )))?,
    ))
}

pub(super) fn dense_choice_step() -> TestResult<(&'static str, State)> {
    Ok((
        "ChoiceStep",
        wrapped_state(StateKind::Choice(dense_choice_state()?))?,
    ))
}

pub(super) fn dense_wait_step() -> TestResult<(&'static str, State)> {
    Ok((
        "WaitStep",
        wrapped_state(StateKind::Wait(valid_wait_state(
            WaitDuration::Seconds(5),
            next_t("MapStep")?,
        )))?,
    ))
}

pub(super) fn dense_parallel_step() -> TestResult<(&'static str, State)> {
    Ok((
        "ParallelStep",
        wrapped_state(StateKind::Parallel(valid_parallel_state(next_t(
            "MapStep",
        )?)?))?,
    ))
}

pub(super) fn dense_map_step() -> TestResult<(&'static str, State)> {
    Ok((
        "MapStep",
        wrapped_state(StateKind::Map(valid_map_state(
            Some(100.0),
            next_t("SucceedStep")?,
        )?))?,
    ))
}

pub(super) fn dense_succeed_step() -> TestResult<(&'static str, State)> {
    Ok((
        "SucceedStep",
        wrapped_state(StateKind::Succeed(SucceedState::new()))?,
    ))
}

pub(super) fn dense_fail_step() -> TestResult<(&'static str, State)> {
    Ok((
        "FailStep",
        wrapped_state(StateKind::Fail(FailState::new(
            Some("Boom".to_owned()),
            Some("still terminal".to_owned()),
        )))?,
    ))
}

pub(super) fn dense_all_variant_states() -> TestResult<[(&'static str, State); 8]> {
    Ok([
        dense_task_step()?,
        dense_pass_step()?,
        dense_choice_step()?,
        dense_wait_step()?,
        dense_parallel_step()?,
        dense_map_step()?,
        dense_succeed_step()?,
        dense_fail_step()?,
    ])
}

pub(super) fn dense_all_variant_machine() -> TestResult<StateMachine> {
    machine("TaskStep", dense_all_variant_states()?).map(|machine| machine.with_timeout(u64::MAX))
}

pub(super) fn assert_machine_dispatch(machine: StateMachine) {
    let result = evaluate_state_machine(&machine, &context());

    assert_eq!(result, Ok(machine));
}

pub(super) fn parallel_inside_map_nested_machine() -> TestResult<StateMachine> {
    machine(
        "MapChildStart",
        [
            (
                "MapChildStart",
                State::new(StateKind::Parallel(valid_parallel_state(next_t(
                    "MapChildDone",
                )?)?)),
            ),
            (
                "MapChildDone",
                State::new(StateKind::Succeed(SucceedState::new())),
            ),
        ],
    )
}

pub(super) fn parallel_inside_map_machine() -> TestResult<StateMachine> {
    let map_state = fixture(MapState::new(
        expr("$.input.items")?,
        Box::new(parallel_inside_map_nested_machine()?),
        Some(2),
        next_t("Done")?,
        vec![],
        vec![],
        Some(25.0),
    ))?;

    machine(
        "MapIt",
        [
            ("MapIt", State::new(StateKind::Map(map_state))),
            ("Done", State::new(StateKind::Succeed(SucceedState::new()))),
        ],
    )
}

pub(super) fn map_inside_parallel_branch_machine() -> TestResult<StateMachine> {
    machine(
        "BranchMap",
        [
            (
                "BranchMap",
                State::new(StateKind::Map(valid_map_state(
                    Some(40.0),
                    next_t("BranchDone")?,
                )?)),
            ),
            (
                "BranchDone",
                State::new(StateKind::Succeed(SucceedState::new())),
            ),
        ],
    )
}

pub(super) fn map_inside_parallel_machine() -> TestResult<StateMachine> {
    let parallel_state = fixture(ParallelState::new(
        vec![map_inside_parallel_branch_machine()?],
        next_t("Done")?,
        Some(false),
    ))?;

    machine(
        "Fork",
        [
            ("Fork", State::new(StateKind::Parallel(parallel_state))),
            ("Done", State::new(StateKind::Succeed(SucceedState::new()))),
        ],
    )
}
