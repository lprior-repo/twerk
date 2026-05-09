use winnow::combinator::{alt, opt, preceded, repeat, separated};
use winnow::error::ContextError;
use winnow::prelude::*;
use winnow::stream::{ContainsToken, TokenSlice};
use winnow::token::one_of;
use winnow::Result as WResult;

use crate::error::{CompilerError, CompilerResult};
use crate::expr_ast::{
    BinaryOp, Expr, ExprKind, FunctionCall, FunctionName, Literal, SlotAccess, UnaryOp,
};
use crate::lexer::{SpannedToken, Token};
use crate::source_map::{SourceLocation, SourceSpan};

type Tokens<'i> = TokenSlice<'i, SpannedToken>;

pub fn parse_expression(source: &str) -> CompilerResult<Expr> {
    crate::lexer::lex_expression(source).and_then(|tokens| parse_expression_tokens(&tokens))
}

pub fn parse_expression_tokens(tokens: &[SpannedToken]) -> CompilerResult<Expr> {
    let mut input = Tokens::new(tokens);
    let parsed = expression
        .parse_next(&mut input)
        .map_err(|_error| parse_failure(&input, "expected expression"))?;

    match input.first() {
        Some(token) => Err(CompilerError::parse(
            token.span.start.line,
            token.span.start.column,
            "unexpected token after expression",
        )),
        None => Ok(parsed),
    }
}

fn expression(input: &mut Tokens<'_>) -> WResult<Expr> {
    or_expr.parse_next(input)
}

fn or_expr(input: &mut Tokens<'_>) -> WResult<Expr> {
    let init = and_expr.parse_next(input)?;

    repeat(0.., (operator(ExprTokenKind::Or, BinaryOp::Or), and_expr))
        .fold(
            move || init.clone(),
            |acc, (op, right)| binary_expr(op, acc, right),
        )
        .parse_next(input)
}

fn and_expr(input: &mut Tokens<'_>) -> WResult<Expr> {
    let init = cmp_expr.parse_next(input)?;

    repeat(0.., (operator(ExprTokenKind::And, BinaryOp::And), cmp_expr))
        .fold(
            move || init.clone(),
            |acc, (op, right)| binary_expr(op, acc, right),
        )
        .parse_next(input)
}

fn cmp_expr(input: &mut Tokens<'_>) -> WResult<Expr> {
    let left = add_expr.parse_next(input)?;

    opt((comparison_operator, add_expr))
        .map(move |comparison| match comparison {
            Some((op, right)) => binary_expr(op, left.clone(), right),
            None => left.clone(),
        })
        .parse_next(input)
}

fn add_expr(input: &mut Tokens<'_>) -> WResult<Expr> {
    let init = unary.parse_next(input)?;

    repeat(0.., (add_operator, unary))
        .fold(
            move || init.clone(),
            |acc, (op, right)| binary_expr(op, acc, right),
        )
        .parse_next(input)
}

fn unary(input: &mut Tokens<'_>) -> WResult<Expr> {
    match opt(ExprTokenKind::Not).parse_next(input)? {
        Some(token) => primary.parse_next(input).map(|expr| {
            Expr::new(
                ExprKind::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr.clone()),
                },
                join_spans(token.span.bytes, expr.span),
            )
        }),
        None => primary.parse_next(input),
    }
}

fn primary(input: &mut Tokens<'_>) -> WResult<Expr> {
    alt((literal, slot_access, func_call, paren_expr)).parse_next(input)
}

fn literal(input: &mut Tokens<'_>) -> WResult<Expr> {
    alt((
        number_literal,
        string_literal,
        true_literal,
        false_literal,
        null_literal,
    ))
    .parse_next(input)
}

fn number_literal(input: &mut Tokens<'_>) -> WResult<Expr> {
    let token = ExprTokenKind::Number.parse_next(input)?;

    match &token.token {
        Token::Number(value) => Ok(Expr::new(
            ExprKind::Literal(Literal::Number(value.clone())),
            token.span.bytes,
        )),
        _other => impossible_token(),
    }
}

fn string_literal(input: &mut Tokens<'_>) -> WResult<Expr> {
    let token = ExprTokenKind::String.parse_next(input)?;

    match &token.token {
        Token::String(value) => Ok(Expr::new(
            ExprKind::Literal(Literal::String(value.clone())),
            token.span.bytes,
        )),
        _other => impossible_token(),
    }
}

