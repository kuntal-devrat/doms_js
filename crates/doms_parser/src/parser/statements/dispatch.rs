use super::*;

impl<'a> Parser<'a> {
    pub fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        let is_top = self.is_top_level;
        self.is_top_level = false;
        match self.current.kind {
            TokenKind::Class => self.parse_class_decl(),
            TokenKind::Let => {
                if !self.is_strict {
                    if self.peeked.kind == TokenKind::LBracket {
                        self.parse_var_decl(VarKind::Let)
                    } else if self.peeked.has_preceding_line_terminator {
                        let expr = self.parse_expression(0)?;
                        self.consume_semicolon()?;
                        Ok(Statement::Expression(expr))
                    } else if self.in_single_statement_context {
                        let expr = self.parse_expression(0)?;
                        self.consume_semicolon()?;
                        Ok(Statement::Expression(expr))
                    } else if matches!(
                        self.peeked.kind,
                        TokenKind::Ident(_)
                            | TokenKind::LBrace
                            | TokenKind::Async
                            | TokenKind::Await
                            | TokenKind::Yield
                    ) {
                        self.parse_var_decl(VarKind::Let)
                    } else {
                        let expr = self.parse_expression(0)?;
                        self.consume_semicolon()?;
                        Ok(Statement::Expression(expr))
                    }
                } else {
                    self.parse_var_decl(VarKind::Let)
                }
            }
            TokenKind::Const => self.parse_var_decl(VarKind::Const),
            TokenKind::Var => self.parse_var_decl(VarKind::Var),
            TokenKind::Ident(ref id)
                if id == "using"
                    && !self.peeked.has_preceding_line_terminator
                    && matches!(self.peeked.kind, TokenKind::Ident(_) | TokenKind::Await) =>
            {
                self.advance()?; // 'using'
                self.parse_using_decl()
            }
            TokenKind::Async => {
                if self.peeked.kind == TokenKind::Function
                    && !self.peeked.has_preceding_line_terminator
                {
                    self.advance()?; // async
                    self.parse_async_function_decl()
                } else {
                    let expr = self.parse_expression(0)?;
                    self.consume_semicolon()?;
                    Ok(Statement::Expression(expr))
                }
            }
            TokenKind::Function => {
                self.advance()?; // function
                if self.match_token(&TokenKind::Star)? {
                    self.parse_generator_decl()
                } else {
                    self.parse_function_decl_body()
                }
            }
            TokenKind::Import => {
                if self.peeked.kind == TokenKind::LParen {
                    let expr = self.parse_expression(0)?;
                    self.consume_semicolon()?;
                    Ok(Statement::Expression(expr))
                } else {
                    self.parse_import_statement(is_top)
                }
            }
            TokenKind::Export => self.parse_export_statement(is_top),
            TokenKind::If => self.parse_if_statement(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::Do => self.parse_do_while_statement(),
            TokenKind::For => self.parse_for_statement(),
            TokenKind::Switch => self.parse_switch_statement(),
            TokenKind::Break => {
                self.advance()?;
                let label = if let TokenKind::Ident(name) = &self.current.kind {
                    if !self.current.has_preceding_line_terminator {
                        let l = name.clone();
                        self.advance()?;
                        Some(l)
                    } else {
                        None
                    }
                } else {
                    None
                };
                self.consume_semicolon()?;
                Ok(Statement::Break(label))
            }
            TokenKind::Continue => {
                self.advance()?;
                let label = if let TokenKind::Ident(name) = &self.current.kind {
                    if !self.current.has_preceding_line_terminator {
                        let l = name.clone();
                        self.advance()?;
                        Some(l)
                    } else {
                        None
                    }
                } else {
                    None
                };
                self.consume_semicolon()?;
                Ok(Statement::Continue(label))
            }
            TokenKind::Try => self.parse_try_statement(),
            TokenKind::Throw => {
                self.advance()?;
                if self.current.has_preceding_line_terminator {
                    return Err(ParseError::UnexpectedToken(
                        self.current.kind.clone(),
                        self.current.line,
                        self.current.col,
                    ));
                }
                let expr = self.parse_expression(0)?;
                self.consume_semicolon()?;
                Ok(Statement::Throw(expr))
            }
            TokenKind::Return => self.parse_return_statement(),
            TokenKind::LBrace => self.parse_block_statement(),
            TokenKind::Semicolon => {
                self.advance()?;
                Ok(Statement::Empty)
            }
            TokenKind::Debugger => {
                self.advance()?;
                self.consume_semicolon()?;
                Ok(Statement::Debugger)
            }
            TokenKind::With => self.parse_with_statement(),
            ref tok
                if self.peeked.kind == TokenKind::Colon
                    && match tok {
                        TokenKind::Ident(_) => true,
                        TokenKind::Implements
                        | TokenKind::Interface
                        | TokenKind::Package
                        | TokenKind::Private
                        | TokenKind::Protected
                        | TokenKind::Public
                        | TokenKind::Static
                        | TokenKind::Yield
                        | TokenKind::Let
                        | TokenKind::Await => true,
                        _ => false,
                    } =>
            {
                let lbl = match &self.current.kind {
                    TokenKind::Ident(s) => s.clone(),
                    other => other.as_prop_name().unwrap_or_default(),
                };
                if lbl == "let" {
                    return Err(ParseError::UnexpectedToken(
                        self.current.kind.clone(),
                        self.current.line,
                        self.current.col,
                    ));
                }
                if self.is_strict && is_strict_reserved_word(&lbl) {
                    return Err(ParseError::UnexpectedToken(
                        self.current.kind.clone(),
                        self.current.line,
                        self.current.col,
                    ));
                }
                if (self.in_async || self.is_module || self.in_static_block) && lbl == "await" {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::Await,
                        self.current.line,
                        self.current.col,
                    ));
                }
                if (self.in_generator || self.is_strict) && lbl == "yield" {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::Yield,
                        self.current.line,
                        self.current.col,
                    ));
                }
                self.advance()?; // ident/keyword
                self.advance()?; // colon
                let stmt = self.parse_sub_statement(true)?;
                Ok(Statement::Labeled { label: lbl, body: Box::new(stmt) })
            }
            _ => {
                let expr = self.parse_expression(0)?;
                self.consume_semicolon()?;
                Ok(Statement::Expression(expr))
            }
        }
    }
}
