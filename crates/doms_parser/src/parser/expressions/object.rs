use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_object_literal(&mut self) -> Result<Expression, ParseError> {
        let mut props = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            // Check for getter: get <prop>() { ... }
            if self.check_ident("get")
                && self.peeked.kind != TokenKind::Colon
                && self.peeked.kind != TokenKind::LParen
                && self.peeked.kind != TokenKind::Comma
                && self.peeked.kind != TokenKind::RBrace
            {
                self.advance()?; // 'get'
                let key = self.parse_property_name()?;
                self.expect(TokenKind::LParen)?;
                self.expect(TokenKind::RParen)?;
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                self.in_method = true;
                self.in_derived_constructor = false;
                let body = self.parse_function_body(&[], None)?;
                self.in_method = prev_method;
                self.in_derived_constructor = prev_ctor;
                props.push(ObjectProperty::Getter(
                    key,
                    Expression::FunctionExpr {
                        name: None,
                        params: vec![],
                        body,
                    },
                ));
                if self.check(&TokenKind::Comma) {
                    self.advance()?;
                }
                continue;
            }

            // Check for setter: set <prop>(param) { ... }
            if self.check_ident("set")
                && self.peeked.kind != TokenKind::Colon
                && self.peeked.kind != TokenKind::LParen
                && self.peeked.kind != TokenKind::Comma
                && self.peeked.kind != TokenKind::RBrace
            {
                self.advance()?; // 'set'
                let key = self.parse_property_name()?;
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
                props.push(ObjectProperty::Setter(
                    key,
                    Expression::FunctionExpr {
                        name: None,
                        params: vec![param],
                        body,
                    },
                ));
                if self.check(&TokenKind::Comma) {
                    self.advance()?;
                }
                continue;
            }

            // Check for generator method: * <prop>() { ... }
            if self.match_token(&TokenKind::Star)? {
                let key = self.parse_property_name()?;
                let (params, defaults) = self.parse_formal_parameters()?;
                let mut body = self.parse_function_body(&params, None)?;
                Self::apply_parameter_defaults(&mut body, defaults);
                let fn_name = match &key {
                    PropertyKey::Literal(s) => Some(s.clone()),
                    PropertyKey::Computed(_) => None,
                };
                props.push(ObjectProperty::KeyValue(
                    key,
                    Expression::GeneratorExpr {
                        name: fn_name,
                        params,
                        body,
                    },
                ));
                if !self.match_token(&TokenKind::Comma)? || self.check(&TokenKind::RBrace) {
                    break;
                }
                continue;
            }

            // Check for async method: async <prop>() { ... } or async * <prop>() { ... }
            if self.check(&TokenKind::Async)
                && self.peeked.kind != TokenKind::Colon
                && self.peeked.kind != TokenKind::Comma
                && self.peeked.kind != TokenKind::RBrace
                && self.peeked.kind != TokenKind::LParen
            {
                self.advance()?; // 'async'
                let is_gen = self.match_token(&TokenKind::Star)?;
                let key = self.parse_property_name()?;
                let (params, defaults) = self.parse_formal_parameters()?;
                let mut body = self.parse_function_body(&params, None)?;
                Self::apply_parameter_defaults(&mut body, defaults);
                let fn_name = match &key {
                    PropertyKey::Literal(s) => Some(s.clone()),
                    PropertyKey::Computed(_) => None,
                };
                if is_gen {
                    props.push(ObjectProperty::KeyValue(
                        key,
                        Expression::AsyncGeneratorExpr { name: fn_name, params, body },
                    ));
                } else {
                    props.push(ObjectProperty::KeyValue(
                        key,
                        Expression::AsyncFunctionExpr { name: fn_name, params, body },
                    ));
                }
                if !self.match_token(&TokenKind::Comma)? || self.check(&TokenKind::RBrace) {
                    break;
                }
                continue;
            }

            let (key, is_ident) = self.parse_property_name_meta()?;
            let val = if self.match_token(&TokenKind::Colon)? {
                self.parse_expression(1)?
            } else if self.check(&TokenKind::LParen) {
                // Method shorthand: foo() { ... } or [expr]() { ... }
                let (params, defaults) = self.parse_formal_parameters()?;
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                self.in_method = true;
                self.in_derived_constructor = false;
                let mut body = self.parse_function_body(&params, None)?;
                self.in_method = prev_method;
                self.in_derived_constructor = prev_ctor;
                Self::apply_parameter_defaults(&mut body, defaults);
                let fn_name = match &key {
                    PropertyKey::Literal(s) => Some(s.clone()),
                    PropertyKey::Computed(_) => None,
                };
                Expression::MethodExpr {
                    name: fn_name,
                    params,
                    body,
                }
            } else if is_ident {
                match &key {
                    PropertyKey::Literal(id) => Expression::Identifier(id.clone()),
                    _ => unreachable!(),
                }
            } else {
                return Err(ParseError::UnexpectedToken(self.current.kind.clone(), self.current.line, self.current.col));
            };

            props.push(ObjectProperty::KeyValue(key, val));
            if !self.match_token(&TokenKind::Comma)? || self.check(&TokenKind::RBrace) {
                break;
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Expression::Object(props))
    }
}
