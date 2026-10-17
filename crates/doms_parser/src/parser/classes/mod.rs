use super::{is_strict_reserved_word, ParseError, Parser};
use crate::ast::*;
use crate::lexer::TokenKind;
use std::collections::HashSet;

mod decl;
mod elements;
mod expr;
mod private_names;
mod properties;
