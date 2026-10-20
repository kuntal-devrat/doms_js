//! Numeric literal scanning: decimal, hex, binary, octal, and BigInt.

use num_bigint::BigInt;

use super::Lexer;
use super::token::{LexError, TokenKind};

impl<'a> Lexer<'a> {
    pub(super) fn read_number(&mut self, line: usize, col: usize) -> Result<TokenKind, LexError> {
        let mut s = String::new();

        // Check for 0x (hex), 0b (bin), 0o (oct)
        if self.peek() == Some('0') {
            match self.peek2() {
                Some('x') | Some('X') => {
                    self.advance(); // '0'
                    self.advance(); // 'x'
                    let mut hex_str = String::new();
                    let mut last_was_underscore = false;
                    if self.peek() == Some('_') {
                        return Err(LexError::InvalidNumber(line, col));
                    }
                    while let Some(c) = self.peek() {
                        if c.is_ascii_hexdigit() {
                            hex_str.push(c);
                            self.advance();
                            last_was_underscore = false;
                        } else if c == '_' {
                            if last_was_underscore {
                                return Err(LexError::InvalidNumber(line, col));
                            }
                            self.advance();
                            last_was_underscore = true;
                        } else {
                            break;
                        }
                    }
                    if last_was_underscore || hex_str.is_empty() {
                        return Err(LexError::InvalidNumber(line, col));
                    }
                    if self.peek() == Some('n') {
                        self.advance(); // consume 'n'
                        let bi = BigInt::parse_bytes(hex_str.as_bytes(), 16).unwrap_or_default();
                        return Ok(TokenKind::BigInt(bi));
                    }
                    let num =
                        u64::from_str_radix(&hex_str, 16).map(|v| v as f64).unwrap_or(f64::NAN);
                    return Ok(TokenKind::Number(num));
                }
                Some('b') | Some('B') => {
                    self.advance(); // '0'
                    self.advance(); // 'b'
                    let mut bin_str = String::new();
                    let mut last_was_underscore = false;
                    if self.peek() == Some('_') {
                        return Err(LexError::InvalidNumber(line, col));
                    }
                    while let Some(c) = self.peek() {
                        if c == '0' || c == '1' {
                            bin_str.push(c);
                            self.advance();
                            last_was_underscore = false;
                        } else if c == '_' {
                            if last_was_underscore {
                                return Err(LexError::InvalidNumber(line, col));
                            }
                            self.advance();
                            last_was_underscore = true;
                        } else {
                            break;
                        }
                    }
                    if last_was_underscore || bin_str.is_empty() {
                        return Err(LexError::InvalidNumber(line, col));
                    }
                    if self.peek() == Some('n') {
                        self.advance(); // consume 'n'
                        let bi = BigInt::parse_bytes(bin_str.as_bytes(), 2).unwrap_or_default();
                        return Ok(TokenKind::BigInt(bi));
                    }
                    let num =
                        u64::from_str_radix(&bin_str, 2).map(|v| v as f64).unwrap_or(f64::NAN);
                    return Ok(TokenKind::Number(num));
                }
                Some('o') | Some('O') => {
                    self.advance(); // '0'
                    self.advance(); // 'o'
                    let mut oct_str = String::new();
                    let mut last_was_underscore = false;
                    if self.peek() == Some('_') {
                        return Err(LexError::InvalidNumber(line, col));
                    }
                    while let Some(c) = self.peek() {
                        if matches!(c, '0'..='7') {
                            oct_str.push(c);
                            self.advance();
                            last_was_underscore = false;
                        } else if c == '_' {
                            if last_was_underscore {
                                return Err(LexError::InvalidNumber(line, col));
                            }
                            self.advance();
                            last_was_underscore = true;
                        } else {
                            break;
                        }
                    }
                    if last_was_underscore || oct_str.is_empty() {
                        return Err(LexError::InvalidNumber(line, col));
                    }
                    if self.peek() == Some('n') {
                        self.advance(); // consume 'n'
                        let bi = BigInt::parse_bytes(oct_str.as_bytes(), 8).unwrap_or_default();
                        return Ok(TokenKind::BigInt(bi));
                    }
                    let num =
                        u64::from_str_radix(&oct_str, 8).map(|v| v as f64).unwrap_or(f64::NAN);
                    return Ok(TokenKind::Number(num));
                }
                _ => {}
            }
        }

        // Check if starts with 0
        if self.peek() == Some('0') {
            if let Some(next_c) = self.peek2() {
                if next_c.is_ascii_digit() {
                    // Legacy octal or non-octal decimal starting with '0' e.g. 010, 08, 09
                    if self.is_strict {
                        return Err(LexError::InvalidNumber(line, col));
                    }
                    // Non-strict: read digits
                    self.advance(); // consume '0'
                    let mut oct_digits = String::new();
                    let mut is_pure_octal = true;
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            if !matches!(c, '0'..='7') {
                                is_pure_octal = false;
                            }
                            oct_digits.push(c);
                            self.advance();
                        } else if c == '_' {
                            // Numeric separator is NEVER allowed in legacy octal or non-octal decimal!
                            return Err(LexError::InvalidNumber(line, col));
                        } else {
                            break;
                        }
                    }
                    // BigInt cannot have legacy octal or non-octal leading zeros: 01n, 08n are invalid!
                    if self.peek() == Some('n') {
                        return Err(LexError::InvalidNumber(line, col));
                    }
                    let num = if is_pure_octal {
                        u64::from_str_radix(&oct_digits, 8).map(|v| v as f64).unwrap_or(f64::NAN)
                    } else {
                        format!("0{}", oct_digits).parse::<f64>().unwrap_or(f64::NAN)
                    };
                    return Ok(TokenKind::Number(num));
                } else if next_c == '_' {
                    // 0_... is NEVER allowed! (0_0, 0_1, etc.)
                    return Err(LexError::InvalidNumber(line, col));
                }
            }
        }

        // Standard decimal, possibly starting with '.' or digits
        let mut has_dot = false;
        let mut has_exp = false;
        let mut last_was_underscore = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
                last_was_underscore = false;
            } else if c == '_' {
                if last_was_underscore || s.is_empty() {
                    return Err(LexError::InvalidNumber(line, col));
                }
                self.advance();
                last_was_underscore = true;
            } else if c == '.' && !has_dot && !has_exp {
                if last_was_underscore {
                    // 1_.2 is invalid!
                    return Err(LexError::InvalidNumber(line, col));
                }
                has_dot = true;
                s.push(c);
                self.advance();
                // 1._2 or 1._ is invalid!
                if self.peek() == Some('_') {
                    return Err(LexError::InvalidNumber(line, col));
                }
            } else if (c == 'e' || c == 'E') && !has_exp {
                if last_was_underscore {
                    // 1_e2 is invalid!
                    return Err(LexError::InvalidNumber(line, col));
                }
                has_exp = true;
                s.push(c);
                self.advance();
                if let Some(sign) = self.peek() {
                    if sign == '+' || sign == '-' {
                        s.push(sign);
                        self.advance();
                    }
                }
                // 1e_2 or 1e+_2 is invalid!
                if self.peek() == Some('_') {
                    return Err(LexError::InvalidNumber(line, col));
                }
            } else {
                break;
            }
        }

        if last_was_underscore {
            return Err(LexError::InvalidNumber(line, col));
        }

        // BigInt literal suffix 'n'
        if self.peek() == Some('n') {
            self.advance();
            if has_dot || has_exp {
                return Err(LexError::InvalidNumber(line, col));
            }
            if s.len() > 1 && s.starts_with('0') {
                return Err(LexError::InvalidNumber(line, col));
            }
            if let Some(next_c) = self.peek() {
                if next_c.is_alphabetic() || next_c == '_' || next_c == '$' || next_c > '\x7f' {
                    return Err(LexError::InvalidNumber(line, col));
                }
            }
            let bi = BigInt::parse_bytes(s.as_bytes(), 10).unwrap_or_default();
            return Ok(TokenKind::BigInt(bi));
        }

        let num = s.parse::<f64>().map_err(|_| LexError::InvalidNumber(line, col))?;
        if let Some(next_c) = self.peek() {
            if next_c.is_alphabetic() || next_c == '_' || next_c == '$' || next_c > '\x7f' {
                return Err(LexError::InvalidNumber(line, col));
            }
        }
        Ok(TokenKind::Number(num))
    }
}
