//! Pratt parser for JavaScript syntax.

pub mod classes;
pub mod destructuring;
pub mod expressions;
pub mod functions;
pub mod modules;
pub mod program;
pub mod statements;
pub mod typescript;
pub mod validation;

use crate::lexer::{LexError, Lexer, Token, TokenKind};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("Lexer error: {0}")]
    Lex(#[from] LexError),
    #[error("Unexpected token {0:?} at line {1}, col {2}")]
    UnexpectedToken(TokenKind, usize, usize),
    #[error("Unexpected EOF")]
    UnexpectedEof,
    #[error("Invalid assignment target at line {0}, col {1}")]
    InvalidAssignmentTarget(usize, usize),
    #[error("Syntax error: {0}")]
    General(String),
}

pub struct Parser<'a> {
    pub(crate) lexer: Lexer<'a>,
    pub(crate) current: Token,
    pub(crate) peeked: Token,
    pub(crate) destruct_counter: usize,
    pub is_module: bool,
    pub is_strict: bool,
    pub is_top_level: bool,
    pub in_generator: bool,
    pub in_function: bool,
    pub in_async: bool,
    pub in_non_arrow_function: bool,
    pub in_method: bool,
    pub in_derived_constructor: bool,
    pub allow_in: bool,
    pub in_param: bool,
    pub in_single_statement_context: bool,
    pub in_static_block: bool,
    pub is_eval: bool,
    pub private_names_stack: Vec<std::collections::HashMap<String, String>>,
    pub class_id_counter: usize,
    pub(crate) trailing_comma_after_rest: bool,
    pub(crate) current_params_simple: bool,
    pub allow_spread: bool,
}

