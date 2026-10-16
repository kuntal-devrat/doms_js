//! Main scan loop: token dispatch, recording, and tokenize_all.

use super::chars::is_identifier_start;
use super::context::{can_precede_slash_operator, is_expr_brace_context};
use super::token::{LexError, Token, TokenKind};
use super::Lexer;

impl<'a> Lexer<'a> {
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

        let kind = self.read_punctuation_or_operator(ch, line, col)?;

        let kind = self.record_token(kind);
        Ok(Token {
            kind,
            line,
            col,
            has_preceding_line_terminator,
        })
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
