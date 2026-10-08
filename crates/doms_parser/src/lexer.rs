//! Lexer for JavaScript source code.

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum LexError {
    #[error("Unexpected character '{0}' at line {1}, col {2}")]
    UnexpectedChar(char, usize, usize),
    #[error("Unterminated string literal at line {0}, col {1}")]
    UnterminatedString(usize, usize),
    #[error("Unterminated multi-line comment at line {0}, col {1}")]
    UnterminatedComment(usize, usize),
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
    Class,
    Extends,
    Super,
    Debugger,
    Enum,
    With,
    Implements,
    Interface,
    Package,
    Private,
    Protected,
    Public,
    Static,

    // Identifiers & Literals
    Ident(String),
    PrivateIdent(String),
    Number(f64),
    StringLit { value: String, has_escape: bool },
    TemplateLiteral { quasis: Vec<String>, raw_exprs: Vec<String> },
    RegExp { pattern: String, flags: String },

    // Operators & Punctuation
    Plus,
    PlusEq,
    PlusPlus,
    Minus,
    MinusEq,
    MinusMinus,
    Star,
    StarEq,
    StarStar,
    StarStarEq,
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
    LtLt,
    LtLtEq,
    Gt,
    GtEq,
    GtGt,
    GtGtGt,
    GtGtEq,
    GtGtGtEq,
    Bang,
    Tilde,
    AmpAmp,
    AmpAmpEq,
    PipePipe,
    PipePipeEq,
    Amp,
    AmpEq,
    Pipe,
    PipeEq,
    Caret,
    CaretEq,
    Question,
    QuestionQuestion,
    QuestionQuestionEq,
    Colon,
    FatArrow,
    DotDotDot,

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
            TokenKind::Class => Some("class".to_string()),
            TokenKind::Extends => Some("extends".to_string()),
            TokenKind::Super => Some("super".to_string()),
            TokenKind::Debugger => Some("debugger".to_string()),
            TokenKind::Enum => Some("enum".to_string()),
            TokenKind::With => Some("with".to_string()),
            TokenKind::Implements => Some("implements".to_string()),
            TokenKind::Interface => Some("interface".to_string()),
            TokenKind::Package => Some("package".to_string()),
            TokenKind::Private => Some("private".to_string()),
            TokenKind::Protected => Some("protected".to_string()),
            TokenKind::Public => Some("public".to_string()),
            TokenKind::Static => Some("static".to_string()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
    pub has_preceding_line_terminator: bool,
}

#[inline]
pub fn is_line_terminator(c: char) -> bool {
    c == '\n' || c == '\r' || c == '\u{2028}' || c == '\u{2029}'
}

#[inline]
pub fn is_ecma_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'
        | '\u{000B}'
        | '\u{000C}'
        | '\u{0020}'
        | '\u{00A0}'
        | '\u{FEFF}'
        | '\u{1680}'
        | '\u{2000}'..='\u{200A}'
        | '\u{202F}'
        | '\u{205F}'
        | '\u{3000}'
    )
}

#[inline]
pub fn is_identifier_start(c: char) -> bool {
    c == '$'
        || c == '_'
        || unicode_ident::is_xid_start(c)
        || matches!(
            c,
            '\u{1885}'..='\u{1886}'
                | '\u{2118}'
                | '\u{212E}'
                | '\u{309B}'..='\u{309C}'
                | '\u{0E33}'
                | '\u{0EB3}'
        )
}

#[inline]
pub fn is_identifier_part(c: char) -> bool {
    c == '$'
        || c == '_'
        || c == '\u{200C}'
        || c == '\u{200D}'
        || unicode_ident::is_xid_continue(c)
        || matches!(
            c,
            '\u{1885}'..='\u{1886}'
                | '\u{2118}'
                | '\u{212E}'
                | '\u{309B}'..='\u{309C}'
                | '\u{0E33}'
                | '\u{0EB3}'
                | '\u{FF9E}'..='\u{FF9F}'
                | '\u{00B7}'
                | '\u{0387}'
                | '\u{1369}'..='\u{1371}'
                | '\u{19DA}'
        )
}

