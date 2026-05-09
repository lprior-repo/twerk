use std::str;

use crate::error::{CompilerError, CompilerResult};
use crate::types::ParseTree;

pub fn parse_yaml_bytes(input: &[u8]) -> CompilerResult<ParseTree> {
    str::from_utf8(input)
        .map(ParseTree::new)
        .map_err(CompilerError::invalid_utf8)
}
