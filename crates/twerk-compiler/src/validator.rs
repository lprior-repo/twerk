use crate::error::CompilerResult;
use crate::types::{ParseTree, ValidatedWorkflow};

pub fn validate_parse_tree(parse_tree: ParseTree) -> CompilerResult<ValidatedWorkflow> {
    Ok(ValidatedWorkflow::from_parse_tree(parse_tree))
}
