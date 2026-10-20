//! Regular expression literal scanning.

use super::Lexer;
use super::chars::is_line_terminator;
use super::token::{LexError, TokenKind};

impl<'a> Lexer<'a> {
    pub(super) fn read_regexp(&mut self, line: usize, col: usize) -> Result<TokenKind, LexError> {
        self.advance(); // consume opening '/'
        let mut pattern = String::new();
        let mut in_class = false;
        while let Some(c) = self.advance() {
            if is_line_terminator(c) {
                return Err(LexError::UnexpectedChar(c, line, col));
            }
            if c == '\\' {
                pattern.push('\\');
                if let Some(esc) = self.advance() {
                    if is_line_terminator(esc) {
                        return Err(LexError::UnexpectedChar(esc, line, col));
                    }
                    pattern.push(esc);
                }
            } else if c == '[' {
                in_class = true;
                pattern.push('[');
            } else if c == ']' {
                in_class = false;
                pattern.push(']');
            } else if c == '/' && !in_class {
                // End of regex pattern
                let mut flags = String::new();
                let mut seen_flags = [false; 26];
                while let Some(fc) = self.peek() {
                    if fc == '\\' {
                        return Err(LexError::UnexpectedChar('\\', line, col));
                    }
                    if fc.is_ascii_alphabetic() {
                        let idx = (fc as u8).wrapping_sub(b'a') as usize;
                        if !matches!(fc, 'd' | 'g' | 'i' | 'm' | 's' | 'u' | 'v' | 'y')
                            || seen_flags[idx]
                        {
                            return Err(LexError::UnexpectedChar(fc, line, col));
                        }
                        seen_flags[idx] = true;
                        flags.push(fc);
                        self.advance();
                    } else if unicode_ident::is_xid_continue(fc) || fc == '$' || fc == '_' {
                        return Err(LexError::UnexpectedChar(fc, line, col));
                    } else {
                        break;
                    }
                }
                if regress::Regex::with_flags(&pattern, flags.as_str()).is_err() {
                    return Err(LexError::UnexpectedChar('/', line, col));
                }
                return Ok(TokenKind::RegExp { pattern, flags });
            } else {
                pattern.push(c);
            }
        }
        Err(LexError::UnexpectedChar('/', line, col))
    }
}
