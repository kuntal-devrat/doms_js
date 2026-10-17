use super::*;

fn prepend_to_body(body: Box<Statement>, mut stmts_to_prepend: Vec<Statement>) -> Box<Statement> {
    if stmts_to_prepend.is_empty() {
        return body;
    }
    match *body {
        Statement::Block(mut existing) => {
            stmts_to_prepend.append(&mut existing);
            Box::new(Statement::Block(stmts_to_prepend))
        }
        other => {
            stmts_to_prepend.push(other);
            Box::new(Statement::Block(stmts_to_prepend))
        }
    }
}

impl<'a> Parser<'a> {
    pub(crate) fn parse_for_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // for
        let is_await = if self.match_token(&TokenKind::Await)? {
            if !self.in_async && !self.is_module {
                return Err(ParseError::UnexpectedToken(TokenKind::Await, self.current.line, self.current.col));
            }
            true
        } else {
            false
        };
        self.expect(TokenKind::LParen)?;

        if self.check(&TokenKind::Async)
            && matches!(&self.peeked.kind, TokenKind::Ident(s) if s == "of")
            && !self.peeked.has_escape
        {
            return Err(ParseError::UnexpectedToken(TokenKind::Ident("async".into()), self.current.line, self.current.col));
        }

        if self.check(&TokenKind::Let) || self.check(&TokenKind::Const) || self.check(&TokenKind::Var) {
            let kind_tok = self.advance()?; // let/const/var
            let var_kind = match kind_tok.kind {
                TokenKind::Let => Some(VarKind::Let),
                TokenKind::Const => Some(VarKind::Const),
                _ => Some(VarKind::Var),
            };

            if self.check(&TokenKind::LBracket) || self.check(&TokenKind::LBrace) {
                let tmp_name = format!("@destruct_for_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let mut destruct_decls = Vec::new();
                let is_bracket = self.match_token(&TokenKind::LBracket)?;
                if is_bracket {
                    self.parse_array_destructuring_into(&tmp_name, &mut destruct_decls)?;
                } else {
                    self.advance()?; // LBrace
                    let obj_tmp = format!("@destruct_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    destruct_decls.push(VarDeclarator {
                        name: obj_tmp.clone(),
                        init: Some(Expression::Identifier(tmp_name.clone())),
                    });
                    self.parse_object_destructuring_into(&obj_tmp, &mut destruct_decls)?;
                }
                let bound_names: Vec<String> = destruct_decls
                    .iter()
                    .filter(|d| !d.name.starts_with('@'))
                    .map(|d| d.name.clone())
                    .collect();

                if var_kind == Some(VarKind::Let) || var_kind == Some(VarKind::Const) {
                    let mut seen = std::collections::HashSet::new();
                    for name in &bound_names {
                        if !seen.insert(name) {
                            return Err(ParseError::General(format!("Duplicate lexical binding '{}' in for loop head", name)));
                        }
                        if name == "let" {
                            return Err(ParseError::UnexpectedToken(TokenKind::Let, self.current.line, self.current.col));
                        }
                    }
                }

                if self.match_token(&TokenKind::In)? {
                    if is_await {
                        return Err(ParseError::UnexpectedToken(TokenKind::In, self.current.line, self.current.col));
                    }
                    let right = self.parse_expression(0)?;
                    self.expect(TokenKind::RParen)?;
                    let body = Box::new(self.parse_statement()?);
                    self.check_sub_statement(&body, false)?;

                    if var_kind == Some(VarKind::Let) || var_kind == Some(VarKind::Const) {
                        let mut body_vars = Vec::new();
                        Parser::collect_var_declared_names(&body, &mut body_vars);
                        for name in &bound_names {
                            if body_vars.contains(name) {
                                return Err(ParseError::General(format!("Identifier '{}' has already been declared", name)));
                            }
                        }
                    }

                    let body = prepend_to_body(body, vec![Statement::VarDecl { kind: var_kind.unwrap(), declarations: destruct_decls }]);
                    return Ok(Statement::ForIn { var_kind, left: tmp_name, right, body });
                }
                if self.check_ident("of") {
                    self.advance()?; // of
                    let right = self.parse_expression(1)?;
                    self.expect(TokenKind::RParen)?;
                    let body = Box::new(self.parse_statement()?);
                    self.check_sub_statement(&body, false)?;

                    if var_kind == Some(VarKind::Let) || var_kind == Some(VarKind::Const) {
                        let mut body_vars = Vec::new();
                        Parser::collect_var_declared_names(&body, &mut body_vars);
                        for name in &bound_names {
                            if body_vars.contains(name) {
                                return Err(ParseError::General(format!("Identifier '{}' has already been declared", name)));
                            }
                        }
                    }

                    let body = prepend_to_body(body, vec![Statement::VarDecl { kind: var_kind.unwrap(), declarations: destruct_decls }]);
                    return Ok(Statement::ForOf { var_kind, left: tmp_name, right, body, is_await });
                }
                if is_await {
                    return Err(ParseError::UnexpectedToken(self.current.kind.clone(), self.current.line, self.current.col));
                }
                let kind = var_kind.unwrap();
                let init_val = if self.match_token(&TokenKind::Eq)? {
                    let old_in = self.allow_in;
                    self.allow_in = false;
                    let val = self.parse_expression(1);
                    self.allow_in = old_in;
                    Some(val?)
                } else {
                    None
                };
                let mut declarations = vec![VarDeclarator { name: tmp_name, init: init_val }];
                declarations.extend(destruct_decls);
                while self.match_token(&TokenKind::Comma)? {
                    if self.match_token(&TokenKind::LBracket)? {
                        let next_tmp = format!("@destruct_for_{}", self.destruct_counter);
                        self.destruct_counter += 1;
                        let mut next_decls = Vec::new();
                        self.parse_array_destructuring_into(&next_tmp, &mut next_decls)?;
                        let iv = if self.match_token(&TokenKind::Eq)? {
                            let old_in = self.allow_in;
                            self.allow_in = false;
                            let val = self.parse_expression(1);
                            self.allow_in = old_in;
                            Some(val?)
                        } else {
                            None
                        };
                        declarations.push(VarDeclarator { name: next_tmp, init: iv });
                        declarations.extend(next_decls);
                    } else if self.match_token(&TokenKind::LBrace)? {
                        let next_tmp = format!("@destruct_for_{}", self.destruct_counter);
                        self.destruct_counter += 1;
                        let mut next_decls = Vec::new();
                        self.parse_object_destructuring_into(&next_tmp, &mut next_decls)?;
                        let iv = if self.match_token(&TokenKind::Eq)? {
                            let old_in = self.allow_in;
                            self.allow_in = false;
                            let val = self.parse_expression(1);
                            self.allow_in = old_in;
                            Some(val?)
                        } else {
                            None
                        };
                        declarations.push(VarDeclarator { name: next_tmp, init: iv });
                        declarations.extend(next_decls);
                    } else {
                        let n = self.expect_identifier()?;
                        let iv = if self.match_token(&TokenKind::Eq)? {
                            let old_in = self.allow_in;
                            self.allow_in = false;
                            let val = self.parse_expression(1);
                            self.allow_in = old_in;
                            Some(val?)
                        } else {
                            None
                        };
                        declarations.push(VarDeclarator { name: n, init: iv });
                    }
                }
                self.expect(TokenKind::Semicolon)?;
                let init = Some(Box::new(Statement::VarDecl { kind, declarations }));
                let condition = if self.check(&TokenKind::Semicolon) { None } else { Some(self.parse_expression(0)?) };
                self.expect(TokenKind::Semicolon)?;
                let update = if self.check(&TokenKind::RParen) { None } else { Some(self.parse_expression(0)?) };
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;
                return Ok(Statement::For { init, condition, update, body });
            }

            let var_name = self.expect_identifier()?;
            if (var_kind == Some(VarKind::Let) || var_kind == Some(VarKind::Const)) && var_name == "let" {
                return Err(ParseError::UnexpectedToken(TokenKind::Let, self.current.line, self.current.col));
            }
            if self.match_token(&TokenKind::In)? {
                if is_await {
                    return Err(ParseError::UnexpectedToken(TokenKind::In, self.current.line, self.current.col));
                }
                let right = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;

                if var_kind == Some(VarKind::Let) || var_kind == Some(VarKind::Const) {
                    let mut body_vars = Vec::new();
                    Parser::collect_var_declared_names(&body, &mut body_vars);
                    if body_vars.contains(&var_name) {
                        return Err(ParseError::General(format!("Identifier '{}' has already been declared", var_name)));
                    }
                }

                return Ok(Statement::ForIn { var_kind, left: var_name, right, body });
            }
            if self.check_ident("of") {
                self.advance()?; // of
                let right = self.parse_expression(1)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;

                if var_kind == Some(VarKind::Let) || var_kind == Some(VarKind::Const) {
                    let mut body_vars = Vec::new();
                    Parser::collect_var_declared_names(&body, &mut body_vars);
                    if body_vars.contains(&var_name) {
                        return Err(ParseError::General(format!("Identifier '{}' has already been declared", var_name)));
                    }
                }

                return Ok(Statement::ForOf { var_kind, left: var_name, right, body, is_await });
            }
            if is_await {
                return Err(ParseError::UnexpectedToken(self.current.kind.clone(), self.current.line, self.current.col));
            }
            let kind = match kind_tok.kind {
                TokenKind::Let => VarKind::Let,
                TokenKind::Const => VarKind::Const,
                _ => VarKind::Var,
            };
            let init_val = if self.match_token(&TokenKind::Eq)? {
                let old_in = self.allow_in;
                self.allow_in = false;
                let val = self.parse_expression(1);
                self.allow_in = old_in;
                Some(val?)
            } else {
                None
            };
            let mut declarations = vec![VarDeclarator { name: var_name, init: init_val }];
            while self.match_token(&TokenKind::Comma)? {
                let n = self.expect_identifier()?;
                if (kind == VarKind::Let || kind == VarKind::Const) && n == "let" {
                    return Err(ParseError::UnexpectedToken(TokenKind::Let, self.current.line, self.current.col));
                }
                let iv = if self.match_token(&TokenKind::Eq)? {
                    let old_in = self.allow_in;
                    self.allow_in = false;
                    let val = self.parse_expression(1);
                    self.allow_in = old_in;
                    Some(val?)
                } else {
                    None
                };
                declarations.push(VarDeclarator { name: n, init: iv });
            }
            self.expect(TokenKind::Semicolon)?;
            let init = Some(Box::new(Statement::VarDecl { kind, declarations }));
            let condition = if self.check(&TokenKind::Semicolon) { None } else { Some(self.parse_expression(0)?) };
            self.expect(TokenKind::Semicolon)?;
            let update = if self.check(&TokenKind::RParen) { None } else { Some(self.parse_expression(0)?) };
            self.expect(TokenKind::RParen)?;
            let body = Box::new(self.parse_statement()?);
            self.check_sub_statement(&body, false)?;

            if kind != VarKind::Var {
                let mut body_vars = Vec::new();
                Parser::collect_var_declared_names(&body, &mut body_vars);
                if let Some(Statement::VarDecl { declarations: decls, .. }) = init.as_deref() {
                    for d in decls {
                        if !d.name.starts_with('@') && body_vars.contains(&d.name) {
                            return Err(ParseError::General(format!("Identifier '{}' has already been declared", d.name)));
                        }
                    }
                }
            }

            return Ok(Statement::For { init, condition, update, body });
        }

