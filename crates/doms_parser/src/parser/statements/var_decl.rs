use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_var_decl(&mut self, kind: VarKind) -> Result<Statement, ParseError> {
        self.advance()?; // let / const / var
        let mut declarations = Vec::new();

        loop {
            if self.match_token(&TokenKind::LBracket)? {
                let tmp_name = format!("@destruct_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let mut destruct_decls = Vec::new();
                self.parse_array_destructuring_into(&tmp_name, &mut destruct_decls)?;
                if self.match_token(&TokenKind::Colon)? {
                    self.skip_type_annotation()?;
                }
                let init = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                for d in &destruct_decls {
                    if !d.name.starts_with('@')
                        && (kind == VarKind::Let || kind == VarKind::Const)
                        && d.name == "let"
                    {
                        return Err(ParseError::UnexpectedToken(
                            TokenKind::Let,
                            self.current.line,
                            self.current.col,
                        ));
                    }
                }
                declarations.push(VarDeclarator { name: tmp_name, init });
                declarations.extend(destruct_decls);
            } else if self.match_token(&TokenKind::LBrace)? {
                let tmp_name = format!("@destruct_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let mut destruct_decls = Vec::new();
                self.parse_object_destructuring_into(&tmp_name, &mut destruct_decls)?;
                if self.match_token(&TokenKind::Colon)? {
                    self.skip_type_annotation()?;
                }
                let init = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                for d in &destruct_decls {
                    if !d.name.starts_with('@')
                        && (kind == VarKind::Let || kind == VarKind::Const)
                        && d.name == "let"
                    {
                        return Err(ParseError::UnexpectedToken(
                            TokenKind::Let,
                            self.current.line,
                            self.current.col,
                        ));
                    }
                }
                declarations.push(VarDeclarator { name: tmp_name, init });
                declarations.extend(destruct_decls);
            } else {
                let name = self.expect_identifier()?;
                if (kind == VarKind::Let || kind == VarKind::Const) && name == "let" {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::Let,
                        self.current.line,
                        self.current.col,
                    ));
                }
                if self.match_token(&TokenKind::Colon)? {
                    self.skip_type_annotation()?;
                }
                let init = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                declarations.push(VarDeclarator { name, init });
            }

            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }

        self.consume_semicolon()?;
        Ok(Statement::VarDecl { kind, declarations })
    }

    pub(crate) fn parse_using_decl(&mut self) -> Result<Statement, ParseError> {
        let mut declarations = Vec::new();
        loop {
            let name = self.expect_identifier()?;
            if name == "let"
                || name == "using"
                || (name == "await" && (self.is_module || self.in_async || self.in_static_block))
            {
                return Err(ParseError::UnexpectedToken(
                    TokenKind::Ident(name),
                    self.current.line,
                    self.current.col,
                ));
            }
            if !self.match_token(&TokenKind::Eq)? {
                return Err(ParseError::General(
                    "Using declarations must have an initializer".to_string(),
                ));
            }
            let init = Some(self.parse_expression(1)?);
            declarations.push(VarDeclarator { name, init });
            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }
        self.consume_semicolon()?;
        Ok(Statement::VarDecl { kind: VarKind::Const, declarations })
    }
}
