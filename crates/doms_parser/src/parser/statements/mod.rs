use super::{ParseError, Parser, is_strict_reserved_word};
use crate::ast::*;
use crate::lexer::TokenKind;

mod dispatch;
mod do_while;
mod for_stmt;
mod if_while;
mod return_block;
mod switch;
mod try_catch;
mod var_decl;
mod with;
