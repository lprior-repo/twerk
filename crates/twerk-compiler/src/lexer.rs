use std::ops::Range;

use logos::{Lexer, Logos};
use serde::{Deserialize, Serialize};

use crate::error::{CompilerError, CompilerResult};
use crate::source_map::{SourceLocation, SourceSpan};

#[derive(Logos, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[logos(skip r"[ \t\r\n\f]+")]
#[logos(skip r"//[^\n]*")]
#[logos(skip r"#[^\n]*")]
pub enum Token {
    #[token("$")]
    Dollar,
    #[token(".")]
    Dot,
    #[token("==")]
    Eq,
    #[token("!=")]
    Neq,
    #[token(">=")]
    Gte,
    #[token("<=")]
    Lte,
    #[token(">")]
    Gt,
    #[token("<")]
    Lt,
    #[token("and")]
    And,
    #[token("or")]
    Or,
    #[token("not")]
    Not,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("null")]
    Null,
    #[token("length")]
    Length,
    #[token("contains")]
    Contains,
    #[token("exists")]
    Exists,
    #[token("as")]
    As,
    #[regex(r"[0-9]+(\.[0-9]+)?", number_token)]
    Number(Box<str>),
    #[regex(r#""([^"\\]|\\.)*""#, string_token)]
    String(Box<str>),
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", text_token)]
    Ident(Box<str>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenSpan {
    pub bytes: SourceSpan,
    pub start: SourceLocation,
    pub end: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpannedToken {
    pub token: Token,
    pub span: TokenSpan,
}

pub fn lex_expression(source: &str) -> CompilerResult<Vec<SpannedToken>> {
    let lines = LineStarts::new(source);

    Token::lexer(source)
        .spanned()
        .map(|(token, range)| spanned_token(source, &lines, token, range))
        .collect()
}

fn spanned_token(
    source: &str,
    lines: &LineStarts,
    token: Result<Token, ()>,
    range: Range<usize>,
) -> CompilerResult<SpannedToken> {
    let start = lines.location(range.start);

    match token {
        Ok(token) => Ok(SpannedToken {
            token,
            span: TokenSpan {
                bytes: SourceSpan::new(range.start, range.end),
                start,
                end: lines.location(range.end),
            },
        }),
        Err(()) => Err(CompilerError::lex(
            start.line,
            start.column,
            invalid_fragment(source, range),
        )),
    }
}

fn invalid_fragment(source: &str, range: Range<usize>) -> &str {
    source.get(range).map_or("", |fragment| fragment)
}

fn text_token(lexer: &mut Lexer<'_, Token>) -> Box<str> {
    lexer.slice().into()
}

fn number_token(lexer: &mut Lexer<'_, Token>) -> Box<str> {
    lexer.slice().into()
}

fn string_token(lexer: &mut Lexer<'_, Token>) -> Box<str> {
    unquote_string(lexer.slice())
}

fn unquote_string(source: &str) -> Box<str> {
    match source
        .strip_prefix('"')
        .and_then(|without_prefix| without_prefix.strip_suffix('"'))
    {
        Some(inner) => inner.into(),
        None => source.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LineStarts {
    starts: Box<[usize]>,
}

impl LineStarts {
    fn new(source: &str) -> Self {
        let starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(index, _)| index + 1))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self { starts }
    }

    fn location(&self, offset: usize) -> SourceLocation {
        let line_index = self.starts.partition_point(|start| *start <= offset) - 1;
        let line_start = match self.starts.get(line_index) {
            Some(start) => *start,
            None => 0,
        };

        SourceLocation::new(line_index + 1, offset.saturating_sub(line_start) + 1)
    }
}
