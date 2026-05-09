use twerk_compiler::{lex_expression, CompilerError, Token};

#[test]
fn lexes_expression_tokens() -> Result<(), CompilerError> {
    let tokens = lex_expression("$input.total >= 10 and contains($items, \"book\")")?;
    let token_kinds = tokens
        .into_iter()
        .map(|spanned| spanned.token)
        .collect::<Vec<_>>();

    assert_eq!(
        token_kinds,
        vec![
            Token::Dollar,
            Token::Ident("input".into()),
            Token::Dot,
            Token::Ident("total".into()),
            Token::Gte,
            Token::Number("10".into()),
            Token::And,
            Token::Contains,
            Token::LParen,
            Token::Dollar,
            Token::Ident("items".into()),
            Token::Comma,
            Token::String("book".into()),
            Token::RParen,
        ]
    );

    Ok(())
}

#[test]
fn tracks_line_and_column_after_skipped_comments() -> Result<(), CompilerError> {
    let tokens = lex_expression("$foo\n// ignored\nand")?;
    let token_locations = tokens
        .into_iter()
        .map(|spanned| {
            (
                spanned.token,
                spanned.span.start.line,
                spanned.span.start.column,
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        token_locations,
        vec![
            (Token::Dollar, 1, 1),
            (Token::Ident("foo".into()), 1, 2),
            (Token::And, 3, 1)
        ]
    );

    Ok(())
}

#[test]
fn reports_invalid_token_location() {
    let result = lex_expression("$foo @");

    assert!(matches!(
        result,
        Err(CompilerError::Lex {
            line: 1,
            column: 6,
            ..
        })
    ));
}
