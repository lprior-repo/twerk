use twerk_compiler::{compile_workflow, parse_yaml, validate_workflow, CompilerError};

#[test]
fn parse_yaml_accepts_utf8_source() -> Result<(), CompilerError> {
    let tree = parse_yaml(b"name: demo\n")?;

    assert_eq!(tree.source.as_ref(), "name: demo\n");
    assert_eq!(tree.source_map.root.start, 0);
    assert_eq!(tree.source_map.root.end, 11);

    Ok(())
}

#[test]
fn parse_yaml_rejects_invalid_utf8() {
    let parsed = parse_yaml(&[0xff]);

    assert!(matches!(parsed, Err(CompilerError::InvalidUtf8 { .. })));
}

#[test]
fn public_pipeline_returns_compiled_stage() -> Result<(), CompilerError> {
    let workflow = compile_workflow(b"name: demo\n")?;

    assert_eq!(workflow.source_len, 11);

    Ok(())
}

#[test]
fn validation_stage_preserves_parse_tree() -> Result<(), CompilerError> {
    let workflow = validate_workflow(b"name: demo\n")?;

    assert_eq!(workflow.parse_tree.source.as_ref(), "name: demo\n");

    Ok(())
}
