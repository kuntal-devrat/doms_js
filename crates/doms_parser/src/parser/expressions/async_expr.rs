use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_async_prefix_expr(&mut self) -> Result<Expression, ParseError> {
                if self.match_token(&TokenKind::Function)? {
                    let prev_in_static = self.in_static_block;
                    self.in_static_block = false;
                    let is_generator = self.match_token(&TokenKind::Star)?;
                    let name = if let TokenKind::Ident(id) = &self.current.kind {
                        if self.is_strict && (is_strict_reserved_word(id) || id == "eval" || id == "arguments") {
                            self.in_static_block = prev_in_static;
                            return Err(ParseError::UnexpectedToken(TokenKind::Ident(id.clone()), self.current.line, self.current.col));
                        }
                        let n = id.clone();
                        self.advance()?;
                        Some(n)
                    } else {
                        None
                    };

                    let prev_gen = self.in_generator;
                    let prev_async = self.in_async;
                    if is_generator {
                        self.in_generator = true;
                    }
                    self.in_async = true;
                    let (params, defaults) = match self.parse_formal_parameters() {
                        Ok(p) => p,
                        Err(e) => {
                            self.in_generator = prev_gen;
                            self.in_async = prev_async;
                            self.in_static_block = prev_in_static;
                            return Err(e);
                        }
                    };
                    let mut body = match self.parse_function_body(&params, name.as_deref()) {
                        Ok(b) => b,
                        Err(e) => {
                            self.in_generator = prev_gen;
                            self.in_async = prev_async;
                            self.in_static_block = prev_in_static;
                            return Err(e);
                        }
                    };
                    self.in_generator = prev_gen;
                    self.in_async = prev_async;
                    self.in_static_block = prev_in_static;
                    Self::apply_parameter_defaults(&mut body, defaults);

                    if is_generator {
                        Ok(Expression::AsyncGeneratorExpr { name, params, body })
                    } else {
                        Ok(Expression::AsyncFunctionExpr { name, params, body })
                    }
                } else if !self.current.has_preceding_line_terminator && self.check(&TokenKind::LParen) {
                    let line = self.current.line;
                    let col = self.current.col;
                    self.advance()?; // consume '('
                    let prev_async = self.in_async;
                    self.in_async = true;
                    let res = self.parse_paren_or_arrow(line, col)?;
                    self.in_async = prev_async;
                    match res {
                        Expression::ArrowExpr { params, body } => {
                            Ok(Expression::AsyncFunctionExpr {
                                name: None,
                                params,
                                body,
                            })
                        }
                        other => {
                            Ok(Expression::Call {
                                callee: Box::new(Expression::Identifier("async".to_string())),
                                args: match other {
                                    Expression::Sequence(exprs) => exprs,
                                    Expression::Grouping(expr) => vec![*expr],
                                    expr => vec![expr],
                                },
                            })
                        }
                    }
                } else if !self.current.has_preceding_line_terminator && matches!(&self.current.kind, TokenKind::Ident(_)) {
                    if let TokenKind::Ident(param_name) = &self.current.kind {
                        let name = param_name.clone();
                        let line = self.current.line;
                        let col = self.current.col;
                        self.advance()?; // consume param ident
                        if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
                            let prev_in_func = self.in_function;
                            let prev_in_static = self.in_static_block;
                            let prev_async = self.in_async;
                            self.in_function = true;
                            self.in_static_block = false;
                            self.in_async = true;
                            let body = if self.check(&TokenKind::LBrace) {
                                match self.parse_statement()? {
                                    Statement::Block(stmts) => stmts,
                                    other => vec![other],
                                }
                            } else {
                                let expr = self.parse_expression(1)?;
                                vec![Statement::Return(Some(expr))]
                            };
                            self.in_function = prev_in_func;
                            self.in_static_block = prev_in_static;
                            self.in_async = prev_async;
                            Ok(Expression::AsyncFunctionExpr {
                                name: None,
                                params: vec![name],
                                body,
                            })
                        } else {
                            Err(ParseError::UnexpectedToken(TokenKind::Ident(name), line, col))
                        }
                    } else {
                        Ok(Expression::Identifier("async".to_string()))
                    }
                } else {
                    Ok(Expression::Identifier("async".to_string()))
                }
    }
}
