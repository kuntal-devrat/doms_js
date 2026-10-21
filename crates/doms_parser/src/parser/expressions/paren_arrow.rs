use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_paren_or_arrow(
        &mut self,
        line: usize,
        col: usize,
    ) -> Result<Expression, ParseError> {
        if self.match_token(&TokenKind::RParen)? {
            if !self.current.has_preceding_line_terminator
                && self.match_token(&TokenKind::FatArrow)?
            {
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
                return Ok(Expression::ArrowExpr { params: vec![], body });
            }
            return Err(ParseError::UnexpectedToken(TokenKind::RParen, line, col));
        }
        let old_in = self.allow_in;
        self.allow_in = true;
        let mut exprs = Vec::new();
        let mut rest_param = None;
        if self.match_token(&TokenKind::DotDotDot)? {
            if self.check(&TokenKind::LBracket) || self.check(&TokenKind::LBrace) {
                let prev_spread = self.allow_spread;
                self.allow_spread = true;
                let pat = self.parse_expression(1)?;
                self.allow_spread = prev_spread;
                rest_param = Some(pat);
            } else {
                let id = self.expect_identifier()?;
                rest_param = Some(Expression::Identifier(id));
            }
        } else {
            let prev_spread = self.allow_spread;
            self.allow_spread = true;
            let first = self.parse_expression(1)?;
            self.allow_spread = prev_spread;
            exprs.push(first);
            while self.match_token(&TokenKind::Comma)? {
                if self.check(&TokenKind::RParen) {
                    break;
                }
                if self.match_token(&TokenKind::DotDotDot)? {
                    if self.check(&TokenKind::LBracket) || self.check(&TokenKind::LBrace) {
                        let prev_spread = self.allow_spread;
                        self.allow_spread = true;
                        let pat = self.parse_expression(1)?;
                        self.allow_spread = prev_spread;
                        rest_param = Some(pat);
                    } else {
                        let id = self.expect_identifier()?;
                        rest_param = Some(Expression::Identifier(id));
                    }
                    break;
                }
                let prev_spread = self.allow_spread;
                self.allow_spread = true;
                let next = self.parse_expression(1)?;
                self.allow_spread = prev_spread;
                exprs.push(next);
            }
        }
        self.expect(TokenKind::RParen)?;
        self.allow_in = old_in;
        if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
            for e in &exprs {
                if contains_yield_expr(e) {
                    return Err(ParseError::UnexpectedToken(TokenKind::Yield, line, col));
                }
            }
            if let Some(ref r) = rest_param {
                if contains_yield_expr(r) {
                    return Err(ParseError::UnexpectedToken(TokenKind::Yield, line, col));
                }
            }

            for e in &exprs {
                validate_arrow_param_pattern(e, line, col, self.trailing_comma_after_rest)?;
            }
            if let Some(ref r) = rest_param {
                if matches!(r, Expression::Assignment { .. }) {
                    return Err(ParseError::UnexpectedToken(TokenKind::Eq, line, col));
                }
                validate_arrow_param_pattern(r, line, col, self.trailing_comma_after_rest)?;
            }

            let mut all_bound_names = Vec::new();
            for e in &exprs {
                collect_arrow_bound_names(e, &mut all_bound_names);
            }
            if let Some(ref r) = rest_param {
                collect_arrow_bound_names(r, &mut all_bound_names);
            }

            let mut seen = std::collections::HashSet::new();
            for name in &all_bound_names {
                if !seen.insert(name.as_str()) {
                    return Err(ParseError::UnexpectedToken(TokenKind::Ident(name.clone()), line, col));
                }
            }

            if self.is_strict {
                for name in &all_bound_names {
                    if is_strict_reserved_word(name) || name == "eval" || name == "arguments" {
                        return Err(ParseError::UnexpectedToken(TokenKind::Ident(name.clone()), line, col));
                    }
                }
            }

            let mut params = Vec::new();
            let mut defaults = Vec::new();
            self.current_params_simple = rest_param.is_none();
            for e in exprs {
                match e {
                    Expression::Identifier(name) => params.push(name),
                    Expression::Assignment { target, value } => {
                        self.current_params_simple = false;
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
                        self.current_params_simple = false;
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
                                declarations: bound_names
                                    .into_iter()
                                    .map(|n| VarDeclarator { name: n, init: None })
                                    .collect(),
                            });
                        }
                        self.desugar_assignment_destructuring(
                            &pattern,
                            Expression::Identifier(name),
                            &mut defaults,
                        );
                    }
                    pat @ Expression::Array(_) | pat @ Expression::Object(_) => {
                        self.current_params_simple = false;
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
                                declarations: bound_names
                                    .into_iter()
                                    .map(|n| VarDeclarator { name: n, init: None })
                                    .collect(),
                            });
                        }
                        self.desugar_assignment_destructuring(
                            &pat,
                            Expression::Identifier(name),
                            &mut defaults,
                        );
                    }
                    _ => {
                        return Err(ParseError::UnexpectedToken(TokenKind::FatArrow, line, col));
                    }
                }
            }
            if let Some(r) = rest_param {
                match r {
                    Expression::Identifier(id) => {
                        params.push(format!("...{}", id));
                    }
                    pat @ Expression::Array(_) | pat @ Expression::Object(_) => {
                        self.current_params_simple = false;
                        let name = format!("__param_rest_{}", self.destruct_counter);
                        self.destruct_counter += 1;
                        params.push(format!("...{}", name));
                        let mut bound_names = Vec::new();
                        collect_arrow_bound_names(&pat, &mut bound_names);
                        if !bound_names.is_empty() {
                            defaults.push(Statement::VarDecl {
                                kind: VarKind::Var,
                                declarations: bound_names
                                    .into_iter()
                                    .map(|n| VarDeclarator { name: n, init: None })
                                    .collect(),
                            });
                        }
                        self.desugar_assignment_destructuring(
                            &pat,
                            Expression::Identifier(name),
                            &mut defaults,
                        );
                    }
                    _ => {
                        return Err(ParseError::UnexpectedToken(TokenKind::FatArrow, line, col));
                    }
                }
            }
            let prev_in_func = self.in_function;
            let prev_in_static = self.in_static_block;
            self.in_function = true;
            self.in_static_block = false;
            let mut body = if self.check(&TokenKind::LBrace) {
                self.parse_function_body(&all_bound_names, None)?
            } else {
                let expr = self.parse_expression(1)?;
                vec![Statement::Return(Some(expr))]
            };
            self.in_function = prev_in_func;
            self.in_static_block = prev_in_static;
            Self::apply_parameter_defaults(&mut body, defaults);
            return Ok(Expression::ArrowExpr { params, body });
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

