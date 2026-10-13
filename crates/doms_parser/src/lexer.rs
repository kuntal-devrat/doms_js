//! Lexer for JavaScript source code.

use num_bigint::BigInt;
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
    #[error("Invalid escape sequence at line {0}, col {1}")]
    InvalidEscape(usize, usize),
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
    BigInt(BigInt),
    StringLit { value: String, has_escape: bool, has_legacy_escape: bool, has_unpaired_surrogate: bool },
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
            TokenKind::PrivateIdent(s) => Some(format!("#{}", s)),
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

fn is_expr_brace_context(prev: &Option<TokenKind>) -> bool {
    match prev {
        Some(TokenKind::LParen)
        | Some(TokenKind::LBracket)
        | Some(TokenKind::Comma)
        | Some(TokenKind::Colon)
        | Some(TokenKind::Question)
        | Some(TokenKind::Eq)
        | Some(TokenKind::PlusEq)
        | Some(TokenKind::MinusEq)
        | Some(TokenKind::StarEq)
        | Some(TokenKind::SlashEq)
        | Some(TokenKind::PercentEq)
        | Some(TokenKind::StarStarEq)
        | Some(TokenKind::LtLtEq)
        | Some(TokenKind::GtGtEq)
        | Some(TokenKind::GtGtGtEq)
        | Some(TokenKind::AmpEq)
        | Some(TokenKind::PipeEq)
        | Some(TokenKind::CaretEq)
        | Some(TokenKind::AmpAmpEq)
        | Some(TokenKind::PipePipeEq)
        | Some(TokenKind::QuestionQuestionEq)
        | Some(TokenKind::Plus)
        | Some(TokenKind::Minus)
        | Some(TokenKind::Star)
        | Some(TokenKind::Slash)
        | Some(TokenKind::Percent)
        | Some(TokenKind::StarStar)
        | Some(TokenKind::Lt)
        | Some(TokenKind::LtEq)
        | Some(TokenKind::Gt)
        | Some(TokenKind::GtEq)
        | Some(TokenKind::LtLt)
        | Some(TokenKind::GtGt)
        | Some(TokenKind::GtGtGt)
        | Some(TokenKind::EqEq)
        | Some(TokenKind::EqEqEq)
        | Some(TokenKind::NotEq)
        | Some(TokenKind::NotEqEq)
        | Some(TokenKind::Amp)
        | Some(TokenKind::Pipe)
        | Some(TokenKind::Caret)
        | Some(TokenKind::AmpAmp)
        | Some(TokenKind::PipePipe)
        | Some(TokenKind::QuestionQuestion)
        | Some(TokenKind::Bang)
        | Some(TokenKind::Tilde)
        | Some(TokenKind::DotDotDot)
        | Some(TokenKind::Return)
        | Some(TokenKind::Throw)
        | Some(TokenKind::Yield)
        | Some(TokenKind::Await)
        | Some(TokenKind::Case)
        | Some(TokenKind::In)
        | Some(TokenKind::Instanceof)
        | Some(TokenKind::Typeof)
        | Some(TokenKind::Void)
        | Some(TokenKind::Delete) => true,
        _ => false,
    }
}

fn can_precede_slash_operator(prev: &Option<TokenKind>, last_rbrace_was_expr: bool) -> bool {
    match prev {
        Some(TokenKind::Ident(_))
        | Some(TokenKind::Number(_))
        | Some(TokenKind::BigInt(_))
        | Some(TokenKind::StringLit { .. })
        | Some(TokenKind::TemplateLiteral { .. })
        | Some(TokenKind::RParen)
        | Some(TokenKind::RBracket)
        | Some(TokenKind::This)
        | Some(TokenKind::Super)
        | Some(TokenKind::True)
        | Some(TokenKind::False)
        | Some(TokenKind::Null)
        | Some(TokenKind::Undefined)
        | Some(TokenKind::PlusPlus)
        | Some(TokenKind::MinusMinus)
        | Some(TokenKind::RegExp { .. })
        | Some(TokenKind::PrivateIdent(_)) => true,
        Some(TokenKind::RBrace) => last_rbrace_was_expr,
        _ => false,
    }
}

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

