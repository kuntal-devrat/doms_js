//! Lexer for JavaScript source code.

mod chars;
mod context;
mod cursor;
mod ident;
mod number;
mod punctuation;
mod regexp;
mod scan;
mod string;
mod template;
mod token;
mod unicode;

#[cfg(test)]
mod tests;

pub use chars::{is_ecma_whitespace, is_identifier_part, is_identifier_start, is_line_terminator, is_reserved_word};
pub use token::{LexError, Token, TokenKind};

#[derive(Clone)]
pub struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    cursor: usize,
    peeked: Option<char>,
    line: usize,
    col: usize,
    prev_token_kind: Option<TokenKind>,
    seen_line_terminator: bool,
    brace_stack: Vec<bool>,
    fn_expr_stack: Vec<bool>,
    last_rbrace_was_expr: bool,
    pub is_strict: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let is_strict = Self::detect_use_strict(source);
        let mut lexer = Self {
            source,
            bytes: source.as_bytes(),
            cursor: 0,
            peeked: None,
            line: 1,
            col: 1,
            prev_token_kind: None,
            seen_line_terminator: false,
            brace_stack: Vec::new(),
            fn_expr_stack: Vec::new(),
            last_rbrace_was_expr: false,
            is_strict,
        };
        lexer.peeked = lexer.next_char_from_bytes();
        lexer
    }

    pub fn new_strict(source: &'a str) -> Self {
        let mut lexer = Self::new(source);
        lexer.is_strict = true;
        lexer
    }

    fn detect_use_strict(source: &str) -> bool {
        let mut chars = source.chars().peekable();
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else if c == '/' {
                chars.next();
                if chars.peek() == Some(&'/') {
                    chars.next();
                    while let Some(ch) = chars.next() {
                        if ch == '\n' || ch == '\r' {
                            break;
                        }
                    }
                } else if chars.peek() == Some(&'*') {
                    chars.next();
                    let mut prev = ' ';
                    while let Some(ch) = chars.next() {
                        if prev == '*' && ch == '/' {
                            break;
                        }
                        prev = ch;
                    }
                } else {
                    return false;
                }
            } else {
                break;
            }
        }
        let rest: String = chars.take(15).collect();
        rest.starts_with("\"use strict\"") || rest.starts_with("'use strict'")
    }
}
