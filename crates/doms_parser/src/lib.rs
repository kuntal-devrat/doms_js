//! # DOMS Parser
//!
//! Lexer, AST definitions, and Pratt parser for JavaScript in the DOMS engine.

#![allow(clippy::all)]

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use lexer::{LexError, Lexer, Token, TokenKind};
pub use parser::{ParseError, Parser};

/// Parse a JavaScript source code string into an AST [`Program`].
pub fn parse(source: &str) -> Result<Program, ParseError> {
    let mut parser = Parser::new(source)?;
    parser.parse_program()
}

/// Parse a JavaScript module source code string into an AST [`Program`].
pub fn parse_module(source: &str) -> Result<Program, ParseError> {
    let mut parser = Parser::new_module(source)?;
    parser.parse_program()
}

/// Parse a JavaScript source code string into an AST [`Program`] in strict mode.
pub fn parse_strict(source: &str) -> Result<Program, ParseError> {
    let mut parser = Parser::new_strict(source)?;
    parser.parse_program()
}

/// Parse an eval JavaScript source code string into an AST [`Program`].
pub fn parse_eval(
    source: &str,
    is_strict: bool,
    in_method: bool,
    in_function: bool,
) -> Result<Program, ParseError> {
    let mut parser = Parser::new_eval(source, is_strict, in_method, in_function)?;
    parser.parse_program()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_script() {
        let code = "let x = 10; let y = 20; let z = x + y;";
        let prog = parse(code).unwrap();
        assert_eq!(prog.body.len(), 3);
    }

    #[test]
    fn test_yield_star_ast() {
        let code = "class A {\n  *g() {\n    yield\n    * 1\n  }\n}";
        let res = parse(code);
        assert!(res.is_err());
    }
}
