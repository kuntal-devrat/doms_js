use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_export_statement(&mut self, is_top: bool) -> Result<Statement, ParseError> {
        let export_tok = self.advance()?; // export
        if !self.is_module || !is_top {
            return Err(ParseError::UnexpectedToken(
                TokenKind::Export,
                export_tok.line,
                export_tok.col,
            ));
        }

        if self.match_token(&TokenKind::Star)? {
            let all_as = if self.match_token(&TokenKind::As)? {
                Some(self.expect_export_name()?)
            } else {
                None
            };
            self.expect(TokenKind::From)?;
            let src_tok = self.advance()?;
            let source = match src_tok.kind {
                TokenKind::StringLit { value, .. } => value,
                other => return Err(ParseError::UnexpectedToken(other, src_tok.line, src_tok.col)),
            };
            self.consume_semicolon()?;
            return Ok(Statement::Export {
                decl: None,
                is_default: false,
                specifiers: Vec::new(),
                source: Some(source),
                is_all: true,
                all_as,
            });
        }

        if self.match_token(&TokenKind::LBrace)? {
            let mut specifiers = Vec::new();
            let mut has_string_local = false;
            while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
                if matches!(&self.current.kind, TokenKind::StringLit { .. }) {
                    has_string_local = true;
                }
                let name = self.expect_export_name()?;
                let alias = if self.match_token(&TokenKind::As)? {
                    Some(self.expect_export_name()?)
                } else {
                    None
                };
                specifiers.push((name, alias));
                if !self.match_token(&TokenKind::Comma)? {
                    break;
                }
            }
            self.expect(TokenKind::RBrace)?;
            let source = if self.match_token(&TokenKind::From)? {
                let src_tok = self.advance()?;
                match src_tok.kind {
                    TokenKind::StringLit { value, .. } => Some(value),
                    other => {
                        return Err(ParseError::UnexpectedToken(other, src_tok.line, src_tok.col));
                    }
                }
            } else {
                None
            };
            if source.is_none() && has_string_local {
                return Err(ParseError::General(
                    "Export specifier cannot be a string literal without a from clause".to_string(),
                ));
            }
            self.consume_semicolon()?;
            return Ok(Statement::Export {
                decl: None,
                is_default: false,
                specifiers,
                source,
                is_all: false,
                all_as: None,
            });
        }

        if self.match_token(&TokenKind::Default)? {
            if self.check(&TokenKind::Class) {
                let decl = self.parse_class_decl(true)?;
                return Ok(Statement::Export {
                    decl: Some(Box::new(decl)),
                    is_default: true,
                    specifiers: Vec::new(),
                    source: None,
                    is_all: false,
                    all_as: None,
                });
            } else if self.check(&TokenKind::Function) {
                self.advance()?; // function
                let is_generator = self.match_token(&TokenKind::Star)?;
                let prev_gen = self.in_generator;
                if is_generator {
                    self.in_generator = true;
                }
                let (name, params, body) = self.parse_function_signature_and_body_opt_name(true)?;
                self.in_generator = prev_gen;
                let decl = if is_generator {
                    Statement::GeneratorDecl { name, params, body }
                } else {
                    Statement::FunctionDecl { name, params, body }
                };
                return Ok(Statement::Export {
                    decl: Some(Box::new(decl)),
                    is_default: true,
                    specifiers: Vec::new(),
                    source: None,
                    is_all: false,
                    all_as: None,
                });
            } else if (self.check(&TokenKind::Async) || self.check_ident("async"))
                && !self.peeked.has_preceding_line_terminator
                && self.peeked.kind == TokenKind::Function
            {
                self.advance()?; // async
                self.expect(TokenKind::Function)?;
                let is_generator = self.match_token(&TokenKind::Star)?;
                let prev_gen = self.in_generator;
                let prev_async = self.in_async;
                self.in_async = true;
                if is_generator {
                    self.in_generator = true;
                }
                let (name, params, body) = self.parse_function_signature_and_body_opt_name(true)?;
                self.in_generator = prev_gen;
                self.in_async = prev_async;
                let decl = if is_generator {
                    Statement::AsyncGeneratorDecl { name, params, body }
                } else {
                    Statement::AsyncFunctionDecl { name, params, body }
                };
                return Ok(Statement::Export {
                    decl: Some(Box::new(decl)),
                    is_default: true,
                    specifiers: Vec::new(),
                    source: None,
                    is_all: false,
                    all_as: None,
                });
            } else {
                let expr = self.parse_expression(1)?;
                self.consume_semicolon()?;
                return Ok(Statement::Export {
                    decl: Some(Box::new(Statement::Expression(expr))),
                    is_default: true,
                    specifiers: Vec::new(),
                    source: None,
                    is_all: false,
                    all_as: None,
                });
            }
        }

        let stmt = self.parse_statement()?;
        Ok(Statement::Export {
            decl: Some(Box::new(stmt)),
            is_default: false,
            specifiers: Vec::new(),
            source: None,
            is_all: false,
            all_as: None,
        })
    }
}
