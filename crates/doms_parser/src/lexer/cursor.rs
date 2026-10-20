//! Cursor movement, peeking, and whitespace/comment skipping.

use super::Lexer;
use super::chars::{is_ecma_whitespace, is_line_terminator};
use super::token::LexError;

impl<'a> Lexer<'a> {
    #[inline(always)]
    pub(super) fn next_char_from_bytes(&mut self) -> Option<char> {
        if self.cursor >= self.bytes.len() {
            return None;
        }
        let b = self.bytes[self.cursor];
        if b < 128 {
            self.cursor += 1;
            return Some(b as char);
        }
        let rest = &self.source[self.cursor..];
        let c = rest.chars().next()?;
        self.cursor += c.len_utf8();
        Some(c)
    }

    #[inline(always)]
    pub(super) fn peek2(&self) -> Option<char> {
        if self.cursor >= self.bytes.len() {
            return None;
        }
        let b = self.bytes[self.cursor];
        if b < 128 {
            return Some(b as char);
        }
        self.source[self.cursor..].chars().next()
    }

    #[inline(always)]
    pub(super) fn peek3(&self) -> Option<char> {
        let mut off = self.cursor;
        if off >= self.bytes.len() {
            return None;
        }
        let b1 = self.bytes[off];
        off += if b1 < 128 { 1 } else { self.source[off..].chars().next()?.len_utf8() };
        if off >= self.bytes.len() {
            return None;
        }
        let b2 = self.bytes[off];
        if b2 < 128 { Some(b2 as char) } else { self.source[off..].chars().next() }
    }

    pub(super) fn advance(&mut self) -> Option<char> {
        let curr = self.peeked;
        self.peeked = self.next_char_from_bytes();
        if let Some(c) = curr {
            if c == '\r' && self.peeked == Some('\n') {
                self.peeked = self.next_char_from_bytes();
                self.line += 1;
                self.col = 1;
            } else if is_line_terminator(c) {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        curr
    }

    pub(super) fn peek(&self) -> Option<char> {
        self.peeked
    }

    pub(super) fn skip_whitespace_and_comments(&mut self) -> Result<(), LexError> {
        // Hashbang comment on line 1, col 1
        if self.line == 1 && self.col == 1 && self.peek() == Some('#') {
            if self.peek2() == Some('!') {
                self.advance(); // '#'
                self.advance(); // '!'
                while let Some(ch) = self.peek() {
                    if is_line_terminator(ch) {
                        break;
                    }
                    self.advance();
                }
            }
        }

        while let Some(c) = self.peek() {
            if is_line_terminator(c) {
                self.seen_line_terminator = true;
                self.advance();
            } else if is_ecma_whitespace(c) {
                self.advance();
            } else if c == '/' {
                match self.peek2() {
                    Some('/') => {
                        // Line comment terminates on any LineTerminator
                        self.advance(); // '/'
                        self.advance(); // '/'
                        while let Some(ch) = self.peek() {
                            if is_line_terminator(ch) {
                                break;
                            }
                            self.advance();
                        }
                    }
                    Some('*') => {
                        // Block comment
                        let start_line = self.line;
                        let start_col = self.col;
                        self.advance(); // '/'
                        self.advance(); // '*'
                        let mut closed = false;
                        while let Some(ch) = self.advance() {
                            if is_line_terminator(ch) {
                                self.seen_line_terminator = true;
                            }
                            if ch == '*' && self.peek() == Some('/') {
                                self.advance(); // '/'
                                closed = true;
                                break;
                            }
                        }
                        if !closed {
                            return Err(LexError::UnterminatedComment(start_line, start_col));
                        }
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }
        Ok(())
    }
}