fn contains_yield_expr(e: &Expression) -> bool {
    match e {
        Expression::Yield(_) | Expression::YieldStar(_) => true,
        Expression::Binary { left, right, .. } => contains_yield_expr(left) || contains_yield_expr(right),
        Expression::Unary { expr, .. } => contains_yield_expr(expr),
        Expression::Assignment { value, .. } => contains_yield_expr(value),
        Expression::DestructuringAssignment { value, .. } => contains_yield_expr(value),
        Expression::Array(elems) => elems.iter().any(contains_yield_expr),
        Expression::Spread(inner) => contains_yield_expr(inner),
        Expression::Grouping(inner) => contains_yield_expr(inner),
        Expression::Sequence(exprs) => exprs.iter().any(contains_yield_expr),
        _ => false,
    }
}

fn validate_arrow_param_pattern(
    e: &Expression,
    line: usize,
    col: usize,
    trailing_comma_after_rest: bool,
) -> Result<(), ParseError> {
    match e {
        Expression::Array(elements) => {
            let total = elements.len();
            for (idx, elem) in elements.iter().enumerate() {
                if let Expression::Spread(inner) = elem {
                    if idx + 1 != total {
                        return Err(ParseError::UnexpectedToken(TokenKind::DotDotDot, line, col));
                    }
                    if trailing_comma_after_rest {
                        return Err(ParseError::UnexpectedToken(TokenKind::Comma, line, col));
                    }
                    if matches!(&**inner, Expression::Assignment { .. }) {
                        return Err(ParseError::UnexpectedToken(TokenKind::Eq, line, col));
                    }
                    validate_arrow_param_pattern(inner, line, col, false)?;
                } else {
                    validate_arrow_param_pattern(elem, line, col, false)?;
                }
            }
            Ok(())
        }
        Expression::Object(props) => {
            let total = props.len();
            for (idx, prop) in props.iter().enumerate() {
                match prop {
                    ObjectProperty::Spread(inner) => {
                        if idx + 1 != total {
                            return Err(ParseError::UnexpectedToken(TokenKind::DotDotDot, line, col));
                        }
                        if matches!(inner, Expression::Assignment { .. }) {
                            return Err(ParseError::UnexpectedToken(TokenKind::Eq, line, col));
                        }
                        validate_arrow_param_pattern(inner, line, col, false)?;
                    }
                    ObjectProperty::KeyValue(_, val) => {
                        validate_arrow_param_pattern(val, line, col, false)?;
                    }
                    _ => {}
                }
            }
            Ok(())
        }
        Expression::Assignment { value, .. } => {
            validate_arrow_param_pattern(value, line, col, false)
        }
        Expression::DestructuringAssignment { pattern, value } => {
            validate_arrow_param_pattern(pattern, line, col, false)?;
            validate_arrow_param_pattern(value, line, col, false)
        }
        Expression::Spread(inner) => {
            if matches!(&**inner, Expression::Assignment { .. }) {
                return Err(ParseError::UnexpectedToken(TokenKind::Eq, line, col));
            }
            validate_arrow_param_pattern(inner, line, col, false)
        }
        Expression::Grouping(inner) => validate_arrow_param_pattern(inner, line, col, false),
        _ => Ok(()),
    }
}

fn collect_arrow_bound_names(e: &Expression, out: &mut Vec<String>) {
    match e {
        Expression::Identifier(name) => {
            out.push(name.clone());
        }
        Expression::Assignment { target, .. } => {
            out.push(target.clone());
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