        let init = if self.match_token(&TokenKind::Semicolon)? {
            if is_await {
                return Err(ParseError::UnexpectedToken(TokenKind::Semicolon, self.current.line, self.current.col));
            }
            None
        } else {
            let old_in = self.allow_in;
            self.allow_in = false;
            let expr_res = self.parse_expression(0);
            self.allow_in = old_in;
            let expr = expr_res?;
            if self.match_token(&TokenKind::In)? {
                if is_await {
                    return Err(ParseError::UnexpectedToken(TokenKind::In, self.current.line, self.current.col));
                }
                self.validate_assignment_pattern(&expr)?;
                let right = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;
                let mut unwrapped = &expr;
                while let Expression::Grouping(inner) = unwrapped {
                    unwrapped = inner;
                }
                return match unwrapped {
                    Expression::Identifier(n) => {
                        Ok(Statement::ForIn { var_kind: None, left: n.clone(), right, body })
                    }
                    other => {
                        let tmp_name = format!("@destruct_for_{}", self.destruct_counter);
                        self.destruct_counter += 1;
                        let mut assign_stmts = Vec::new();
                        self.desugar_assignment_destructuring(other, Expression::Identifier(tmp_name.clone()), &mut assign_stmts);
                        let body = prepend_to_body(body, assign_stmts);
                        Ok(Statement::ForIn { var_kind: Some(VarKind::Let), left: tmp_name, right, body })
                    }
                };
            }
            if self.check_ident("of") {
                self.advance()?; // of
                self.validate_assignment_pattern(&expr)?;
                let right = self.parse_expression(1)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;
                let mut unwrapped = &expr;
                while let Expression::Grouping(inner) = unwrapped {
                    unwrapped = inner;
                }
                return match unwrapped {
                    Expression::Identifier(n) => {
                        Ok(Statement::ForOf { var_kind: None, left: n.clone(), right, body, is_await })
                    }
                    other => {
                        let tmp_name = format!("@destruct_for_{}", self.destruct_counter);
                        self.destruct_counter += 1;
                        let mut assign_stmts = Vec::new();
                        self.desugar_assignment_destructuring(other, Expression::Identifier(tmp_name.clone()), &mut assign_stmts);
                        let body = prepend_to_body(body, assign_stmts);
                        Ok(Statement::ForOf { var_kind: Some(VarKind::Let), left: tmp_name, right, body, is_await })
                    }
                };
            }
            if is_await {
                return Err(ParseError::UnexpectedToken(self.current.kind.clone(), self.current.line, self.current.col));
            }
            self.expect(TokenKind::Semicolon)?;
            Some(Box::new(Statement::Expression(expr)))
        };

        let condition = if self.check(&TokenKind::Semicolon) { None } else { Some(self.parse_expression(0)?) };
        self.expect(TokenKind::Semicolon)?;
        let update = if self.check(&TokenKind::RParen) { None } else { Some(self.parse_expression(0)?) };
        self.expect(TokenKind::RParen)?;
        let body = Box::new(self.parse_statement()?);
        self.check_sub_statement(&body, false)?;
        Ok(Statement::For { init, condition, update, body })
    }
}
