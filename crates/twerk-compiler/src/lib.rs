pub mod compiler;
pub mod error;
pub mod expr_ast;
pub mod expr_parser;
pub mod lexer;
pub mod parser;
pub mod source_map;
pub mod types;
pub mod validator;

pub use compiler::compile_validated_workflow;
pub use error::{CompilerError, CompilerResult};
pub use expr_ast::{
    BinaryOp, Expr, ExprKind, FunctionCall, FunctionName, Literal, SlotAccess, UnaryOp,
};
pub use expr_parser::{parse_expression, parse_expression_tokens};
pub use lexer::{lex_expression, SpannedToken, Token, TokenSpan};
pub use parser::parse_yaml_bytes;
pub use source_map::{ByteOffset, SourceLocation, SourceMap, SourceSpan};
pub use types::{CompiledWorkflow, ParseTree, ValidatedWorkflow};
pub use validator::validate_parse_tree;

pub fn parse_yaml(input: &[u8]) -> CompilerResult<ParseTree> {
    parse_yaml_bytes(input)
}

pub fn validate_workflow(input: &[u8]) -> CompilerResult<ValidatedWorkflow> {
    parse_yaml(input).and_then(validate_parse_tree)
}

pub fn compile_workflow(input: &[u8]) -> CompilerResult<CompiledWorkflow> {
    validate_workflow(input).and_then(compile_validated_workflow)
}
