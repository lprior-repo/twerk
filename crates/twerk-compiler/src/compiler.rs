use crate::error::CompilerResult;
use crate::types::{CompiledWorkflow, ValidatedWorkflow};

pub fn compile_validated_workflow(workflow: ValidatedWorkflow) -> CompilerResult<CompiledWorkflow> {
    Ok(CompiledWorkflow::from_validated(&workflow))
}
