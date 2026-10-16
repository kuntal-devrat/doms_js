//! Identifier and keyword scanning.

use super::chars::{is_identifier_part, is_identifier_start, is_reserved_word};
use super::token::{LexError, TokenKind};
use super::Lexer;

impl<'a> Lexer<'a> {
    pub(super) fn read_identifier_or_keyword(&mut self, line: usize, col: usize) -> Result<TokenKind, LexError> {
        let mut s = String::new();
        let mut has_escape = false;
        while let Some(c) = self.peek() {
            if is_identifier_part(c) {
                if s.is_empty() && !is_identifier_start(c) {
                    break;
                }
                s.push(c);
                self.advance();
            } else if c == '\\' {
                if self.peek2() == Some('u') {
                    self.advance(); // consume '\'
                    self.advance(); // consume 'u'
                    has_escape = true;
                    let ch = if self.peek() == Some('{') {
                        self.advance(); // '{'
                        let mut hex = String::new();
                        let mut closed = false;
                        while let Some(hc) = self.advance() {
                            if hc == '}' {
                                closed = true;
                                break;
                            }
                            if hc.is_ascii_hexdigit() {
                                hex.push(hc);
                            } else {
                                return Err(LexError::UnexpectedChar(hc, line, col));
                            }
                        }
                        if !closed || hex.is_empty() {
                            return Err(LexError::UnexpectedChar('\\', line, col));
                        }
                        let code = u32::from_str_radix(&hex, 16)
                            .map_err(|_| LexError::UnexpectedChar('\\', line, col))?;
                        if code > 0x10FFFF {
                            return Err(LexError::UnexpectedChar('\\', line, col));
                        }
                        char::from_u32(code)
                            .ok_or_else(|| LexError::UnexpectedChar('\\', line, col))?
                    } else {
                        let mut hex = String::new();
                        for _ in 0..4 {
                            if let Some(hc) = self.advance() {
                                if hc.is_ascii_hexdigit() {
                                    hex.push(hc);
                                } else {
                                    return Err(LexError::UnexpectedChar(hc, line, col));
                                }
                            } else {
                                return Err(LexError::UnexpectedChar('\\', line, col));
                            }
                        }
                        let code = u32::from_str_radix(&hex, 16)
                            .map_err(|_| LexError::UnexpectedChar('\\', line, col))?;
                        char::from_u32(code)
                            .ok_or_else(|| LexError::UnexpectedChar('\\', line, col))?
                    };

                    let valid = if s.is_empty() {
                        is_identifier_start(ch)
                    } else {
                        is_identifier_part(ch)
                    };

                    if !valid {
                        return Err(LexError::UnexpectedChar(ch, line, col));
                    }
                    s.push(ch);
                } else {
                    self.advance();
                    return Err(LexError::UnexpectedChar('\\', line, col));
                }
            } else {
                break;
            }
        }

        if s.is_empty() {
            let ch = self.advance().unwrap_or('\\');
            return Err(LexError::UnexpectedChar(ch, line, col));
        }

        if has_escape {
            if is_reserved_word(&s) {
                return Err(LexError::UnexpectedChar('\\', line, col));
            }
            return Ok(TokenKind::Ident(s));
        }

        let kind = match s.as_str() {
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "var" => TokenKind::Var,
            "function" => TokenKind::Function,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "finally" => TokenKind::Finally,
            "throw" => TokenKind::Throw,
            "new" => TokenKind::New,
            "this" => TokenKind::This,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "undefined" => TokenKind::Undefined,
            "typeof" => TokenKind::Typeof,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "yield" => TokenKind::Yield,
            "import" => TokenKind::Import,
            "export" => TokenKind::Export,
            "from" => TokenKind::From,
            "as" => TokenKind::As,
            "default" => TokenKind::Default,
            "instanceof" => TokenKind::Instanceof,
            "in" => TokenKind::In,
            "switch" => TokenKind::Switch,
            "case" => TokenKind::Case,
            "do" => TokenKind::Do,
            "void" => TokenKind::Void,
            "delete" => TokenKind::Delete,
            "class" => TokenKind::Class,
            "extends" => TokenKind::Extends,
            "super" => TokenKind::Super,
            "debugger" => TokenKind::Debugger,
            "enum" => TokenKind::Enum,
            "with" => TokenKind::With,
            "implements" => TokenKind::Implements,
            "interface" => TokenKind::Interface,
            "package" => TokenKind::Package,
            "private" => TokenKind::Private,
            "protected" => TokenKind::Protected,
            "public" => TokenKind::Public,
            "static" => TokenKind::Static,
            _ => TokenKind::Ident(s),
        };

        Ok(kind)
    }
}
