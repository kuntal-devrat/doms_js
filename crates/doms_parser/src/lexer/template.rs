//! Template literal scanning, including embedded expressions.

use super::Lexer;
use super::chars::is_line_terminator;
use super::token::{LexError, TokenKind};
use super::unicode::{flush_surrogate, push_unicode_code_unit};

impl<'a> Lexer<'a> {
    pub(super) fn read_template_literal(
        &mut self,
        line: usize,
        col: usize,
    ) -> Result<TokenKind, LexError> {
        self.advance(); // consume opening '`'
        let mut quasis: Vec<Option<String>> = Vec::new();
        let mut raw_quasis: Vec<String> = Vec::new();
        let mut raw_exprs: Vec<String> = Vec::new();
        let mut current_quasi = String::new();
        let mut current_raw = String::new();
        let mut current_quasi_valid = true;

        while let Some(c) = self.advance() {
            if c == '`' {
                flush_surrogate(&mut current_quasi);
                quasis.push(if current_quasi_valid { Some(current_quasi) } else { None });
                raw_quasis.push(current_raw);
                return Ok(TokenKind::TemplateLiteral { quasis, raw_quasis, raw_exprs });
            } else if c == '$' && self.peek() == Some('{') {
                self.advance(); // consume '{'
                flush_surrogate(&mut current_quasi);
                quasis.push(if current_quasi_valid {
                    Some(std::mem::take(&mut current_quasi))
                } else {
                    current_quasi.clear();
                    None
                });
                raw_quasis.push(std::mem::take(&mut current_raw));
                current_quasi_valid = true;

                let mut expr_str = String::new();
                let mut brace_depth = 1;
                while let Some(ec) = self.advance() {
                    match ec {
                        '{' => {
                            brace_depth += 1;
                            expr_str.push(ec);
                        }
                        '}' => {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                break;
                            }
                            expr_str.push(ec);
                        }
                        '\'' | '"' => {
                            expr_str.push(ec);
                            while let Some(sc) = self.advance() {
                                expr_str.push(sc);
                                if sc == '\\' {
                                    if let Some(esc) = self.advance() {
                                        expr_str.push(esc);
                                    }
                                } else if sc == ec {
                                    break;
                                }
                            }
                        }
                        '`' => {
                            expr_str.push(ec);
                            while let Some(tc) = self.advance() {
                                expr_str.push(tc);
                                if tc == '\\' {
                                    if let Some(esc) = self.advance() {
                                        expr_str.push(esc);
                                    }
                                } else if tc == '`' {
                                    break;
                                }
                            }
                        }
                        '/' => {
                            if self.peek() == Some('/') {
                                self.advance();
                                expr_str.push('/');
                                expr_str.push('/');
                                while let Some(cc) = self.advance() {
                                    expr_str.push(cc);
                                    if is_line_terminator(cc) {
                                        break;
                                    }
                                }
                            } else if self.peek() == Some('*') {
                                self.advance();
                                expr_str.push('/');
                                expr_str.push('*');
                                while let Some(cc) = self.advance() {
                                    expr_str.push(cc);
                                    if cc == '*' && self.peek() == Some('/') {
                                        self.advance();
                                        expr_str.push('/');
                                        break;
                                    }
                                }
                            } else {
                                let trimmed = expr_str.trim_end();
                                let is_regex = if trimmed.is_empty() {
                                    true
                                } else {
                                    let last_char = trimmed.chars().last().unwrap();
                                    match last_char {
                                        '(' | '[' | '{' | ',' | ';' | ':' | '?' | '=' | '!'
                                        | '+' | '-' | '*' | '/' | '%' | '&' | '|' | '^' | '~'
                                        | '<' | '>' => true,
                                        _ => {
                                            trimmed.ends_with("return")
                                                || trimmed.ends_with("case")
                                                || trimmed.ends_with("delete")
                                                || trimmed.ends_with("throw")
                                                || trimmed.ends_with("void")
                                                || trimmed.ends_with("typeof")
                                                || trimmed.ends_with("yield")
                                                || trimmed.ends_with("await")
                                        }
                                    }
                                };
                                if is_regex {
                                    expr_str.push(ec);
                                    let mut in_char_class = false;
                                    while let Some(rc) = self.advance() {
                                        expr_str.push(rc);
                                        if rc == '\\' {
                                            if let Some(esc) = self.advance() {
                                                expr_str.push(esc);
                                            }
                                        } else if rc == '[' {
                                            in_char_class = true;
                                        } else if rc == ']' {
                                            in_char_class = false;
                                        } else if rc == '/' && !in_char_class {
                                            while let Some(fc) = self.peek() {
                                                if fc.is_ascii_alphabetic() {
                                                    expr_str.push(fc);
                                                    self.advance();
                                                } else {
                                                    break;
                                                }
                                            }
                                            break;
                                        }
                                    }
                                } else {
                                    expr_str.push(ec);
                                }
                            }
                        }
                        _ => {
                            expr_str.push(ec);
                        }
                    }
                }
                if brace_depth != 0 {
                    return Err(LexError::UnterminatedString(line, col));
                }
                raw_exprs.push(expr_str);
            } else if c == '\\' {
                current_raw.push('\\');
                if let Some(escaped) = self.advance() {
                    current_raw.push(escaped);
                    if is_line_terminator(escaped) {
                        if escaped == '\r' && self.peek() == Some('\n') {
                            self.advance();
                            current_raw.push('\n');
                        }
                        continue;
                    }
                    match escaped {
                        'n' => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('\n');
                            }
                        }
                        'r' => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('\r');
                            }
                        }
                        't' => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('\t');
                            }
                        }
                        'b' => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('\x08');
                            }
                        }
                        'f' => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('\x0C');
                            }
                        }
                        'v' => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('\x0B');
                            }
                        }
                        '0' => {
                            if self.peek().map_or(false, |next| next.is_ascii_digit()) {
                                current_quasi_valid = false;
                            } else if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('\0');
                            }
                        }
                        '1'..='9' => {
                            current_quasi_valid = false;
                        }
                        '\\' => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('\\');
                            }
                        }
                        '\'' => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('\'');
                            }
                        }
                        '"' => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('"');
                            }
                        }
                        '`' => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('`');
                            }
                        }
                        '$' => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push('$');
                            }
                        }
                        'x' => {
                            let mut hex = String::new();
                            for _ in 0..2 {
                                if let Some(h) = self.peek() {
                                    if h.is_ascii_hexdigit() {
                                        self.advance();
                                        current_raw.push(h);
                                        hex.push(h);
                                    } else {
                                        break;
                                    }
                                }
                            }
                            if hex.len() == 2 && current_quasi_valid {
                                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                    push_unicode_code_unit(&mut current_quasi, code);
                                } else {
                                    current_quasi_valid = false;
                                }
                            } else {
                                current_quasi_valid = false;
                            }
                        }
                        'u' => {
                            if self.peek() == Some('{') {
                                self.advance();
                                current_raw.push('{');
                                let mut hex = String::new();
                                let mut closed = false;
                                while let Some(ch) = self.peek() {
                                    if ch == '}' {
                                        self.advance();
                                        current_raw.push('}');
                                        closed = true;
                                        break;
                                    } else if ch.is_ascii_hexdigit() {
                                        self.advance();
                                        current_raw.push(ch);
                                        hex.push(ch);
                                    } else {
                                        break;
                                    }
                                }
                                if closed && !hex.is_empty() && current_quasi_valid {
                                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                        if code <= 0x10_FFFF {
                                            push_unicode_code_unit(&mut current_quasi, code);
                                        } else {
                                            current_quasi_valid = false;
                                        }
                                    } else {
                                        current_quasi_valid = false;
                                    }
                                } else {
                                    current_quasi_valid = false;
                                }
                            } else {
                                let mut hex = String::new();
                                for _ in 0..4 {
                                    if let Some(h) = self.peek() {
                                        if h.is_ascii_hexdigit() {
                                            self.advance();
                                            current_raw.push(h);
                                            hex.push(h);
                                        } else {
                                            break;
                                        }
                                    }
                                }
                                if hex.len() == 4 && current_quasi_valid {
                                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                        push_unicode_code_unit(&mut current_quasi, code);
                                    } else {
                                        current_quasi_valid = false;
                                    }
                                } else {
                                    current_quasi_valid = false;
                                }
                            }
                        }
                        other => {
                            if current_quasi_valid {
                                flush_surrogate(&mut current_quasi);
                                current_quasi.push(other);
                            }
                        }
                    }
                }
            } else if c == '\r' {
                if self.peek() == Some('\n') {
                    self.advance();
                }
                current_raw.push('\n');
                if current_quasi_valid {
                    flush_surrogate(&mut current_quasi);
                    current_quasi.push('\n');
                }
            } else {
                current_raw.push(c);
                if current_quasi_valid {
                    flush_surrogate(&mut current_quasi);
                    current_quasi.push(c);
                }
            }
        }
        Err(LexError::UnterminatedString(line, col))
    }
}
