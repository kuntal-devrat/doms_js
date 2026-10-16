//! String literal scanning, including escape sequences.

use super::token::{LexError, TokenKind};
use super::unicode::{flush_surrogate, push_unicode_code_unit};
use super::Lexer;

impl<'a> Lexer<'a> {
    pub(super) fn read_string(&mut self, quote: char, line: usize, col: usize) -> Result<TokenKind, LexError> {
        self.advance(); // consume opening quote
        let mut s = String::new();
        let mut has_escape = false;
        let mut has_legacy_escape = false;
        let mut has_unpaired_surrogate = false;
        while let Some(c) = self.advance() {
            if c == '\r' || c == '\n' {
                return Err(LexError::UnterminatedString(line, col));
            }
            if c == quote {
                if flush_surrogate(&mut s) {
                    has_unpaired_surrogate = true;
                }
                return Ok(TokenKind::StringLit { value: s, has_escape, has_legacy_escape, has_unpaired_surrogate });
            } else if c == '\\' {
                has_escape = true;
                if let Some(escaped) = self.advance() {
                    if escaped == '\r' || escaped == '\n' || escaped == '\u{2028}' || escaped == '\u{2029}' {
                        // Line continuation: \ followed by line terminator
                        if escaped == '\r' && self.peek() == Some('\n') {
                            self.advance();
                        }
                        continue;
                    }
                    match escaped {
                        'n' => { flush_surrogate(&mut s); s.push('\n'); }
                        'r' => { flush_surrogate(&mut s); s.push('\r'); }
                        't' => { flush_surrogate(&mut s); s.push('\t'); }
                        'b' => { flush_surrogate(&mut s); s.push('\x08'); }
                        'f' => { flush_surrogate(&mut s); s.push('\x0C'); }
                        'v' => { flush_surrogate(&mut s); s.push('\x0B'); }
                        '\\' => { flush_surrogate(&mut s); s.push('\\'); }
                        '\'' => { flush_surrogate(&mut s); s.push('\''); }
                        '"' => { flush_surrogate(&mut s); s.push('"'); }
                        'x' => {
                            // Exactly 2 hex digits required
                            let mut hex = String::new();
                            for _ in 0..2 {
                                match self.advance() {
                                    Some(h) if h.is_ascii_hexdigit() => hex.push(h),
                                    _ => return Err(LexError::InvalidEscape(line, col)),
                                }
                            }
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| LexError::InvalidEscape(line, col))?;
                            if push_unicode_code_unit(&mut s, code) {
                                has_unpaired_surrogate = true;
                            }
                        }
                        'u' => {
                            if self.peek() == Some('{') {
                                self.advance(); // '{'
                                let mut hex = String::new();
                                while let Some(ch) = self.advance() {
                                    if ch == '}' {
                                        break;
                                    }
                                    if ch.is_ascii_hexdigit() {
                                        hex.push(ch);
                                    } else {
                                        return Err(LexError::InvalidEscape(line, col));
                                    }
                                }
                                if hex.is_empty() || hex.len() > 6 {
                                    return Err(LexError::InvalidEscape(line, col));
                                }
                                let code = u32::from_str_radix(&hex, 16)
                                    .map_err(|_| LexError::InvalidEscape(line, col))?;
                                if code > 0x10FFFF {
                                    return Err(LexError::InvalidEscape(line, col));
                                }
                                if push_unicode_code_unit(&mut s, code) {
                                    has_unpaired_surrogate = true;
                                }
                            } else {
                                // Exactly 4 hex digits required
                                let mut hex = String::new();
                                for _ in 0..4 {
                                    match self.advance() {
                                        Some(h) if h.is_ascii_hexdigit() => hex.push(h),
                                        _ => return Err(LexError::InvalidEscape(line, col)),
                                    }
                                }
                                let code = u32::from_str_radix(&hex, 16)
                                    .map_err(|_| LexError::InvalidEscape(line, col))?;
                                if push_unicode_code_unit(&mut s, code) {
                                    has_unpaired_surrogate = true;
                                }
                            }
                        }
                        '0' => {
                            if let Some(next_c) = self.peek() {
                                if next_c.is_ascii_digit() {
                                    if self.is_strict {
                                        return Err(LexError::InvalidEscape(line, col));
                                    }
                                    has_legacy_escape = true;
                                    if ('0'..='7').contains(&next_c) {
                                        let mut oct = next_c as u32 - '0' as u32;
                                        self.advance();
                                        if let Some(next_c2) = self.peek() {
                                            if ('0'..='7').contains(&next_c2) {
                                                oct = oct * 8 + (next_c2 as u32 - '0' as u32);
                                                self.advance();
                                            }
                                        }
                                        flush_surrogate(&mut s);
                                        s.push(char::from_u32(oct).unwrap_or('\0'));
                                    } else {
                                        // e.g. \08 or \09: \0 followed by 8 or 9
                                        flush_surrogate(&mut s);
                                        s.push('\0');
                                    }
                                } else {
                                    flush_surrogate(&mut s);
                                    s.push('\0');
                                }
                            } else {
                                flush_surrogate(&mut s);
                                s.push('\0');
                            }
                        }
                        '1'..='7' => {
                            if self.is_strict {
                                return Err(LexError::InvalidEscape(line, col));
                            }
                            has_legacy_escape = true;
                            let mut oct = escaped as u32 - '0' as u32;
                            for _ in 0..2 {
                                if let Some(c) = self.peek() {
                                    if ('0'..='7').contains(&c) {
                                        let next_oct = oct * 8 + (c as u32 - '0' as u32);
                                        if next_oct <= 255 {
                                            oct = next_oct;
                                            self.advance();
                                        } else {
                                            break;
                                        }
                                    } else {
                                        break;
                                    }
                                }
                            }
                            flush_surrogate(&mut s);
                            s.push(char::from_u32(oct).unwrap_or(escaped));
                        }
                        '8' | '9' => {
                            if self.is_strict {
                                return Err(LexError::InvalidEscape(line, col));
                            }
                            has_legacy_escape = true;
                            flush_surrogate(&mut s);
                            s.push(escaped);
                        }
                        _ => {
                            flush_surrogate(&mut s);
                            s.push(escaped);
                        }
                    }
                }
            } else {
                flush_surrogate(&mut s);
                s.push(c);
            }
        }
        Err(LexError::UnterminatedString(line, col))
    }
}