fn true_literal(input: &mut Tokens<'_>) -> WResult<Expr> {
    token_kind(ExprTokenKind::True)
        .map(|token| Expr::new(ExprKind::Literal(Literal::Bool(true)), token.span.bytes))
        .parse_next(input)
}

fn false_literal(input: &mut Tokens<'_>) -> WResult<Expr> {
    token_kind(ExprTokenKind::False)
        .map(|token| Expr::new(ExprKind::Literal(Literal::Bool(false)), token.span.bytes))
        .parse_next(input)
}

fn null_literal(input: &mut Tokens<'_>) -> WResult<Expr> {
    token_kind(ExprTokenKind::Null)
        .map(|token| Expr::new(ExprKind::Literal(Literal::Null), token.span.bytes))
        .parse_next(input)
}

fn slot_access(input: &mut Tokens<'_>) -> WResult<Expr> {
    let dollar = ExprTokenKind::Dollar.parse_next(input)?;
    let first = ident_segment.parse_next(input)?;
    let tail: Vec<PathSegment> =
        repeat(0.., preceded(ExprTokenKind::Dot, ident_segment)).parse_next(input)?;
    let end_span = tail.last().map_or(first.span, |segment| segment.span);
    let path = std::iter::once(first)
        .chain(tail)
        .map(|segment| segment.name)
        .collect();

    Ok(Expr::new(
        ExprKind::SlotAccess(SlotAccess { path }),
        join_spans(dollar.span.bytes, end_span),
    ))
}

fn func_call(input: &mut Tokens<'_>) -> WResult<Expr> {
    let name = function_name.parse_next(input)?;
    ExprTokenKind::LParen.parse_next(input)?;
    let args = separated(0.., expression, ExprTokenKind::Comma).parse_next(input)?;
    let rparen = ExprTokenKind::RParen.parse_next(input)?;

    Ok(Expr::new(
        ExprKind::FunctionCall(FunctionCall {
            name: name.name,
            args,
        }),
        join_spans(name.span, rparen.span.bytes),
    ))
}

fn paren_expr(input: &mut Tokens<'_>) -> WResult<Expr> {
    let lparen = ExprTokenKind::LParen.parse_next(input)?;
    let expr = expression.parse_next(input)?;
    let rparen = ExprTokenKind::RParen.parse_next(input)?;

    Ok(Expr::new(
        ExprKind::Paren(Box::new(expr)),
        join_spans(lparen.span.bytes, rparen.span.bytes),
    ))
}

fn comparison_operator(input: &mut Tokens<'_>) -> WResult<BinaryOp> {
    alt((
        operator(ExprTokenKind::Eq, BinaryOp::Eq),
        operator(ExprTokenKind::Neq, BinaryOp::Neq),
        operator(ExprTokenKind::Gt, BinaryOp::Gt),
        operator(ExprTokenKind::Lt, BinaryOp::Lt),
        operator(ExprTokenKind::Gte, BinaryOp::Gte),
        operator(ExprTokenKind::Lte, BinaryOp::Lte),
    ))
    .parse_next(input)
}

fn add_operator(input: &mut Tokens<'_>) -> WResult<BinaryOp> {
    alt((
        operator(ExprTokenKind::Plus, BinaryOp::Plus),
        operator(ExprTokenKind::Minus, BinaryOp::Minus),
    ))
    .parse_next(input)
}

fn operator<'i>(
    mut kind: ExprTokenKind,
    op: BinaryOp,
) -> impl Parser<Tokens<'i>, BinaryOp, ContextError> {
    move |input: &mut Tokens<'i>| kind.parse_next(input).map(|_token| op)
}

fn function_name(input: &mut Tokens<'_>) -> WResult<NamedSpan> {
    alt((length_name, contains_name, exists_name, ident_function_name)).parse_next(input)
}

fn ident_segment(input: &mut Tokens<'_>) -> WResult<PathSegment> {
    let token = ExprTokenKind::Ident.parse_next(input)?;

    match &token.token {
        Token::Ident(value) => Ok(PathSegment {
            name: value.clone(),
            span: token.span.bytes,
        }),
        _other => impossible_token(),
    }
}

fn token_kind<'i>(
    mut kind: ExprTokenKind,
) -> impl Parser<Tokens<'i>, &'i SpannedToken, ContextError> {
    move |input: &mut Tokens<'i>| kind.parse_next(input)
}