fn flush_surrogate(s: &mut String) -> bool {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len >= 3 && bytes[len - 3] == 0xED && (0xA0..=0xAF).contains(&bytes[len - 2]) && (0x80..=0xBF).contains(&bytes[len - 1]) {
        s.truncate(len - 3);
        s.push('\u{FFFD}');
        true
    } else {
        false
    }
}

fn push_unicode_code_unit(s: &mut String, code: u32) -> bool {
    if (0xD800..=0xDBFF).contains(&code) {
        let unp = flush_surrogate(s);
        unsafe {
            s.as_mut_vec().extend_from_slice(&[
                0xED,
                (0x80 | ((code >> 6) & 0x3F)) as u8,
                (0x80 | (code & 0x3F)) as u8,
            ]);
        }
        unp
    } else if (0xDC00..=0xDFFF).contains(&code) {
        let bytes = s.as_bytes();
        let len = bytes.len();
        if len >= 3 && bytes[len - 3] == 0xED && (0xA0..=0xAF).contains(&bytes[len - 2]) && (0x80..=0xBF).contains(&bytes[len - 1]) {
            let b1 = bytes[len - 2] as u32;
            let b2 = bytes[len - 1] as u32;
            let high = 0xD800 | ((b1 & 0x0F) << 6) | (b2 & 0x3F);
            s.truncate(len - 3);
            let combined = 0x10000 + ((high - 0xD800) << 10) + (code - 0xDC00);
            if let Some(ch) = char::from_u32(combined) {
                s.push(ch);
                return false;
            }
        }
        s.push('\u{FFFD}');
        true
    } else {
        let unp = flush_surrogate(s);
        if let Some(ch) = char::from_u32(code) {
            s.push(ch);
            unp
        } else {
            s.push('\u{FFFD}');
            true
        }
    }
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

    #[inline(always)]
    fn record_token(&mut self, kind: TokenKind) -> TokenKind {
        if kind == TokenKind::Function {
            let is_expr = is_expr_brace_context(&self.prev_token_kind);
            self.fn_expr_stack.push(is_expr);
        }
        if kind != TokenKind::RBrace {
            self.last_rbrace_was_expr = false;
        }
        self.prev_token_kind = Some(kind.clone());
        kind
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
                let mut seen_flags = [false; 26];
                while let Some(fc) = self.peek() {
                    if fc == '\\' {
                        return Err(LexError::UnexpectedChar('\\', line, col));
                    }
                    if fc.is_ascii_alphabetic() {
                        let idx = (fc as u8).wrapping_sub(b'a') as usize;
                        if !matches!(fc, 'd' | 'g' | 'i' | 'm' | 's' | 'u' | 'v' | 'y') || seen_flags[idx] {
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
                self.record_token(TokenKind::Eof);
                return Ok(tok);
            }
        };

        if ch == '/' && !can_precede_slash_operator(&self.prev_token_kind, self.last_rbrace_was_expr) {
            let kind = self.read_regexp(line, col)?;
            let kind = self.record_token(kind);
            return Ok(Token {
                kind,
                line,
                col,
                has_preceding_line_terminator,
            });
        }

        if is_identifier_start(ch) || ch == '\\' {
            let kind = self.read_identifier_or_keyword(line, col)?;
            let kind = self.record_token(kind);
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
                    let kind = self.record_token(TokenKind::PrivateIdent(name));
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
                let kind = self.record_token(kind);
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
                let kind = self.record_token(TokenKind::DotDotDot);
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
            let kind = self.record_token(kind);
            return Ok(Token {
                kind,
                line,
                col,
                has_preceding_line_terminator,
            });
        }

        if ch == '"' || ch == '\'' {
            let kind = self.read_string(ch, line, col)?;
            let kind = self.record_token(kind);
            return Ok(Token {
                kind,
                line,
                col,
                has_preceding_line_terminator,
            });
        }

        if ch == '`' {
            let kind = self.read_template_literal(line, col)?;
            let kind = self.record_token(kind);
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
            '{' => {
                let is_expr = if self.prev_token_kind == Some(TokenKind::RParen) && !self.fn_expr_stack.is_empty() {
                    self.fn_expr_stack.pop().unwrap_or(false)
                } else {
                    is_expr_brace_context(&self.prev_token_kind)
                };
                self.brace_stack.push(is_expr);
                TokenKind::LBrace
            }
            '}' => {
                let was_expr = self.brace_stack.pop().unwrap_or(false);
                self.last_rbrace_was_expr = was_expr;
                TokenKind::RBrace
            }
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

        let kind = self.record_token(kind);
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
                    let num = u64::from_str_radix(&hex_str, 16)
                        .map(|v| v as f64)
                        .unwrap_or(f64::NAN);
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
                    let num = u64::from_str_radix(&bin_str, 2)
                        .map(|v| v as f64)
                        .unwrap_or(f64::NAN);
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
                    let num = u64::from_str_radix(&oct_str, 8)
                        .map(|v| v as f64)
                        .unwrap_or(f64::NAN);
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

    fn read_string(&mut self, quote: char, line: usize, col: usize) -> Result<TokenKind, LexError> {
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

    fn read_template_literal(&mut self, line: usize, col: usize) -> Result<TokenKind, LexError> {
        self.advance(); // consume opening '`'
        let mut quasis = Vec::new();
        let mut raw_exprs = Vec::new();
        let mut current_quasi = String::new();

        while let Some(c) = self.advance() {
            if c == '`' {
                flush_surrogate(&mut current_quasi);
                quasis.push(current_quasi);
                return Ok(TokenKind::TemplateLiteral { quasis, raw_exprs });
            } else if c == '$' && self.peek() == Some('{') {
                self.advance(); // consume '{'
                flush_surrogate(&mut current_quasi);
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
                        'n' => { flush_surrogate(&mut current_quasi); current_quasi.push('\n'); }
                        'r' => { flush_surrogate(&mut current_quasi); current_quasi.push('\r'); }
                        't' => { flush_surrogate(&mut current_quasi); current_quasi.push('\t'); }
                        'b' => { flush_surrogate(&mut current_quasi); current_quasi.push('\x08'); }
                        'f' => { flush_surrogate(&mut current_quasi); current_quasi.push('\x0C'); }
                        'v' => { flush_surrogate(&mut current_quasi); current_quasi.push('\x0B'); }
                        '0' => { flush_surrogate(&mut current_quasi); current_quasi.push('\0'); }
                        '\\' => { flush_surrogate(&mut current_quasi); current_quasi.push('\\'); }
                        '\'' => { flush_surrogate(&mut current_quasi); current_quasi.push('\''); }
                        '"' => { flush_surrogate(&mut current_quasi); current_quasi.push('"'); }
                        '`' => { flush_surrogate(&mut current_quasi); current_quasi.push('`'); }
                        '$' => { flush_surrogate(&mut current_quasi); current_quasi.push('$'); }
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
                                push_unicode_code_unit(&mut current_quasi, code);
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
                                    push_unicode_code_unit(&mut current_quasi, code);
                                }
                            } else {
                                let mut hex = String::new();
                                for _ in 0..4 {
                                    if let Some(h) = self.advance() {
                                        if h.is_ascii_hexdigit() { hex.push(h); }
                                    }
                                }
                                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                    push_unicode_code_unit(&mut current_quasi, code);
                                }
                            }
                        }
                        other => {
                            flush_surrogate(&mut current_quasi);
                            current_quasi.push(other);
                        }
                    }
                }
            } else if c == '\r' {
                if self.peek() == Some('\n') {
                    self.advance();
                }
                flush_surrogate(&mut current_quasi);
                current_quasi.push('\n');
            } else {
                flush_surrogate(&mut current_quasi);
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
