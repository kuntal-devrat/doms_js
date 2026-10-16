use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_class_expr(&mut self) -> Result<Expression, ParseError> {
        if self.is_module && self.check(&TokenKind::Await) {
            return Err(ParseError::UnexpectedToken(TokenKind::Await, self.current.line, self.current.col));
        }
        let name = if let TokenKind::Ident(id) = &self.current.kind {
            let n = id.clone();
            self.advance()?;
            Some(n)
        } else {
            None
        };
        let super_class = if self.match_token(&TokenKind::Extends)? {
            Some(Box::new(self.parse_expression(0)?))
        } else {
            None
        };
        let (mut ctor_params, mut ctor_body, has_ctor, elements) = self.parse_class_elements(super_class.is_some())?;
        if !has_ctor {
            if let Some(ref sup) = super_class {
                ctor_params = vec!["...__args".to_string()];
                ctor_body = vec![
                    Statement::Expression(Expression::Call {
                        callee: Box::new(Expression::Member {
                            object: sup.clone(),
                            property: "apply".to_string(),
                        }),
                        args: vec![Expression::This, Expression::Identifier("__args".to_string())],
                    }),
                ];
            }
        }
        if let Some(ref sup) = super_class {
            let this_var = format!("__this_{}", self.destruct_counter);
            self.destruct_counter += 1;
            ctor_body.insert(0, Statement::VarDecl {
                kind: VarKind::Var,
                declarations: vec![
                    VarDeclarator {
                        name: this_var.clone(),
                        init: Some(Expression::This),
                    },
                    VarDeclarator {
                        name: "super".to_string(),
                        init: Some(Expression::FunctionExpr {
                            name: None,
                            params: vec!["...__sargs".to_string()],
                            body: vec![Statement::Return(Some(Expression::Call {
                                callee: Box::new(Expression::Member {
                                    object: sup.clone(),
                                    property: "apply".to_string(),
                                }),
                                args: vec![Expression::Identifier(this_var), Expression::Identifier("__sargs".to_string())],
                            }))],
                        }),
                    },
                ],
            });
        }
        Ok(Expression::ClassExpr(Box::new(ClassDefinition {
            name,
            super_class,
            ctor_params,
            ctor_body,
            elements,
        })))
    }
}
