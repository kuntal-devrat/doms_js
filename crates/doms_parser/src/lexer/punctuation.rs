//! Operator and punctuation token scanning.

use super::Lexer;
use super::context::is_expr_brace_context;
use super::token::{LexError, TokenKind};

impl<'a> Lexer<'a> {
    pub(super) fn read_punctuation_or_operator(
        &mut self,
        ch: char,
        line: usize,
        col: usize,
    ) -> Result<TokenKind, LexError> {
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
                let is_expr = if self.prev_token_kind == Some(TokenKind::RParen)
                    && !self.fn_expr_stack.is_empty()
                {
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
                } else if self.peek() == Some('.') {
                    // OptionalChainingPunctuator :: ?. [lookahead ∉ DecimalDigit]
                    if self.peek2().map_or(false, |c| c.is_ascii_digit()) {
                        TokenKind::Question
                    } else {
                        self.advance();
                        TokenKind::QuestionDot
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
        Ok(kind)
    }
}
