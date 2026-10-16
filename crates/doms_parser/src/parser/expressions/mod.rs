use super::{is_strict_reserved_word, ParseError, Parser};
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
