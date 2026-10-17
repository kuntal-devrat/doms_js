use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_paren_or_arrow(&mut self, line: usize, col: usize) -> Result<Expression, ParseError> {
                if self.match_token(&TokenKind::RParen)? {
                    if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
                        let prev_in_func = self.in_function;
                        let prev_in_static = self.in_static_block;
                        self.in_function = true;
                        self.in_static_block = false;
                        let body = if self.check(&TokenKind::LBrace) {
                            self.parse_function_body(&[], None)?
                        } else {
                            let expr = self.parse_expression(1)?;
                            vec![Statement::Return(Some(expr))]
                        };
                        self.in_function = prev_in_func;
                        self.in_static_block = prev_in_static;
                        return Ok(Expression::ArrowExpr {
                            params: vec![],
                            body,
                        });
                    }
                    return Err(ParseError::UnexpectedToken(TokenKind::RParen, line, col));
                }
                let old_in = self.allow_in;
                self.allow_in = true;
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
                self.allow_in = old_in;
                if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
                    let mut params = Vec::new();
                    let mut defaults = Vec::new();
                    for e in exprs {
                        match e {
                            Expression::Identifier(name) => params.push(name),
                            Expression::Assignment { target, value } => {
                                params.push(target.clone());
                                defaults.push(Statement::If {
                                    condition: Expression::Binary {
                                        op: BinaryOp::StrictEq,
                                        left: Box::new(Expression::Identifier(target.clone())),
                                        right: Box::new(Expression::Literal(Literal::Undefined)),
                                    },
                                    then_branch: Box::new(Statement::Expression(Expression::Assignment {
                                        target,
                                        value,
                                    })),
                                    else_branch: None,
                                });
                            }
                            Expression::DestructuringAssignment { pattern, value } => {
                                let name = format!("__param_{}", self.destruct_counter);
                                self.destruct_counter += 1;
                                params.push(name.clone());
                                defaults.push(Statement::If {
                                    condition: Expression::Binary {
                                        op: BinaryOp::StrictEq,
                                        left: Box::new(Expression::Identifier(name.clone())),
                                        right: Box::new(Expression::Literal(Literal::Undefined)),
                                    },
                                    then_branch: Box::new(Statement::Expression(Expression::Assignment {
                                        target: name.clone(),
                                        value,
                                    })),
                                    else_branch: None,
                                });
                                defaults.push(Statement::If {
                                    condition: Expression::Binary {
                                        op: BinaryOp::Eq,
                                        left: Box::new(Expression::Identifier(name.clone())),
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
                                let mut bound_names = Vec::new();
                                collect_arrow_bound_names(&pattern, &mut bound_names);
                                if !bound_names.is_empty() {
                                    defaults.push(Statement::VarDecl {
                                        kind: VarKind::Var,
                                        declarations: bound_names.into_iter().map(|n| VarDeclarator { name: n, init: None }).collect(),
                                    });
                                }
                                self.desugar_assignment_destructuring(&pattern, Expression::Identifier(name), &mut defaults);
                            }
                            pat @ Expression::Array(_) | pat @ Expression::Object(_) => {
                                let name = format!("__param_{}", self.destruct_counter);
                                self.destruct_counter += 1;
                                params.push(name.clone());
                                defaults.push(Statement::If {
                                    condition: Expression::Binary {
                                        op: BinaryOp::Eq,
                                        left: Box::new(Expression::Identifier(name.clone())),
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
                                let mut bound_names = Vec::new();
                                collect_arrow_bound_names(&pat, &mut bound_names);
                                if !bound_names.is_empty() {
                                    defaults.push(Statement::VarDecl {
                                        kind: VarKind::Var,
                                        declarations: bound_names.into_iter().map(|n| VarDeclarator { name: n, init: None }).collect(),
                                    });
                                }
                                self.desugar_assignment_destructuring(&pat, Expression::Identifier(name), &mut defaults);
                            }
                            _ => {}
                        }
                    }
                    if let Some(r) = rest_param {
                        params.push(r);
                    }
                    let prev_in_func = self.in_function;
                    let prev_in_static = self.in_static_block;
                    self.in_function = true;
                    self.in_static_block = false;
                    let mut body = if self.check(&TokenKind::LBrace) {
                        self.parse_function_body(&params, None)?
                    } else {
                        let expr = self.parse_expression(1)?;
                        vec![Statement::Return(Some(expr))]
                    };
                    self.in_function = prev_in_func;
                    self.in_static_block = prev_in_static;
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

fn collect_arrow_bound_names(e: &Expression, out: &mut Vec<String>) {
    match e {
        Expression::Identifier(name) => {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        Expression::Assignment { target, .. } => {
            if !out.contains(target) {
                out.push(target.clone());
            }
        }
        Expression::DestructuringAssignment { pattern, .. } => {
            collect_arrow_bound_names(pattern, out);
        }
        Expression::Spread(inner) => {
            collect_arrow_bound_names(inner, out);
        }
        Expression::ObjectRest { target, .. } => {
            collect_arrow_bound_names(target, out);
        }
        Expression::Array(elements) => {
            for elem in elements {
                collect_arrow_bound_names(elem, out);
            }
        }
        Expression::Object(props) => {
            for prop in props {
                match prop {
                    ObjectProperty::KeyValue(_, val) => collect_arrow_bound_names(val, out),
                    ObjectProperty::Spread(inner) => collect_arrow_bound_names(inner, out),
                    _ => {}
                }
            }
        }
        Expression::Grouping(inner) => {
            collect_arrow_bound_names(inner, out);
        }
        _ => {}
    }
}
