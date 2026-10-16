use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_class_elements(&mut self, is_derived: bool) -> Result<(Vec<String>, Vec<Statement>, bool, Vec<ClassElement>), ParseError> {
        let mut ctor_params = Vec::new();
        let mut ctor_body = Vec::new();
        let mut has_ctor = false;
        let mut elements = Vec::new();

        self.expect(TokenKind::LBrace)?;
        let private_names = self.scan_declared_private_names();
        self.private_names_stack.push(private_names);
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            if self.match_token(&TokenKind::Semicolon)? {
                continue;
            }
            let is_static = if self.check(&TokenKind::Static) || self.check_ident("static") {
                if self.peeked.kind == TokenKind::LParen || self.peeked.kind == TokenKind::Colon || self.peeked.kind == TokenKind::Eq {
                    false
                } else {
                    self.advance()?;
                    true
                }
            } else {
                false
            };

            let is_async = if self.check(&TokenKind::Async) || self.check_ident("async") {
                if self.peeked.kind == TokenKind::LParen || self.peeked.kind == TokenKind::Colon || self.peeked.kind == TokenKind::Eq || self.peeked.kind == TokenKind::Semicolon {
                    false
                } else {
                    self.advance()?;
                    true
                }
            } else {
                false
            };

            let is_generator = self.match_token(&TokenKind::Star)?;

            if !is_generator && (self.check_ident("get") || self.check_ident("set"))
                && self.peeked.kind != TokenKind::LParen
                && self.peeked.kind != TokenKind::Semicolon
                && self.peeked.kind != TokenKind::RBrace
                && self.peeked.kind != TokenKind::Colon
                && self.peeked.kind != TokenKind::Eq
            {
                let is_getter = self.check_ident("get");
                self.advance()?; // 'get' or 'set'
                let key = self.parse_property_name()?;
                if is_getter {
                    self.expect(TokenKind::LParen)?;
                    self.expect(TokenKind::RParen)?;
                    let prev_ctor = self.in_derived_constructor;
                    let prev_method = self.in_method;
                    self.in_method = true;
                    self.in_derived_constructor = false;
                    let body = self.parse_function_body(&[], None)?;
                    self.in_method = prev_method;
                    self.in_derived_constructor = prev_ctor;
                    elements.push(ClassElement {
                        is_static,
                        key,
                        kind: ClassElementKind::Getter(Expression::FunctionExpr {
                            name: None,
                            params: vec![],
                            body,
                        }),
                    });
                } else {
                    self.expect(TokenKind::LParen)?;
                    let param = self.expect_identifier()?;
                    self.expect(TokenKind::RParen)?;
                    let prev_ctor = self.in_derived_constructor;
                    let prev_method = self.in_method;
                    self.in_method = true;
                    self.in_derived_constructor = false;
                    let body = self.parse_function_body(&[param.clone()], None)?;
                    self.in_method = prev_method;
                    self.in_derived_constructor = prev_ctor;
                    elements.push(ClassElement {
                        is_static,
                        key,
                        kind: ClassElementKind::Setter(Expression::FunctionExpr {
                            name: None,
                            params: vec![param],
                            body,
                        }),
                    });
                }
                continue;
            }

            let key = self.parse_property_name()?;
            if !is_static && !is_generator && matches!(&key, PropertyKey::Literal(k) if k == "constructor") {
                if self.check(&TokenKind::LParen) {
                    let (params, defaults) = self.parse_formal_parameters()?;
                    let prev_ctor = self.in_derived_constructor;
                    let prev_method = self.in_method;
                    self.in_derived_constructor = is_derived;
                    self.in_method = false;
                    let mut body = self.parse_function_body(&params, None)?;
                    self.in_derived_constructor = prev_ctor;
                    self.in_method = prev_method;
                    Self::apply_parameter_defaults(&mut body, defaults);
                    ctor_params = params;
                    ctor_body = body;
                    has_ctor = true;
                    continue;
                }
            }

            if self.check(&TokenKind::LParen) {
                let (params, defaults) = self.parse_formal_parameters()?;
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                self.in_method = true;
                self.in_derived_constructor = false;
                let mut body = self.parse_function_body(&params, None)?;
                self.in_derived_constructor = prev_ctor;
                self.in_method = prev_method;
                Self::apply_parameter_defaults(&mut body, defaults);
                let method_expr = if is_async && is_generator {
                    Expression::AsyncGeneratorExpr {
                        name: None,
                        params,
                        body,
                    }
                } else if is_async {
                    Expression::AsyncFunctionExpr {
                        name: None,
                        params,
                        body,
                    }
                } else if is_generator {
                    Expression::GeneratorExpr {
                        name: None,
                        params,
                        body,
                    }
                } else {
                    Expression::MethodExpr {
                        name: None,
                        params,
                        body,
                    }
                };
                elements.push(ClassElement {
                    is_static,
                    key,
                    kind: ClassElementKind::Method(method_expr),
                });
            } else if self.match_token(&TokenKind::Eq)? {
                let init_expr = self.parse_expression(0)?;
                let _ = self.match_token(&TokenKind::Semicolon)?;
                if !is_static {
                    let init_stmt = match key {
                        PropertyKey::Literal(k) => Statement::Expression(Expression::MemberAssignment {
                            object: Box::new(Expression::This),
                            property: k,
                            value: Box::new(init_expr),
                        }),
                        PropertyKey::Computed(expr) => Statement::Expression(Expression::IndexAssignment {
                            object: Box::new(Expression::This),
                            index: Box::new(expr),
                            value: Box::new(init_expr),
                        }),
                    };
                    ctor_body.push(init_stmt);
                }
            } else {
                let _ = self.match_token(&TokenKind::Semicolon)?;
            }
        }
        self.expect(TokenKind::RBrace)?;
        self.private_names_stack.pop();
        let _ = self.match_token(&TokenKind::Semicolon)?;

        Ok((ctor_params, ctor_body, has_ctor, elements))
    }
}
