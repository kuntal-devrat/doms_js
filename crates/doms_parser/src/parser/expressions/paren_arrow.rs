use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_paren_or_arrow(&mut self, line: usize, col: usize) -> Result<Expression, ParseError> {
                if self.match_token(&TokenKind::RParen)? {
                    if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
                        let prev_in_func = self.in_function;
                        self.in_function = true;
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
                        return Ok(Expression::ArrowExpr {
                            params: vec![],
                            body,
                        });
                    }
                    return Err(ParseError::UnexpectedToken(TokenKind::RParen, line, col));
                }
                let mut exprs = Vec::new();
                let mut rest_param = None;
                if self.match_token(&TokenKind::DotDotDot)? {
                    let id = self.expect_identifier()?;
                    rest_param = Some(format!("...{}", id));
                } else {
                    exprs.push(self.parse_expression(1)?);
                    while self.match_token(&TokenKind::Comma)? {
                        if self.check(&TokenKind::RParen) {
                            break;
                        }
                        if self.match_token(&TokenKind::DotDotDot)? {
                            let id = self.expect_identifier()?;
                            rest_param = Some(format!("...{}", id));
                            break;
                        }
                        exprs.push(self.parse_expression(1)?);
                    }
                }
                self.expect(TokenKind::RParen)?;
                if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
                    let mut params = Vec::new();
                    let mut defaults = Vec::new();
                    for e in &exprs {
                        match e {
                            Expression::Identifier(name) => params.push(name.clone()),
                            Expression::Assignment { target, value } => {
                                params.push(target.clone());
                                defaults.push(Statement::If {
                                    condition: Expression::Binary {
                                        op: BinaryOp::StrictEq,
                                        left: Box::new(Expression::Identifier(target.clone())),
                                        right: Box::new(Expression::Literal(Literal::Undefined)),
                                    },
                                    then_branch: Box::new(Statement::Expression(Expression::Assignment {
                                        target: target.clone(),
                                        value: value.clone(),
                                    })),
                                    else_branch: None,
                                });
                            }
                            Expression::Array(_) | Expression::Object(_) => {
                                let name = format!("__param_{}", self.destruct_counter);
                                self.destruct_counter += 1;
                                params.push(name.clone());
                                defaults.push(Statement::If {
                                    condition: Expression::Binary {
                                        op: BinaryOp::Eq,
                                        left: Box::new(Expression::Identifier(name)),
                                        right: Box::new(Expression::Literal(Literal::Null)),
                                    },
                                    then_branch: Box::new(Statement::Throw(Expression::New {
                                        callee: Box::new(Expression::Identifier("TypeError".to_string())),
                                        args: vec![Expression::Literal(Literal::String {
                                            value: "Cannot destructure 'undefined' or 'null'".to_string(),
                                            has_escape: false,
                                            has_legacy_escape: false,
                                        })],
                                    })),
                                    else_branch: None,
                                });
                            }
                            _ => {}
                        }
                    }
                    if let Some(r) = rest_param {
                        params.push(r);
                    }
                    let prev_in_func = self.in_function;
                    self.in_function = true;
                    let mut body = if self.check(&TokenKind::LBrace) {
                        self.parse_function_body(&params, None)?
                    } else {
                        let expr = self.parse_expression(1)?;
                        vec![Statement::Return(Some(expr))]
                    };
                    self.in_function = prev_in_func;
                    Self::apply_parameter_defaults(&mut body, defaults);
                    return Ok(Expression::ArrowExpr {
                        params,
                        body,
                    });
                }
                if rest_param.is_some() {
                    return Err(ParseError::UnexpectedToken(TokenKind::RParen, line, col));
                }
                if exprs.len() == 1 {
                    Ok(Expression::Grouping(Box::new(exprs.pop().unwrap())))
                } else {
                    Ok(Expression::Sequence(exprs))
                }
    }
}
