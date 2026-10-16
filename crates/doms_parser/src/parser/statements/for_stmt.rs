use super::*;

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

        if self.check(&TokenKind::Let) || self.check(&TokenKind::Const) || self.check(&TokenKind::Var) {
            let kind_tok = self.advance()?; // let/const/var
            let var_name = self.expect_identifier()?;
            let var_kind = match kind_tok.kind {
                TokenKind::Let => Some(VarKind::Let),
                TokenKind::Const => Some(VarKind::Const),
                _ => Some(VarKind::Var),
            };
            if self.match_token(&TokenKind::In)? {
                if is_await {
                    return Err(ParseError::UnexpectedToken(TokenKind::In, self.current.line, self.current.col));
                }
                let right = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;
                return Ok(Statement::ForIn { var_kind, left: var_name, right, body });
            }
            if self.check_ident("of") {
                self.advance()?; // of
                let right = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;
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
                let right = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;
                let left = match expr {
                    Expression::Identifier(n) => n,
                    _ => "__lhs".to_string(),
                };
                return Ok(Statement::ForIn { var_kind: None, left, right, body });
            }
            if self.check_ident("of") {
                self.advance()?; // of
                let right = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;
                let left = match expr {
                    Expression::Identifier(n) => n,
                    _ => "__lhs".to_string(),
                };
                return Ok(Statement::ForOf { var_kind: None, left, right, body, is_await });
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
