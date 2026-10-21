use super::{ParseError, Parser, is_strict_reserved_word};
use crate::ast::*;
use crate::lexer::TokenKind;
use std::collections::HashSet;

mod decl;
mod elements;
mod expr;
mod private_names;
mod properties;

impl<'a> Parser<'a> {
    pub(crate) fn parse_class_binding_identifier(&mut self) -> Result<String, ParseError> {
        let tok = self.advance()?;
        match &tok.kind {
            TokenKind::Ident(id) => {
                if tok.has_escape
                    && (crate::lexer::is_reserved_word(id)
                        || is_strict_reserved_word(id)
                        || ((self.in_async || self.is_module || self.in_static_block)
                            && id == "await")
                        || id == "yield")
                {
                    return Err(ParseError::UnexpectedToken(tok.kind.clone(), tok.line, tok.col));
                }
                if is_strict_reserved_word(id)
                    || id == "let"
                    || id == "yield"
                    || id == "eval"
                    || id == "arguments"
                {
                    return Err(ParseError::UnexpectedToken(tok.kind.clone(), tok.line, tok.col));
                }
                if (self.in_async || self.is_module || self.in_static_block) && id == "await" {
                    return Err(ParseError::UnexpectedToken(TokenKind::Await, tok.line, tok.col));
                }
                Ok(id.clone())
            }
            TokenKind::Await => {
                if self.in_async || self.is_module || self.in_static_block {
                    return Err(ParseError::UnexpectedToken(TokenKind::Await, tok.line, tok.col));
                }
                Ok("await".to_string())
            }
            _ => Err(ParseError::UnexpectedToken(tok.kind.clone(), tok.line, tok.col)),
        }
    }
}
