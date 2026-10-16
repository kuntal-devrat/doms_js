use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_import_statement(&mut self, is_top: bool) -> Result<Statement, ParseError> {
        let import_tok = self.advance()?; // import
        if !self.is_module || !is_top {
            return Err(ParseError::UnexpectedToken(TokenKind::Import, import_tok.line, import_tok.col));
        }

        // Bare import: import "module-name";
        if let TokenKind::StringLit { value, has_unpaired_surrogate, .. } = &self.current.kind {
            if *has_unpaired_surrogate {
                return Err(ParseError::General("Import name contains unpaired surrogate".to_string()));
            }
            let source = value.clone();
            self.advance()?;
            self.consume_semicolon()?;
            return Ok(Statement::Import { specifiers: Vec::new(), source, is_default: false, is_all: false });
        }

        let mut specifiers = Vec::new();
        let mut is_default = false;
        let mut is_all = false;

        if let TokenKind::Ident(default_name) = &self.current.kind {
            let name = default_name.clone();
            self.advance()?;
            specifiers.push(("default".to_string(), Some(name)));
            is_default = true;
            if self.match_token(&TokenKind::Comma)? {
                // optional comma after default
            }
        }

        if self.match_token(&TokenKind::Star)? {
            self.expect(TokenKind::As)?;
            let ns = self.expect_identifier()?;
            specifiers.push(("*".to_string(), Some(ns)));
            is_all = true;
        } else if self.match_token(&TokenKind::LBrace)? {
            while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
                let is_string_name = matches!(&self.current.kind, TokenKind::StringLit { .. });
                let name = self.expect_export_name()?;
                let alias = if self.match_token(&TokenKind::As)? {
                    Some(self.expect_identifier()?)
                } else {
                    if is_string_name {
                        return Err(ParseError::General("String literal import must have an 'as' clause".to_string()));
                    }
                    if is_strict_reserved_word(&name) {
                        return Err(ParseError::UnexpectedToken(TokenKind::Ident(name), self.current.line, self.current.col));
                    }
                    None
                };
                specifiers.push((name, alias));
                if !self.match_token(&TokenKind::Comma)? {
                    break;
                }
            }
            self.expect(TokenKind::RBrace)?;
        }

        self.expect(TokenKind::From)?;
        let src_tok = self.advance()?;
        let source = match src_tok.kind {
            TokenKind::StringLit { value, .. } => value,
            other => return Err(ParseError::UnexpectedToken(other, src_tok.line, src_tok.col)),
        };
        self.consume_semicolon()?;

        let mut bound_names = std::collections::HashSet::new();
        for (name, alias) in &specifiers {
            let bound = alias.as_ref().unwrap_or(name);
            if bound == "eval" || bound == "arguments" {
                return Err(ParseError::UnexpectedToken(TokenKind::Ident(bound.clone()), self.current.line, self.current.col));
            }
            if !bound_names.insert(bound.clone()) {
                return Err(ParseError::General(format!("Duplicate import binding '{}'", bound)));
            }
        }

        Ok(Statement::Import { specifiers, source, is_default, is_all })
    }
}