pub fn is_reserved_word(s: &str) -> bool {
    matches!(
        s,
        "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "export"
            | "extends"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "enum"
            | "null"
            | "true"
            | "false"
    )
}

fn can_precede_slash_operator(prev: &Option<TokenKind>) -> bool {
    match prev {
        Some(TokenKind::Ident(_))
        | Some(TokenKind::Number(_))
        | Some(TokenKind::StringLit { .. })
        | Some(TokenKind::TemplateLiteral { .. })
        | Some(TokenKind::RParen)
        | Some(TokenKind::RBracket)
        | Some(TokenKind::This)
        | Some(TokenKind::True)
        | Some(TokenKind::False)
        | Some(TokenKind::Null)
        | Some(TokenKind::PlusPlus)
        | Some(TokenKind::MinusMinus) => true,
        _ => false,
    }
}

pub struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    cursor: usize,
    peeked: Option<char>,
    line: usize,
    col: usize,
    prev_token_kind: Option<TokenKind>,
    seen_line_terminator: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut lexer = Self {
            source,
            bytes: source.as_bytes(),
            cursor: 0,
            peeked: None,
            line: 1,
            col: 1,
            prev_token_kind: None,
            seen_line_terminator: false,
        };
        lexer.peeked = lexer.next_char_from_bytes();
        lexer
    }

    #[inline(always)]
    fn next_char_from_bytes(&mut self) -> Option<char> {
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
    fn peek2(&self) -> Option<char> {
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
    fn peek3(&self) -> Option<char> {
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
        if b2 < 128 {
            Some(b2 as char)
        } else {
            self.source[off..].chars().next()
        }
    }

    fn advance(&mut self) -> Option<char> {
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

    fn peek(&self) -> Option<char> {
        self.peeked
    }

    fn skip_whitespace_and_comments(&mut self) -> Result<(), LexError> {
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

    fn read_regexp(&mut self, line: usize, col: usize) -> Result<TokenKind, LexError> {
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
                while let Some(fc) = self.peek() {
                    if fc.is_ascii_alphabetic() {
                        flags.push(fc);
                        self.advance();
                    } else {
                        break;
                    }
                }
                return Ok(TokenKind::RegExp { pattern, flags });
            } else {
                pattern.push(c);
            }
        }
        Err(LexError::UnexpectedChar('/', line, col))
    }

    pub fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace_and_comments()?;

        let has_preceding_line_terminator = self.seen_line_terminator;
        self.seen_line_terminator = false;

        let line = self.line;
        let col = self.col;

        let ch = match self.peek() {
            Some(c) => c,
            None => {
                let tok = Token {
                    kind: TokenKind::Eof,
                    line,
                    col,
                    has_preceding_line_terminator,
                };
                self.prev_token_kind = Some(TokenKind::Eof);
                return Ok(tok);
            }
        };

        if ch == '/' && !can_precede_slash_operator(&self.prev_token_kind) {
            let kind = self.read_regexp(line, col)?;
            self.prev_token_kind = Some(kind.clone());
            return Ok(Token {
                kind,
                line,
                col,
                has_preceding_line_terminator,
            });
        }

        if is_identifier_start(ch) || ch == '\\' {
            let kind = self.read_identifier_or_keyword(line, col)?;
            self.prev_token_kind = Some(kind.clone());
            return Ok(Token {
                kind,
                line,
                col,
                has_preceding_line_terminator,
            });
        }

        if ch == '#' {
            self.advance(); // consume '#'
            if let Some(next_c) = self.peek() {
                if is_identifier_start(next_c) || next_c == '\\' {
                    let ident_token = self.read_identifier_or_keyword(line, col)?;
                    let name = ident_token.as_prop_name().unwrap_or_default();
                    let kind = TokenKind::PrivateIdent(name);
                    self.prev_token_kind = Some(kind.clone());
                    return Ok(Token {
                        kind,
                        line,
                        col,
                        has_preceding_line_terminator,
                    });
                }
            }
            return Err(LexError::UnexpectedChar('#', line, col));
        }

        if ch == '.' {
            let next_c = self.peek2();
            if next_c.is_some_and(|c| c.is_ascii_digit()) {
                let kind = self.read_number(line, col)?;
                self.prev_token_kind = Some(kind.clone());
                return Ok(Token {
                    kind,
                    line,
                    col,
                    has_preceding_line_terminator,
                });
            }
            if next_c == Some('.') && self.peek3() == Some('.') {
                self.advance(); // '.'
                self.advance(); // '.'
                self.advance(); // '.'
                let kind = TokenKind::DotDotDot;
                self.prev_token_kind = Some(kind.clone());
                return Ok(Token {
                    kind,
                    line,
                    col,
                    has_preceding_line_terminator,
                });
            }
        }

        if ch.is_ascii_digit() {
            let kind = self.read_number(line, col)?;
            self.prev_token_kind = Some(kind.clone());
            return Ok(Token {
                kind,
                line,
                col,
                has_preceding_line_terminator,
            });
        }

        if ch == '"' || ch == '\'' {
            let kind = self.read_string(ch, line, col)?;
            self.prev_token_kind = Some(kind.clone());
            return Ok(Token {
                kind,
                line,
                col,
                has_preceding_line_terminator,
            });
        }

        if ch == '`' {
            let kind = self.read_template_literal(line, col)?;
            self.prev_token_kind = Some(kind.clone());
            return Ok(Token {
                kind,
                line,
                col,
                has_preceding_line_terminator,
            });
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
                if self.peek() == Some('*') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::StarStarEq
                    } else {
                        TokenKind::StarStar
                    }
                } else if self.peek() == Some('=') {
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
            '^' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::CaretEq
                } else {
                    TokenKind::Caret
                }
            }
            '?' => {
                if self.peek() == Some('?') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::QuestionQuestionEq
                    } else {
                        TokenKind::QuestionQuestion
                    }
                } else {
                    TokenKind::Question
                }
            }
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
                } else if self.peek() == Some('<') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::LtLtEq
                    } else {
                        TokenKind::LtLt
                    }
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::GtEq
                } else if self.peek() == Some('>') {
                    self.advance();
                    if self.peek() == Some('>') {
                        self.advance();
                        if self.peek() == Some('=') {
                            self.advance();
                            TokenKind::GtGtGtEq
                        } else {
                            TokenKind::GtGtGt
                        }
                    } else if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::GtGtEq
                    } else {
                        TokenKind::GtGt
                    }
                } else {
                    TokenKind::Gt
                }
            }
            '&' => {
                if self.peek() == Some('&') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::AmpAmpEq
                    } else {
                        TokenKind::AmpAmp
                    }
                } else if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::AmpEq
                } else {
                    TokenKind::Amp
                }
            }
            '|' => {
                if self.peek() == Some('|') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenKind::PipePipeEq
                    } else {
                        TokenKind::PipePipe
                    }
                } else if self.peek() == Some('=') {
                    self.advance();
                    TokenKind::PipeEq
                } else {
                    TokenKind::Pipe
                }
            }
            _ => return Err(LexError::UnexpectedChar(ch, line, col)),
        };

        self.prev_token_kind = Some(kind.clone());
        Ok(Token {
            kind,
            line,
            col,
            has_preceding_line_terminator,
        })
    }

    fn read_identifier_or_keyword(&mut self, line: usize, col: usize) -> Result<TokenKind, LexError> {
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

    fn read_number(&mut self, line: usize, col: usize) -> Result<TokenKind, LexError> {
        let mut s = String::new();

        // Check for 0x (hex), 0b (bin), 0o (oct)
        if self.peek() == Some('0') {
            match self.peek2() {
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
                    return Ok(TokenKind::Number(num));
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
                    return Ok(TokenKind::Number(num));
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
                    return Ok(TokenKind::Number(num));
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
                if self.peek2().is_some_and(|next_c| next_c.is_ascii_alphabetic() || next_c == '$' || next_c == '_') {
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
        Ok(TokenKind::Number(num))
    }

    fn read_string(&mut self, quote: char, line: usize, col: usize) -> Result<TokenKind, LexError> {
        self.advance(); // consume opening quote
        let mut s = String::new();
        let mut has_escape = false;
        while let Some(c) = self.advance() {
            if is_line_terminator(c) {
                return Err(LexError::UnterminatedString(line, col));
            }
            if c == quote {
                return Ok(TokenKind::StringLit { value: s, has_escape });
            } else if c == '\\' {
                has_escape = true;
                if let Some(escaped) = self.advance() {
                    if is_line_terminator(escaped) {
                        // Line continuation: \ followed by line terminator
                        if escaped == '\r' && self.peek() == Some('\n') {
                            self.advance();
                        }
                        continue;
                    }
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

    fn read_template_literal(&mut self, line: usize, col: usize) -> Result<TokenKind, LexError> {
        self.advance(); // consume opening '`'
        let mut quasis = Vec::new();
        let mut raw_exprs = Vec::new();
        let mut current_quasi = String::new();

        while let Some(c) = self.advance() {
            if c == '`' {
                quasis.push(current_quasi);
                return Ok(TokenKind::TemplateLiteral { quasis, raw_exprs });
            } else if c == '$' && self.peek() == Some('{') {
                self.advance(); // consume '{'
                quasis.push(std::mem::take(&mut current_quasi));

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
                                expr_str.push(ec);
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
                if let Some(escaped) = self.advance() {
                    if is_line_terminator(escaped) {
                        if escaped == '\r' && self.peek() == Some('\n') {
                            self.advance();
                        }
                        continue;
                    }
                    match escaped {
                        'n' => current_quasi.push('\n'),
                        'r' => current_quasi.push('\r'),
                        't' => current_quasi.push('\t'),
                        'b' => current_quasi.push('\x08'),
                        'f' => current_quasi.push('\x0C'),
                        'v' => current_quasi.push('\x0B'),
                        '0' => current_quasi.push('\0'),
                        '\\' => current_quasi.push('\\'),
                        '\'' => current_quasi.push('\''),
                        '"' => current_quasi.push('"'),
                        '`' => current_quasi.push('`'),
                        '$' => current_quasi.push('$'),
                        'x' => {
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
                                    current_quasi.push(ch);
                                }
                            }
                        }
                        'u' => {
                            if self.peek() == Some('{') {
                                self.advance();
                                let mut hex = String::new();
                                while let Some(ch) = self.advance() {
                                    if ch == '}' { break; }
                                    if ch.is_ascii_hexdigit() { hex.push(ch); }
                                }
                                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                    if let Some(ch) = char::from_u32(code) {
                                        current_quasi.push(ch);
                                    }
                                }
                            } else {
                                let mut hex = String::new();
                                for _ in 0..4 {
                                    if let Some(h) = self.advance() {
                                        if h.is_ascii_hexdigit() { hex.push(h); }
                                    }
                                }
                                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                    if let Some(ch) = char::from_u32(code) {
                                        current_quasi.push(ch);
                                    }
                                }
                            }
                        }
                        other => {
                            current_quasi.push(other);
                        }
                    }
                }
            } else if c == '\r' {
                if self.peek() == Some('\n') {
                    self.advance();
                }
                current_quasi.push('\n');
            } else {
                current_quasi.push(c);
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
