use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_object_literal(&mut self) -> Result<Expression, ParseError> {
        let prev_in = self.allow_in;
        self.allow_in = true;
        let mut props = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            // Check for spread: ...expr
            if self.match_token(&TokenKind::DotDotDot)? {
                let expr = self.parse_expression(1)?;
                props.push(ObjectProperty::Spread(expr));
                if self.match_token(&TokenKind::Comma)? {
                    if self.check(&TokenKind::RBrace) {
                        self.trailing_comma_after_rest = true;
                    }
                    continue;
                } else {
                    break;
                }
            }

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
                    Expression::FunctionExpr { name: None, params: vec![], body },
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
                let prev_in_static = self.in_static_block;
                self.in_static_block = false;
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                self.in_method = true;
                self.in_derived_constructor = false;
                let (params, defaults) = self.parse_formal_parameters()?;
                if params.len() != 1 || params[0].starts_with("...") {
                    self.in_method = prev_method;
                    self.in_derived_constructor = prev_ctor;
                    self.in_static_block = prev_in_static;
                    return Err(ParseError::General(
                        "Setter must have exactly one formal parameter".to_string(),
                    ));
                }
                let mut body = self.parse_function_body(&params, None)?;
                self.in_method = prev_method;
                self.in_derived_constructor = prev_ctor;
                self.in_static_block = prev_in_static;
                Self::apply_parameter_defaults(&mut body, defaults);
                props.push(ObjectProperty::Setter(
                    key,
                    Expression::FunctionExpr { name: None, params, body },
                ));
                if self.check(&TokenKind::Comma) {
                    self.advance()?;
                }
                continue;
            }

            // Check for generator method: * <prop>() { ... }
            if self.match_token(&TokenKind::Star)? {
                let key = self.parse_property_name()?;
                let prev_in_static = self.in_static_block;
                self.in_static_block = false;
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                self.in_method = true;
                self.in_derived_constructor = false;
                let (params, defaults) = self.parse_formal_parameters()?;
                let mut body = self.parse_function_body(&params, None)?;
                self.in_method = prev_method;
                self.in_derived_constructor = prev_ctor;
                self.in_static_block = prev_in_static;
                Self::apply_parameter_defaults(&mut body, defaults);
                let fn_name = match &key {
                    PropertyKey::Literal(s) => Some(s.clone()),
                    PropertyKey::Computed(_) => None,
                };
                props.push(ObjectProperty::KeyValue(
                    key,
                    Expression::GeneratorExpr { name: fn_name, params, body },
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
                let prev_in_static = self.in_static_block;
                self.in_static_block = false;
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                self.in_method = true;
                self.in_derived_constructor = false;
                let (params, defaults) = self.parse_formal_parameters()?;
                let mut body = self.parse_function_body(&params, None)?;
                self.in_method = prev_method;
                self.in_derived_constructor = prev_ctor;
                self.in_static_block = prev_in_static;
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
            } else if self.match_token(&TokenKind::Eq)? {
                let default_expr = self.parse_expression(1)?;
                match &key {
                    PropertyKey::Literal(id) => {
                        Expression::Assignment { target: id.clone(), value: Box::new(default_expr) }
                    }
                    _ => {
                        self.allow_in = prev_in;
                        return Err(ParseError::UnexpectedToken(
                            TokenKind::Eq,
                            self.current.line,
                            self.current.col,
                        ));
                    }
                }
            } else if self.check(&TokenKind::LParen) {
                // Method shorthand: foo() { ... } or [expr]() { ... }
                let prev_in_static = self.in_static_block;
                self.in_static_block = false;
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                self.in_method = true;
                self.in_derived_constructor = false;
                let (params, defaults) = self.parse_formal_parameters()?;
                let mut body = self.parse_function_body(&params, None)?;
                self.in_method = prev_method;
                self.in_derived_constructor = prev_ctor;
                self.in_static_block = prev_in_static;
                Self::apply_parameter_defaults(&mut body, defaults);
                let fn_name = match &key {
                    PropertyKey::Literal(s) => Some(s.clone()),
                    PropertyKey::Computed(_) => None,
                };
                Expression::MethodExpr { name: fn_name, params, body }
            } else if is_ident {
                match &key {
                    PropertyKey::Literal(id) => Expression::Identifier(id.clone()),
                    _ => unreachable!(),
                }
            } else {
                self.allow_in = prev_in;
                return Err(ParseError::UnexpectedToken(
                    self.current.kind.clone(),
                    self.current.line,
                    self.current.col,
                ));
            };

            props.push(ObjectProperty::KeyValue(key, val));
            if !self.match_token(&TokenKind::Comma)? || self.check(&TokenKind::RBrace) {
                break;
            }
        }
        self.expect(TokenKind::RBrace)?;
        self.allow_in = prev_in;
        Ok(Expression::Object(props))
    }
}
