use serde::{Deserialize, Serialize};

use crate::source_map::SourceMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseTree {
    pub source: Box<str>,
    pub source_map: SourceMap,
}

impl ParseTree {
    #[must_use]
    pub fn new(source: &str) -> Self {
        Self {
            source: source.into(),
            source_map: SourceMap::for_len(source.len()),
        }
    }

    #[must_use]
    pub fn source_len(&self) -> usize {
        self.source.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedWorkflow {
    pub parse_tree: ParseTree,
}

impl ValidatedWorkflow {
    #[must_use]
    pub const fn from_parse_tree(parse_tree: ParseTree) -> Self {
        Self { parse_tree }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledWorkflow {
    pub source_len: usize,
}

impl CompiledWorkflow {
    #[must_use]
    pub fn from_validated(workflow: &ValidatedWorkflow) -> Self {
        Self {
            source_len: workflow.parse_tree.source_len(),
        }
    }
}
