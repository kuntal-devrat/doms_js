//! Native TypeScript syntax support (type annotations, type aliases, interfaces, type assertions).

use super::{ParseError, Parser};
use crate::lexer::TokenKind;

impl<'a> Parser<'a> {
    /// Skip a TypeScript type annotation following a `:` delimiter.
    ///
    /// Balances `(...)`, `[...]`, `{...}`, and `<...>` and stops at statement/expression
    /// terminators (`=`, `,`, `;`, `)`, `}`, `=>`, or EOF).
    pub(crate) fn skip_type_annotation(&mut self) -> Result<(), ParseError> {
        let mut paren_depth = 0;
        let mut bracket_depth = 0;
        let mut brace_depth = 0;
        let mut angle_depth = 0;
        let mut token_count = 0;

        while self.current.kind != TokenKind::Eof {
            if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 && angle_depth == 0 {
                if token_count > 0 && self.current.kind == TokenKind::LBrace {
                    break;
                }
                match self.current.kind {
                    TokenKind::Eq
                    | TokenKind::Comma
                    | TokenKind::Semicolon
                    | TokenKind::RParen
                    | TokenKind::RBrace
                    | TokenKind::FatArrow => {
                        break;
                    }
                    _ => {}
                }
            }

            match &self.current.kind {
                TokenKind::LParen => paren_depth += 1,
                TokenKind::RParen => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    } else {
                        break;
                    }
                }
                TokenKind::LBracket => bracket_depth += 1,
                TokenKind::RBracket => {
                    if bracket_depth > 0 {
                        bracket_depth -= 1;
                    } else {
                        break;
                    }
                }
                TokenKind::LBrace => brace_depth += 1,
                TokenKind::RBrace => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                    } else {
                        break;
                    }
                }
                TokenKind::Lt => angle_depth += 1,
                TokenKind::Gt => {
                    if angle_depth > 0 {
                        angle_depth -= 1;
                    }
                }
                _ => {}
            }
            token_count += 1;
            self.advance()?;
        }
        Ok(())
    }

    /// Skip generic type parameters `<T, U extends V>`
    pub(crate) fn skip_type_parameters(&mut self) -> Result<(), ParseError> {
        if !self.check(&TokenKind::Lt) {
            return Ok(());
        }
        self.advance()?; // '<'
        let mut angle_depth = 1;
        let mut paren_depth = 0;
        let mut bracket_depth = 0;
        let mut brace_depth = 0;

        while self.current.kind != TokenKind::Eof && angle_depth > 0 {
            match &self.current.kind {
                TokenKind::Lt => angle_depth += 1,
                TokenKind::Gt => angle_depth -= 1,
                TokenKind::LParen => paren_depth += 1,
                TokenKind::RParen => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                }
                TokenKind::LBracket => bracket_depth += 1,
                TokenKind::RBracket => {
                    if bracket_depth > 0 {
                        bracket_depth -= 1;
                    }
                }
                TokenKind::LBrace => brace_depth += 1,
                TokenKind::RBrace => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                    }
                }
                _ => {}
            }
            self.advance()?;
        }
        Ok(())
    }

    /// Check if a token is a TypeScript accessibility or member modifier.
    pub(crate) fn is_ts_modifier(kind: &TokenKind) -> bool {
        matches!(kind, TokenKind::Public | TokenKind::Private | TokenKind::Protected)
            || matches!(
                kind,
                TokenKind::Ident(id) if id == "public" || id == "private" || id == "protected" || id == "readonly" || id == "override"
            )
    }

    /// Parse `type Alias = ...;` or `interface Name { ... }`
    pub(crate) fn parse_typescript_type_or_interface_decl(&mut self) -> Result<(), ParseError> {
        let is_interface = self.check(&TokenKind::Interface)
            || matches!(&self.current.kind, TokenKind::Ident(id) if id == "interface");
        self.advance()?; // skip 'type' or 'interface'
        let _name = self.expect_identifier()?;
        if self.check(&TokenKind::Lt) {
            self.skip_type_parameters()?;
        }

        if is_interface {
            // Optional 'extends Base'
            if self.check(&TokenKind::Extends)
                || matches!(&self.current.kind, TokenKind::Ident(id) if id == "extends")
            {
                while !self.check(&TokenKind::LBrace) && !self.check(&TokenKind::Eof) {
                    self.advance()?;
                }
            }
            if self.check(&TokenKind::LBrace) {
                self.skip_balanced(TokenKind::LBrace, TokenKind::RBrace)?;
            }
            let _ = self.consume_semicolon();
        } else {
            // type Alias = ...;
            if self.match_token(&TokenKind::Eq)? {
                self.skip_type_annotation()?;
            }
            let _ = self.consume_semicolon();
        }
        Ok(())
    }
}
