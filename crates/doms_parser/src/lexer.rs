//! Lexer for JavaScript source code.

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum LexError {
    #[error("Unexpected character '{0}' at line {1}, col {2}")]
    UnexpectedChar(char, usize, usize),
    #[error("Unterminated string literal at line {0}, col {1}")]
    UnterminatedString(usize, usize),
    #[error("Invalid number literal at line {0}, col {1}")]
    InvalidNumber(usize, usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Let,
    Const,
    Var,
    Function,
    If,
    Else,
    While,
    Return,
    True,
    False,
    Null,
    Undefined,
    Typeof,

    // Identifiers & Literals
    Ident(String),
    Number(f64),
    StringLit(String),

    // Operators & Punctuation
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    EqEqEq,
    NotEq,
    NotEqEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Bang,
    Tilde,
    AmpAmp,
    PipePipe,
    Amp,
    Pipe,
    Caret,

    // Delimiters
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Semicolon,
    Comma,
    Dot,

    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

pub struct Lexer<'a> {
    chars: std::str::Chars<'a>,
    peeked: Option<char>,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut lexer = Self { chars: source.chars(), peeked: None, line: 1, col: 1 };
        lexer.peeked = lexer.chars.next();
        lexer
    }

    fn advance(&mut self) -> Option<char> {
        let curr = self.peeked;
        self.peeked = self.chars.next();
        if let Some(c) = curr {
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        curr
    }

    fn peek(&self) -> Option<char> {
        self.peeked
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else if c == '/' {
                let mut lookahead = self.chars.clone();
                match lookahead.next() {
                    Some('/') => {
                        // Line comment
                        self.advance();
                        self.advance();
                        while let Some(ch) = self.peek() {
                            self.advance();
                            if ch == '\n' {
                                break;
                            }
                        }
                    }
                    Some('*') => {
                        // Block comment
                        self.advance();
                        self.advance();
                        while let Some(ch) = self.advance() {
                            if ch == '*' && self.peek() == Some('/') {
                                self.advance();
                                break;
                            }
                        }
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace_and_comments();

        let line = self.line;
        let col = self.col;

        let ch = match self.peek() {
            Some(c) => c,
            None => return Ok(Token { kind: TokenKind::Eof, line, col }),
        };

        if ch.is_alphabetic() || ch == '_' || ch == '$' {
            return self.read_identifier_or_keyword(line, col);
        }

        if ch.is_ascii_digit() {
            return self.read_number(line, col);
        }

        if ch == '"' || ch == '\'' {
            return self.read_string(ch, line, col);
        }

        self.advance();

        let kind = match ch {
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            ';' => TokenKind::Semicolon,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            '~' => TokenKind::Tilde,
            '^' => TokenKind::Caret,
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::EqEqEq
                    } else {
                        TokenKind::EqEq
                    }
                } else {
                    TokenKind::Eq
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::NotEqEq
                    } else {
                        TokenKind::NotEq
                    }
                } else {
                    TokenKind::Bang
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }
            '&' => {
                if self.peek() == Some('&') {
                    self.advance();
                    TokenKind::AmpAmp
                } else {
                    TokenKind::Amp
                }
            }
            '|' => {
                if self.peek() == Some('|') {
                    self.advance();
                    TokenKind::PipePipe
                } else {
                    TokenKind::Pipe
                }
            }
            _ => return Err(LexError::UnexpectedChar(ch, line, col)),
        };

        Ok(Token { kind, line, col })
    }

    fn read_identifier_or_keyword(&mut self, line: usize, col: usize) -> Result<Token, LexError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '$' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let kind = match s.as_str() {
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "var" => TokenKind::Var,
            "function" => TokenKind::Function,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "undefined" => TokenKind::Undefined,
            "typeof" => TokenKind::Typeof,
            _ => TokenKind::Ident(s),
        };

        Ok(Token { kind, line, col })
    }

    fn read_number(&mut self, line: usize, col: usize) -> Result<Token, LexError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        let num = s.parse::<f64>().map_err(|_| LexError::InvalidNumber(line, col))?;
        Ok(Token { kind: TokenKind::Number(num), line, col })
    }

    fn read_string(&mut self, quote: char, line: usize, col: usize) -> Result<Token, LexError> {
        self.advance(); // consume opening quote
        let mut s = String::new();
        while let Some(c) = self.advance() {
            if c == quote {
                return Ok(Token { kind: TokenKind::StringLit(s), line, col });
            } else if c == '\\' {
                if let Some(escaped) = self.advance() {
                    match escaped {
                        'n' => s.push('\n'),
                        'r' => s.push('\r'),
                        't' => s.push('\t'),
                        '\\' => s.push('\\'),
                        '\'' => s.push('\''),
                        '"' => s.push('"'),
                        _ => s.push(escaped),
                    }
                }
            } else {
                s.push(c);
            }
        }
        Err(LexError::UnterminatedString(line, col))
    }

    pub fn tokenize_all(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_lexing() {
        let source = "let x = 42; const msg = \"hello\";";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize_all().unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Let);
        assert_eq!(tokens[1].kind, TokenKind::Ident("x".into()));
        assert_eq!(tokens[2].kind, TokenKind::Eq);
        assert_eq!(tokens[3].kind, TokenKind::Number(42.0));
        assert_eq!(tokens[4].kind, TokenKind::Semicolon);
    }
}
