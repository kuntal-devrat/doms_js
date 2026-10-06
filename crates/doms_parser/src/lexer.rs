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
    For,
    Break,
    Continue,
    Try,
    Catch,
    Finally,
    Throw,
    New,
    This,
    Return,
    True,
    False,
    Null,
    Undefined,
    Typeof,
    Async,
    Await,
    Yield,
    Import,
    Export,
    From,
    As,
    Default,
    Instanceof,
    In,
    Switch,
    Case,
    Do,
    Void,
    Delete,

    // Identifiers & Literals
    Ident(String),
    Number(f64),
    StringLit(String),

    // Operators & Punctuation
    Plus,
    PlusEq,
    PlusPlus,
    Minus,
    MinusEq,
    MinusMinus,
    Star,
    StarEq,
    Slash,
    SlashEq,
    Percent,
    PercentEq,
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
    Question,
    Colon,
    FatArrow,

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

impl TokenKind {
    pub fn as_prop_name(&self) -> Option<String> {
        match self {
            TokenKind::Ident(s) => Some(s.clone()),
            TokenKind::Catch => Some("catch".to_string()),
            TokenKind::Finally => Some("finally".to_string()),
            TokenKind::Default => Some("default".to_string()),
            TokenKind::New => Some("new".to_string()),
            TokenKind::This => Some("this".to_string()),
            TokenKind::Return => Some("return".to_string()),
            TokenKind::Let => Some("let".to_string()),
            TokenKind::Const => Some("const".to_string()),
            TokenKind::Var => Some("var".to_string()),
            TokenKind::Function => Some("function".to_string()),
            TokenKind::If => Some("if".to_string()),
            TokenKind::Else => Some("else".to_string()),
            TokenKind::While => Some("while".to_string()),
            TokenKind::For => Some("for".to_string()),
            TokenKind::Break => Some("break".to_string()),
            TokenKind::Continue => Some("continue".to_string()),
            TokenKind::Try => Some("try".to_string()),
            TokenKind::Throw => Some("throw".to_string()),
            TokenKind::Typeof => Some("typeof".to_string()),
            TokenKind::Async => Some("async".to_string()),
            TokenKind::Await => Some("await".to_string()),
            TokenKind::Yield => Some("yield".to_string()),
            TokenKind::Import => Some("import".to_string()),
            TokenKind::Export => Some("export".to_string()),
            TokenKind::From => Some("from".to_string()),
            TokenKind::As => Some("as".to_string()),
            TokenKind::True => Some("true".to_string()),
            TokenKind::False => Some("false".to_string()),
            TokenKind::Null => Some("null".to_string()),
            TokenKind::Undefined => Some("undefined".to_string()),
            TokenKind::Instanceof => Some("instanceof".to_string()),
            TokenKind::In => Some("in".to_string()),
            TokenKind::Switch => Some("switch".to_string()),
            TokenKind::Case => Some("case".to_string()),
            TokenKind::Do => Some("do".to_string()),
            TokenKind::Void => Some("void".to_string()),
            TokenKind::Delete => Some("delete".to_string()),
            _ => None,
        }
    }
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

        if ch == '.' {
            let mut lookahead = self.chars.clone();
            if lookahead.next().is_some_and(|c| c.is_ascii_digit()) {
                return self.read_number(line, col);
            }
        }

        if ch.is_ascii_digit() {
            return self.read_number(line, col);
        }

        if ch == '"' || ch == '\'' {
            return self.read_string(ch, line, col);
        }

        self.advance();

