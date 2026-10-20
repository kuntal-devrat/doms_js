use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_class_decl(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // 'class'
        if (self.is_module || self.in_async || self.in_static_block)
            && self.check(&TokenKind::Await)
        {
            return Err(ParseError::UnexpectedToken(
                TokenKind::Await,
                self.current.line,
                self.current.col,
            ));
        }
        let name = if let TokenKind::Ident(id) = &self.current.kind {
            let n = id.clone();
            self.advance()?;
            Some(n)
        } else if !self.is_module
            && !self.in_async
            && !self.in_static_block
            && self.check(&TokenKind::Await)
        {
            self.advance()?;
            Some("await".to_string())
        } else {
            None
        };
        let super_class = if self.match_token(&TokenKind::Extends)? {
            Some(Box::new(self.parse_expression(0)?))
        } else {
            None
        };
        let (mut ctor_params, mut ctor_body, has_ctor, elements) =
            self.parse_class_elements(super_class.is_some())?;
        if !has_ctor {
            if let Some(ref sup) = super_class {
                ctor_params = vec!["...__args".to_string()];
                ctor_body = vec![Statement::Return(Some(Expression::Call {
                    callee: Box::new(Expression::Member {
                        object: Box::new(Expression::Identifier("Reflect".to_string())),
                        property: "construct".to_string(),
                    }),
                    args: vec![
                        *sup.clone(),
                        Expression::Identifier("__args".to_string()),
                        Expression::NewTarget,
                    ],
                }))];
            }
        }
        if let Some(ref sup) = super_class {
            let nt_var = format!("__nt_{}", self.destruct_counter);
            self.destruct_counter += 1;
            ctor_body.insert(
                0,
                Statement::VarDecl {
                    kind: VarKind::Var,
                    declarations: vec![
                        VarDeclarator { name: nt_var.clone(), init: Some(Expression::NewTarget) },
                        VarDeclarator {
                            name: "super".to_string(),
                            init: Some(Expression::FunctionExpr {
                                name: None,
                                params: vec!["...__sargs".to_string()],
                                body: vec![Statement::Return(Some(Expression::Call {
                                    callee: Box::new(Expression::Member {
                                        object: Box::new(Expression::Identifier(
                                            "Reflect".to_string(),
                                        )),
                                        property: "construct".to_string(),
                                    }),
                                    args: vec![
                                        *sup.clone(),
                                        Expression::Identifier("__sargs".to_string()),
                                        Expression::Identifier(nt_var),
                                    ],
                                }))],
                            }),
                        },
                    ],
                },
            );
        }
        Ok(Statement::ClassDecl(ClassDefinition {
            name,
            super_class,
            ctor_params,
            ctor_body,
            elements,
        }))
    }
}
