use super::{ParseError, Parser, is_strict_reserved_word};
use crate::ast::*;
use crate::lexer::TokenKind;

mod array;
mod async_expr;
mod identifier;
mod new_expr;
mod object;
mod paren_arrow;
mod pratt;
mod prefix;
mod reserved_words;
mod template;
mod update;

pub(crate) fn unwrap_grouping<'b>(mut expr: &'b Expression) -> &'b Expression {
    while let Expression::Grouping(inner) = expr {
        expr = inner;
    }
    expr
}
