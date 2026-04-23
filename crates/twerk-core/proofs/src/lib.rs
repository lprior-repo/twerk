// Kani proof harnesses for twerk-core
//
// These harnesses verify:
// 1. State transition validity (ErrorCode, Transition, WaitDuration)
// 2. ASL structural invariants (StateMachine, Retrier, Choice, Catcher)
// 3. Domain type validation (Port, Progress, BackoffRate, Hostname, etc.)
// 4. Redaction correctness
// 5. Webhook retry logic
// 6. State machine transitions (Job, Task)
// 7. Eval context conversions
// 8. Data flow path operations

mod error_code_harness;
mod retrier_harness;
mod choice_harness;
mod machine_harness;
mod transition_harness;
mod wait_harness;
mod types_harness;
mod redact_harness;
mod asl_types_harness;
mod domain_harness;
mod job_state_harness;
mod task_state_harness;
mod webhook_harness;
mod eval_context_harness;
mod data_flow_harness;
mod transform_harness;
mod asl_validation_harness;
mod asl_task_state_harness;
mod asl_constructors_harness;
mod redact_deep_harness;