#[inline]
pub fn is_strict_reserved_word(s: &str) -> bool {
    matches!(
        s,
        "implements"
            | "interface"
            | "let"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "static"
            | "yield"
    )
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token()?;
        let peeked = lexer.next_token()?;
        let is_strict = lexer.is_strict
            || matches!(&current.kind, TokenKind::StringLit { value, has_escape, .. } if value == "use strict" && !has_escape);
        lexer.is_strict = is_strict;
        Ok(Self {
            lexer,
            current,
            peeked,
            destruct_counter: 0,
            is_module: false,
            is_strict,
            is_top_level: true,
            in_generator: false,
            in_function: false,
            in_async: false,
            in_non_arrow_function: false,
            in_method: false,
            in_derived_constructor: false,
            allow_in: true,
            in_param: false,
            in_single_statement_context: false,
            in_static_block: false,
            is_eval: false,
            private_names_stack: Vec::new(),
            class_id_counter: 0,
            trailing_comma_after_rest: false,
            current_params_simple: true,
            allow_spread: false,
        })
    }

    pub fn new_module(source: &'a str) -> Result<Self, ParseError> {
        let mut p = Self::new_strict(source)?;
        p.is_module = true;
        Ok(p)
    }

    pub fn new_strict(source: &'a str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new_strict(source);
        let current = lexer.next_token()?;
        let peeked = lexer.next_token()?;
        Ok(Self {
            lexer,
            current,
            peeked,
            destruct_counter: 0,
            is_module: false,
            is_strict: true,
            is_top_level: true,
            in_generator: false,
            in_function: false,
            in_async: false,
            in_non_arrow_function: false,
            in_method: false,
            in_derived_constructor: false,
            allow_in: true,
            in_param: false,
            in_single_statement_context: false,
            in_static_block: false,
            is_eval: false,
            private_names_stack: Vec::new(),
            class_id_counter: 0,
            trailing_comma_after_rest: false,
            current_params_simple: true,
            allow_spread: false,
        })
    }

    pub fn new_eval(
        source: &'a str,
        is_strict: bool,
        in_method: bool,
        in_function: bool,
    ) -> Result<Self, ParseError> {
        let mut p = if is_strict { Self::new_strict(source)? } else { Self::new(source)? };
        p.in_method = in_method;
        p.in_function = in_function;
        p.in_non_arrow_function = in_function;
        p.is_eval = true;
        Ok(p)
    }

    pub(crate) fn advance(&mut self) -> Result<Token, ParseError> {
        let prev = std::mem::replace(
            &mut self.current,
            std::mem::replace(&mut self.peeked, self.lexer.next_token()?),
        );
        Ok(prev)
    }

    pub(crate) fn check(&self, kind: &TokenKind) -> bool {
        &self.current.kind == kind
    }

    pub(crate) fn check_ident(&self, name: &str) -> bool {
        match &self.current.kind {
            TokenKind::Ident(s) => s == name && !self.current.has_escape,
            _ => false,
        }
    }

    pub(crate) fn resolve_private_name(&self, name: &str) -> Option<String> {
        self.private_names_stack.iter().rev().find_map(|map| map.get(name).cloned())
    }

    pub(crate) fn match_token(&mut self, kind: &TokenKind) -> Result<bool, ParseError> {
        if self.check(kind) {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn consume_semicolon(&mut self) -> Result<(), ParseError> {
        if self.match_token(&TokenKind::Semicolon)? {
            return Ok(());
        }
        if self.current.has_preceding_line_terminator
            || self.current.kind == TokenKind::RBrace
            || self.current.kind == TokenKind::Eof
        {
            return Ok(());
        }
        Err(ParseError::UnexpectedToken(
            self.current.kind.clone(),
            self.current.line,
            self.current.col,
        ))
    }

    pub(crate) fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        if self.current.kind == kind {
            self.advance()
        } else {
            Err(ParseError::UnexpectedToken(
                self.current.kind.clone(),
                self.current.line,
                self.current.col,
            ))
        }
    }

    pub(crate) fn expect_identifier(&mut self) -> Result<String, ParseError> {
        let tok = self.advance()?;
        let name = match tok.kind {
            TokenKind::Ident(ref id) => {
                if tok.has_escape && crate::lexer::is_reserved_word(id) {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::Ident(id.clone()),
                        tok.line,
                        tok.col,
                    ));
                }
                if (self.in_async || self.is_module || self.in_static_block) && id == "await" {
                    return Err(ParseError::UnexpectedToken(TokenKind::Await, tok.line, tok.col));
                }
                if (self.in_generator || self.is_strict) && id == "yield" {
                    return Err(ParseError::UnexpectedToken(TokenKind::Yield, tok.line, tok.col));
                }
                if self.is_strict
                    && (is_strict_reserved_word(id) || id == "eval" || id == "arguments")
                {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::Ident(id.clone()),
                        tok.line,
                        tok.col,
                    ));
                }
                id.clone()
            }
            TokenKind::Undefined => "undefined".to_string(),
            TokenKind::As => "as".to_string(),
            TokenKind::From => "from".to_string(),
            TokenKind::Async => "async".to_string(),
            TokenKind::Await => {
                if self.is_module || self.in_async || self.in_static_block {
                    return Err(ParseError::UnexpectedToken(TokenKind::Await, tok.line, tok.col));
                }
                "await".to_string()
            }
            TokenKind::Yield => {
                if self.is_strict || self.in_generator {
                    return Err(ParseError::UnexpectedToken(TokenKind::Yield, tok.line, tok.col));
                }
                "yield".to_string()
            }
            TokenKind::Implements
            | TokenKind::Interface
            | TokenKind::Package
            | TokenKind::Private
            | TokenKind::Protected
            | TokenKind::Public
            | TokenKind::Static => {
                if self.is_strict {
                    return Err(ParseError::UnexpectedToken(tok.kind, tok.line, tok.col));
                }
                tok.kind.as_prop_name().unwrap_or_default()
            }
            TokenKind::Let => {
                if self.is_strict {
                    return Err(ParseError::UnexpectedToken(tok.kind, tok.line, tok.col));
                }
                "let".to_string()
            }
            other => return Err(ParseError::UnexpectedToken(other, tok.line, tok.col)),
        };
        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    #[test]
    fn test_parse_binary_precedence() {
        let mut parser = Parser::new("1 + 2 * 3").unwrap();
        let expr = parser.parse_expression(0).unwrap();
        assert_eq!(
            expr,
            Expression::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expression::Literal(Literal::Number(1.0))),
                right: Box::new(Expression::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(Expression::Literal(Literal::Number(2.0))),
                    right: Box::new(Expression::Literal(Literal::Number(3.0))),
                }),
            }
        );
    }

    #[test]
    fn test_parse_function_and_call() {
        let mut parser = Parser::new("function add(a, b) { return a + b; } add(10, 20);").unwrap();
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.body.len(), 2);
    }

    #[test]
    fn test_parse_object_and_array() {
        let mut parser = Parser::new("let obj = { x: 1, y: [10, 20] };").unwrap();
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.body.len(), 1);
    }

    #[test]
    fn test_parse_for_and_ternary() {
        let mut parser =
            Parser::new("for (let i = 0; i < 10; i = i + 1) { let res = i > 5 ? 1 : 0; }").unwrap();
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.body.len(), 1);
    }

    #[test]
    fn test_parse_async_and_modules() {
        let code = "async function fetchData() { let res = await p; return res; } import { a, b as c } from 'module'; export default 42; new Promise(fn);";
        let mut parser = Parser::new_module(code).unwrap();
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.body.len(), 4);
    }

    #[test]
    fn test_parse_default_parameters() {
        let code = "function foo(a, b = 42) { return a + b; } const bar = (x = 10) => x * 2;";
        let mut parser = Parser::new(code).unwrap();
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.body.len(), 2);
    }

    #[test]
    fn test_is_strict_reserved_word() {
        // True cases
        let strict_reserved = [
            "implements",
            "interface",
            "let",
            "package",
            "private",
            "protected",
            "public",
            "static",
            "yield",
        ];
        for word in strict_reserved {
            assert!(is_strict_reserved_word(word));
        }

        // False cases
        let not_strict_reserved =
            ["eval", "arguments", "foo", "bar", "var", "const", "function", "await"];
        for word in not_strict_reserved {
            assert!(!is_strict_reserved_word(word));
        }
    }
}
