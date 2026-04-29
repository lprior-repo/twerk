use std::boxed::Box;

#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub line: u32,
    pub column: u32,
    pub byte_offset: usize,
    pub byte_length: usize,
}

#[derive(Debug, Clone)]
pub struct DiagnosticPath {
    pub node_idx: u32,
    pub message: Box<str>,
}

#[derive(Debug, Clone)]
pub struct SourceMap {
    pub node_locations: Box<[SourceLocation]>,
    pub expr_locations: Box<[SourceLocation]>,
    pub diagnostic_paths: Box<[DiagnosticPath]>,
}