        let kind = match ch {
            '+' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::PlusEq
                } else if self.peek() == Some('+') {
                    self.advance();
                    TokenKind::PlusPlus
                } else {
                    TokenKind::Plus
                }
            }
            '-' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::MinusEq
                } else if self.peek() == Some('-') {
                    self.advance();
                    TokenKind::MinusMinus
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::StarEq
                } else {
                    TokenKind::Star
                }
            }
            '/' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::SlashEq
                } else {
                    TokenKind::Slash
                }
            }
            '%' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::PercentEq
                } else {
                    TokenKind::Percent
                }
            }
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
            '?' => TokenKind::Question,
            ':' => TokenKind::Colon,
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::EqEqEq
                    } else {
                        TokenKind::EqEq
                    }
                } else if self.peek() == Some('>') {
                    self.advance();
                    TokenKind::FatArrow
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
            _ => TokenKind::Ident(s),
        };

        Ok(Token { kind, line, col })
    }

    fn read_number(&mut self, line: usize, col: usize) -> Result<Token, LexError> {
        let mut s = String::new();

        // Check for 0x (hex), 0b (bin), 0o (oct)
        if self.peek() == Some('0') {
            let mut lookahead = self.chars.clone();
            match lookahead.next() {
                Some('x') | Some('X') => {
                    self.advance(); // '0'
                    self.advance(); // 'x'
                    let mut hex_str = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_hexdigit() {
                            hex_str.push(c);
                            self.advance();
                        } else if c == '_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    if self.peek() == Some('n') {
                        self.advance(); // consume 'n'
                    }
                    let num = u64::from_str_radix(&hex_str, 16)
                        .map(|v| v as f64)
                        .unwrap_or(f64::NAN);
                    return Ok(Token { kind: TokenKind::Number(num), line, col });
                }
                Some('b') | Some('B') => {
                    self.advance(); // '0'
                    self.advance(); // 'b'
                    let mut bin_str = String::new();
                    while let Some(c) = self.peek() {
                        if c == '0' || c == '1' {
                            bin_str.push(c);
                            self.advance();
                        } else if c == '_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    if self.peek() == Some('n') {
                        self.advance(); // consume 'n'
                    }
                    let num = u64::from_str_radix(&bin_str, 2)
                        .map(|v| v as f64)
                        .unwrap_or(f64::NAN);
                    return Ok(Token { kind: TokenKind::Number(num), line, col });
                }
                Some('o') | Some('O') => {
                    self.advance(); // '0'
                    self.advance(); // 'o'
                    let mut oct_str = String::new();
                    while let Some(c) = self.peek() {
                        if matches!(c, '0'..='7') {
                            oct_str.push(c);
                            self.advance();
                        } else if c == '_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    if self.peek() == Some('n') {
                        self.advance(); // consume 'n'
                    }
                    let num = u64::from_str_radix(&oct_str, 8)
                        .map(|v| v as f64)
                        .unwrap_or(f64::NAN);
                    return Ok(Token { kind: TokenKind::Number(num), line, col });
                }
                _ => {}
            }
        }

        // Standard decimal, possibly starting with '.' or digits
        let mut has_dot = false;
        let mut has_exp = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else if c == '.' && !has_dot && !has_exp {
                // Peek ahead to ensure it's not a method call like 42.toString()
                let mut lookahead = self.chars.clone();
                if lookahead.next().is_some_and(|next_c| next_c.is_ascii_alphabetic() || next_c == '$' || next_c == '_') {
                    // It's followed by an identifier (e.g. 42.toString), stop before the dot
                    break;
                }
                has_dot = true;
                s.push(c);
                self.advance();
            } else if (c == 'e' || c == 'E') && !has_exp {
                has_exp = true;
                s.push(c);
                self.advance();
                if let Some(sign) = self.peek() {
                    if sign == '+' || sign == '-' {
                        s.push(sign);
                        self.advance();
                    }
                }
            } else if c == '_' {
                self.advance(); // skip numeric separator
            } else {
                break;
            }
        }

        // BigInt literal suffix 'n'
        if self.peek() == Some('n') {
            self.advance();
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
                        'b' => s.push('\x08'),
                        'f' => s.push('\x0C'),
                        'v' => s.push('\x0B'),
                        '0' => s.push('\0'),
                        '\\' => s.push('\\'),
                        '\'' => s.push('\''),
                        '"' => s.push('"'),
                        '\n' => {} // line continuation
                        'x' => {
                            // 2 hex digits
                            let mut hex = String::new();
                            for _ in 0..2 {
                                if let Some(h) = self.advance() {
                                    if h.is_ascii_hexdigit() {
                                        hex.push(h);
                                    }
                                }
                            }
                            if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                if let Some(ch) = char::from_u32(code) {
                                    s.push(ch);
                                }
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
                                    }
                                }
                                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                    if let Some(ch) = char::from_u32(code) {
                                        s.push(ch);
                                    }
                                }
                            } else {
                                // 4 hex digits
                                let mut hex = String::new();
                                for _ in 0..4 {
                                    if let Some(h) = self.advance() {
                                        if h.is_ascii_hexdigit() {
                                            hex.push(h);
                                        }
                                    }
                                }
                                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                    if let Some(ch) = char::from_u32(code) {
                                        s.push(ch);
                                    }
                                }
                            }
                        }
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
