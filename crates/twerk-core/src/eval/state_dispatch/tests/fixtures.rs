use super::*;

pub(super) fn context() -> HashMap<String, Value> {
    HashMap::from([("input".to_owned(), json!({"value": 7, "items": [1, 2, 3]}))])
}

pub(super) fn fixture<T, E>(result: Result<T, E>) -> TestResult<T>
where
    E: StdError + 'static,
{
    result.map_err(Into::into)
}

pub(super) fn sn(value: &str) -> TestResult<StateName> {
    fixture(StateName::new(value))
}

pub(super) fn expr(value: &str) -> TestResult<Expression> {
    fixture(Expression::new(value))
}

pub(super) fn jp(value: &str) -> TestResult<JsonPath> {
    fixture(JsonPath::new(value))
}

pub(super) fn image(value: &str) -> TestResult<ImageRef> {
    fixture(ImageRef::new(value))
}

pub(super) fn script(value: &str) -> TestResult<ShellScript> {
    fixture(ShellScript::new(value))
}

pub(super) fn variable(value: &str) -> TestResult<VariableName> {
    fixture(VariableName::new(value))
}

pub(super) fn next_t(value: &str) -> TestResult<Transition> {
    sn(value).map(Transition::next)
}

pub(super) fn end_t() -> Transition {
    Transition::end()
}

pub(super) fn task_arm_spec(
    env: HashMap<String, Expression>,
    timeout: Option<u64>,
    heartbeat: Option<u64>,
) -> TestResult<TaskArmSpec> {
    Ok(TaskArmSpec {
        image: image("ghcr.io/foo:latest")?,
        run: script("echo")?,
        env,
        var: None,
        timing: TaskArmTiming { timeout, heartbeat },
        recovery: TaskArmRecovery {
            retry: vec![],
            catch: vec![],
        },
        transition: end_t(),
    })
}

pub(super) fn map_arm_spec(tolerance: Option<f64>) -> TestResult<MapArmSpec> {
    Ok(MapArmSpec {
        items_path: expr("$.items")?,
        item_processor: Box::new(terminal_machine("Done")?),
        execution: MapArmExecution {
            max_concurrency: None,
            transition: end_t(),
            retry: vec![],
            catch: vec![],
            tolerance,
        },
    })
}

pub(super) fn shared_assignments() -> TestResult<HashMap<VariableName, Expression>> {
    Ok(HashMap::from([(
        variable("result_id")?,
        expr("$.input.value")?,
    )]))
}

pub(super) fn task_env() -> TestResult<HashMap<String, Expression>> {
    Ok(HashMap::from([
        ("TOKEN".to_owned(), expr("7")?),
        ("MODE".to_owned(), expr("\"prod\"")?),
    ]))
}

pub(super) fn valid_task_state(
    timeout: Option<u64>,
    heartbeat: Option<u64>,
    transition: Transition,
) -> TestResult<TaskState> {
    task_state_with_env(task_env()?, timeout, heartbeat, transition)
}

pub(super) fn task_state_with_env(
    env: HashMap<String, Expression>,
    timeout: Option<u64>,
    heartbeat: Option<u64>,
    transition: Transition,
) -> TestResult<TaskState> {
    fixture(TaskState::new(
        image("ghcr.io/runabol/twerk:latest")?,
        script("echo hello")?,
        env,
        Some(variable("task_output")?),
        timeout,
        heartbeat,
        vec![],
        vec![],
        transition,
    ))
}

pub(super) fn invalid_task_state(transition: Transition) -> TestResult<TaskState> {
    task_state_with_env(
        HashMap::from([("BROKEN".to_owned(), expr("(")?)]),
        Some(10),
        Some(9),
        transition,
    )
}

pub(super) fn valid_wait_state(duration: WaitDuration, transition: Transition) -> WaitState {
    WaitState::new(duration, transition)
}

pub(super) fn assert_wait_state_preserved(duration: WaitDuration) -> TestResult {
    let state = wrapped_state(StateKind::Wait(valid_wait_state(duration, next_t("Done")?)))?;
    let result = evaluate_state(&state, &context());

    assert_eq!(result, Ok(state));
    Ok(())
}

pub(super) fn valid_choice_state() -> TestResult<ChoiceState> {
    fixture(ChoiceState::new(
        vec![
            ChoiceRule::new(expr("$.input.value > 10")?, sn("TooLarge")?, None),
            ChoiceRule::new(
                expr("$.input.value == 7")?,
                sn("ExactMatch")?,
                Some(shared_assignments()?),
            ),
        ],
        Some(sn("Fallback")?),
    ))
}

pub(super) fn terminal_machine(name: &str) -> TestResult<StateMachine> {
    let mut states = IndexMap::new();
    states.insert(
        sn(name)?,
        State::new(StateKind::Succeed(SucceedState::new())),
    );
    Ok(StateMachine::new(sn(name)?, states))
}

pub(super) fn branch_machine(prefix: &str) -> TestResult<StateMachine> {
    let start = format!("{prefix}Start");
    let done = format!("{prefix}Done");
    let mut states = IndexMap::new();
    states.insert(
        sn(&start)?,
        State::new(StateKind::Pass(PassState::new(None, next_t(&done)?))),
    );
    states.insert(
        sn(&done)?,
        State::new(StateKind::Succeed(SucceedState::new())),
    );
    Ok(StateMachine::new(sn(&start)?, states))
}

pub(super) fn valid_parallel_state(transition: Transition) -> TestResult<ParallelState> {
    fixture(ParallelState::new(
        vec![branch_machine("BranchOne")?, branch_machine("BranchTwo")?],
        transition,
        Some(true),
    ))
}

pub(super) fn valid_map_state(
    tolerance: Option<f64>,
    transition: Transition,
) -> TestResult<MapState> {
    fixture(MapState::new(
        expr("$.input.items")?,
        Box::new(branch_machine("MapChild")?),
        Some(3),
        transition,
        vec![],
        vec![],
        tolerance,
    ))
}
pub(super) fn assert_fail_state_preserved(error: Option<&str>, cause: Option<&str>) -> TestResult {
    let state = wrapped_state(StateKind::Fail(FailState::new(
        error.map(str::to_owned),
        cause.map(str::to_owned),
    )))?;
    let result = evaluate_state(&state, &context());

    assert_eq!(result, Ok(state));
    Ok(())
}

pub(super) fn wrapped_state(kind: StateKind) -> TestResult<State> {
    Ok(State::new(kind)
        .with_comment("dispatch this state")
        .with_input_path(jp("$.input")?)
        .with_output_path(jp("$.output")?)
        .with_assign(shared_assignments()?))
}

pub(super) fn machine<const N: usize>(
    start_at: &str,
    states: [(&str, State); N],
) -> TestResult<StateMachine> {
    let states = states
        .into_iter()
        .map(|(name, state)| sn(name).map(|state_name| (state_name, state)))
        .collect::<Result<IndexMap<_, _>, _>>()?;

    Ok(StateMachine::new(sn(start_at)?, states))
}