fn length_name(input: &mut Tokens<'_>) -> WResult<NamedSpan> {
    token_kind(ExprTokenKind::Length)
        .map(|token| NamedSpan {
            name: FunctionName::Length,
            span: token.span.bytes,
        })
        .parse_next(input)
}

fn contains_name(input: &mut Tokens<'_>) -> WResult<NamedSpan> {
    token_kind(ExprTokenKind::Contains)
        .map(|token| NamedSpan {
            name: FunctionName::Contains,
            span: token.span.bytes,
        })
        .parse_next(input)
}

fn exists_name(input: &mut Tokens<'_>) -> WResult<NamedSpan> {
    token_kind(ExprTokenKind::Exists)
        .map(|token| NamedSpan {
            name: FunctionName::Exists,
            span: token.span.bytes,
        })
        .parse_next(input)
}

fn ident_function_name(input: &mut Tokens<'_>) -> WResult<NamedSpan> {
    let token = ExprTokenKind::Ident.parse_next(input)?;

    match &token.token {
        Token::Ident(value) => Ok(NamedSpan {
            name: FunctionName::Ident(value.clone()),
            span: token.span.bytes,
        }),
        _other => impossible_token(),
    }
}

fn binary_expr(op: BinaryOp, left: Expr, right: Expr) -> Expr {
    let span = join_spans(left.span, right.span);

    Expr::new(
        ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        },
        span,
    )
}

fn join_spans(start: SourceSpan, end: SourceSpan) -> SourceSpan {
    SourceSpan::new(start.start, end.end)
}

fn impossible_token<T>() -> WResult<T> {
    Err(ContextError::new())
}

fn parse_failure(input: &Tokens<'_>, message: &str) -> CompilerError {
    let location = error_location(input);
    CompilerError::parse(location.line, location.column, message)
}

fn error_location(input: &Tokens<'_>) -> SourceLocation {
    match input.first() {
        Some(token) => token.span.start,
        None => previous_location(input),
    }
}

fn previous_location(input: &Tokens<'_>) -> SourceLocation {
    match input.previous_tokens().next() {
        Some(token) => token.span.end,
        None => SourceLocation::new(1, 1),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExprTokenKind {
    Dollar,
    Dot,
    Eq,
    Neq,
    Gte,
    Lte,
    Gt,
    Lt,
    And,
    Or,
    Not,
    Plus,
    Minus,
    LParen,
    RParen,
    Comma,
    True,
    False,
    Null,
    Length,
    Contains,
    Exists,
    Number,
    String,
    Ident,
}

impl<'i> Parser<Tokens<'i>, &'i SpannedToken, ContextError> for ExprTokenKind {
    fn parse_next(&mut self, input: &mut Tokens<'i>) -> WResult<&'i SpannedToken> {
        one_of(*self).parse_next(input)
    }
}

impl ContainsToken<&'_ SpannedToken> for ExprTokenKind {
    fn contains_token(&self, token: &'_ SpannedToken) -> bool {
        self.matches(&token.token)
    }
}

impl ExprTokenKind {
    fn matches(self, token: &Token) -> bool {
        matches!(
            (self, token),
            (Self::Dollar, Token::Dollar)
                | (Self::Dot, Token::Dot)
                | (Self::Eq, Token::Eq)
                | (Self::Neq, Token::Neq)
                | (Self::Gte, Token::Gte)
                | (Self::Lte, Token::Lte)
                | (Self::Gt, Token::Gt)
                | (Self::Lt, Token::Lt)
                | (Self::And, Token::And)
                | (Self::Or, Token::Or)
                | (Self::Not, Token::Not)
                | (Self::Plus, Token::Plus)
                | (Self::Minus, Token::Minus)
                | (Self::LParen, Token::LParen)
                | (Self::RParen, Token::RParen)
                | (Self::Comma, Token::Comma)
                | (Self::True, Token::True)
                | (Self::False, Token::False)
                | (Self::Null, Token::Null)
                | (Self::Length, Token::Length)
                | (Self::Contains, Token::Contains)
                | (Self::Exists, Token::Exists)
                | (Self::Number, Token::Number(_))
                | (Self::String, Token::String(_))
                | (Self::Ident, Token::Ident(_))
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NamedSpan {
    name: FunctionName,
    span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathSegment {
    name: Box<str>,
    span: SourceSpan,
}
