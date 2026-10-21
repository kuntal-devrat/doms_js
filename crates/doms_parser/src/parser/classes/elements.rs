use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_class_elements(
        &mut self,
        is_derived: bool,
    ) -> Result<(Vec<String>, Vec<Statement>, bool, Vec<ClassElement>), ParseError> {
        let mut ctor_params = Vec::new();
        let mut ctor_body = Vec::new();
        let mut has_ctor = false;
        let mut elements = Vec::new();
        let mut instance_fields = Vec::new();

        let prev_strict = self.is_strict;
        let prev_in_static = self.in_static_block;
        self.is_strict = true;
        self.in_static_block = false;

        self.expect(TokenKind::LBrace)?;
        let class_id = self.class_id_counter;
        self.class_id_counter += 1;
        let private_names = self.scan_declared_private_names();
        if private_names.contains("#constructor") {
            return Err(ParseError::General(
                "Class cannot have private member '#constructor'".to_string(),
            ));
        }
        let mut declared_privates: std::collections::HashMap<String, (bool, u8)> =
            std::collections::HashMap::new();
        let mut scoped_names = std::collections::HashMap::new();
        for name in &private_names {
            scoped_names.insert(name.clone(), format!("{}@{}", name, class_id));
        }
        self.private_names_stack.push(scoped_names);
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            if self.match_token(&TokenKind::Semicolon)? {
                continue;
            }
            while Self::is_ts_modifier(&self.current.kind)
                && !matches!(
                    &self.peeked.kind,
                    TokenKind::LParen | TokenKind::Colon | TokenKind::Eq | TokenKind::Semicolon
                )
                && !self.peeked.has_preceding_line_terminator
            {
                self.advance()?;
            }
            let is_static = if self.check(&TokenKind::Static) {
                if self.peeked.kind == TokenKind::LParen
                    || self.peeked.kind == TokenKind::Colon
                    || self.peeked.kind == TokenKind::Eq
                {
                    false
                } else {
                    self.advance()?;
                    true
                }
            } else {
                false
            };
            while matches!(&self.current.kind, TokenKind::Ident(id) if id == "readonly")
                && !matches!(
                    &self.peeked.kind,
                    TokenKind::LParen | TokenKind::Colon | TokenKind::Eq | TokenKind::Semicolon
                )
                && !self.peeked.has_preceding_line_terminator
            {
                self.advance()?;
            }

            if is_static && self.check(&TokenKind::LBrace) {
                let prev_in_func = self.in_function;
                let prev_in_gen = self.in_generator;
                let prev_in_method = self.in_method;
                let prev_in_async = self.in_async;
                let prev_in_static = self.in_static_block;
                self.in_function = false;
                self.in_generator = false;
                self.in_method = true;
                self.in_async = false;
                self.in_static_block = true;
                let body = match self.parse_block_statement()? {
                    Statement::Block(stmts) => stmts,
                    other => vec![other],
                };
                self.in_function = prev_in_func;
                self.in_generator = prev_in_gen;
                self.in_method = prev_in_method;
                self.in_async = prev_in_async;
                self.in_static_block = prev_in_static;
                self.validate_static_block(&body)?;
                elements.push(ClassElement {
                    is_static: true,
                    key: PropertyKey::Literal(String::new()),
                    kind: ClassElementKind::StaticBlock(body),
                });
                continue;
            }

            let is_async = if self.check(&TokenKind::Async) {
                if self.peeked.kind == TokenKind::LParen
                    || self.peeked.kind == TokenKind::Colon
                    || self.peeked.kind == TokenKind::Eq
                    || self.peeked.kind == TokenKind::Semicolon
                    || self.peeked.has_preceding_line_terminator
                {
                    false
                } else {
                    self.advance()?;
                    true
                }
            } else {
                false
            };

            let is_generator = self.match_token(&TokenKind::Star)?;

            if !is_generator
                && (self.check_ident("get") || self.check_ident("set"))
                && self.peeked.kind != TokenKind::LParen
                && self.peeked.kind != TokenKind::Semicolon
                && self.peeked.kind != TokenKind::RBrace
                && self.peeked.kind != TokenKind::Colon
                && self.peeked.kind != TokenKind::Eq
                && self.peeked.kind != TokenKind::Star
                && !self.peeked.has_preceding_line_terminator
            {
                let is_getter = self.check_ident("get");
                self.advance()?; // 'get' or 'set'
                let mut key = self.parse_property_name()?;
                if let PropertyKey::Literal(ref k) = key {
                    if k == "#constructor" {
                        return Err(ParseError::General(
                            "Class cannot have private member '#constructor'".to_string(),
                        ));
                    }
                    if is_static && k == "prototype" {
                        return Err(ParseError::General(
                            "Classes may not have a static property named 'prototype'".to_string(),
                        ));
                    }
                    if !is_static && k == "constructor" {
                        return Err(ParseError::General(
                            "Class constructor may not be an accessor".to_string(),
                        ));
                    }
                    if k.starts_with('#') {
                        let kind_bit = if is_getter { 2u8 } else { 4u8 };
                        if let Some((prev_static, prev_kind)) = declared_privates.get_mut(k) {
                            if *prev_static != is_static
                                || (*prev_kind & 1) != 0
                                || (*prev_kind & kind_bit) != 0
                            {
                                return Err(ParseError::General(format!(
                                    "Identifier '{}' has already been declared",
                                    k
                                )));
                            }
                            *prev_kind |= kind_bit;
                        } else {
                            declared_privates.insert(k.clone(), (is_static, kind_bit));
                        }
                    }
                }
                if let PropertyKey::Literal(ref mut k) = key {
                    if k.starts_with('#') {
                        if let Some(unique) = self.resolve_private_name(k) {
                            *k = unique;
                        }
                    }
                }
                if is_getter {
                    self.expect(TokenKind::LParen)?;
                    self.expect(TokenKind::RParen)?;
                    if self.match_token(&TokenKind::Colon)? {
                        self.skip_type_annotation()?;
                    }
                    let prev_ctor = self.in_derived_constructor;
                    let prev_method = self.in_method;
                    let prev_gen = self.in_generator;
                    let prev_async = self.in_async;
                    self.in_method = true;
                    self.in_derived_constructor = false;
                    self.in_generator = false;
                    self.in_async = false;
                    let body = self.parse_function_body(&[], None)?;
                    self.in_generator = prev_gen;
                    self.in_async = prev_async;
                    self.in_method = prev_method;
                    self.in_derived_constructor = prev_ctor;
                    let priv_func_name = if matches!(&key, PropertyKey::Literal(k) if k.starts_with('#'))
                    {
                        let PropertyKey::Literal(k) = &key else { unreachable!() };
                        let priv_name = k.split('@').next().unwrap_or(k);
                        Some(format!("get {}", priv_name))
                    } else {
                        None
                    };
                    if !is_static && matches!(&key, PropertyKey::Literal(k) if k.starts_with('#')) {
                        if let PropertyKey::Literal(k) = &key {
                            instance_fields.push(Statement::Expression(
                                Expression::DefinePrivateAccessor {
                                    target: Box::new(Expression::This),
                                    property: k.clone(),
                                    is_getter: true,
                                    accessor: Box::new(Expression::MethodExpr {
                                        name: priv_func_name.clone(),
                                        params: vec![],
                                        body: body.clone(),
                                    }),
                                },
                            ));
                        }
                    }
                    elements.push(ClassElement {
                        is_static,
                        key,
                        kind: ClassElementKind::Getter(Expression::MethodExpr {
                            name: priv_func_name,
                            params: vec![],
                            body,
                        }),
                    });
                } else {
                    let prev_ctor = self.in_derived_constructor;
                    let prev_method = self.in_method;
                    let prev_gen = self.in_generator;
                    let prev_async = self.in_async;
                    self.in_method = true;
                    self.in_derived_constructor = false;
                    self.in_generator = false;
                    self.in_async = false;
                    let (params, defaults) = self.parse_formal_parameters()?;
                    if self.match_token(&TokenKind::Colon)? {
                        self.skip_type_annotation()?;
                    }
                    if params.len() != 1 || params[0].starts_with("...") {
                        return Err(ParseError::General(
                            "Setter must have exactly one formal parameter".to_string(),
                        ));
                    }
                    let mut body = self.parse_function_body(&params, None)?;
                    self.in_generator = prev_gen;
                    self.in_async = prev_async;
                    self.in_method = prev_method;
                    self.in_derived_constructor = prev_ctor;
                    Self::apply_parameter_defaults(&mut body, defaults);
                    let priv_func_name = if matches!(&key, PropertyKey::Literal(k) if k.starts_with('#'))
                    {
                        let PropertyKey::Literal(k) = &key else { unreachable!() };
                        let priv_name = k.split('@').next().unwrap_or(k);
                        Some(format!("set {}", priv_name))
                    } else {
                        None
                    };
                    if !is_static && matches!(&key, PropertyKey::Literal(k) if k.starts_with('#')) {
                        if let PropertyKey::Literal(k) = &key {
                            instance_fields.push(Statement::Expression(
                                Expression::DefinePrivateAccessor {
                                    target: Box::new(Expression::This),
                                    property: k.clone(),
                                    is_getter: false,
                                    accessor: Box::new(Expression::MethodExpr {
                                        name: priv_func_name.clone(),
                                        params: params.clone(),
                                        body: body.clone(),
                                    }),
                                },
                            ));
                        }
                    }
                    elements.push(ClassElement {
                        is_static,
                        key,
                        kind: ClassElementKind::Setter(Expression::MethodExpr {
                            name: priv_func_name,
                            params,
                            body,
                        }),
                    });
                }
                continue;
            }

            let mut key = self.parse_property_name()?;
            if let PropertyKey::Literal(ref k) = key {
                if k == "#constructor" {
                    return Err(ParseError::General(
                        "Class cannot have private member '#constructor'".to_string(),
                    ));
                }
                if is_static && k == "prototype" {
                    return Err(ParseError::General(
                        "Classes may not have a static property named 'prototype'".to_string(),
                    ));
                }
                if k.starts_with('#') {
                    if declared_privates.contains_key(k) {
                        return Err(ParseError::General(format!(
                            "Identifier '{}' has already been declared",
                            k
                        )));
                    }
                    declared_privates.insert(k.clone(), (is_static, 1u8));
                }
            }
            if let PropertyKey::Literal(ref mut k) = key {
                if k.starts_with('#') {
                    if let Some(unique) = self.resolve_private_name(k) {
                        *k = unique;
                    }
                }
            }
            if matches!(&key, PropertyKey::Literal(k) if k == "constructor") {
                if !is_static {
                    if is_generator || is_async {
                        return Err(ParseError::General(
                            "Class constructor may not be an async method or generator".to_string(),
                        ));
                    }
                    if !self.check(&TokenKind::LParen) {
                        return Err(ParseError::General(
                            "Classes may not have a field named 'constructor'".to_string(),
                        ));
                    }
                    if has_ctor {
                        return Err(ParseError::General(
                            "A class may only have one constructor".to_string(),
                        ));
                    }
                    let prev_ctor = self.in_derived_constructor;
                    let prev_method = self.in_method;
                    let prev_gen = self.in_generator;
                    let prev_async = self.in_async;
                    self.in_derived_constructor = is_derived;
                    self.in_method = false;
                    self.in_generator = false;
                    self.in_async = false;
                    let (params, defaults) = self.parse_formal_parameters()?;
                    if self.match_token(&TokenKind::Colon)? {
                        self.skip_type_annotation()?;
                    }
                    let mut body = self.parse_function_body(&params, None)?;
                    self.in_generator = prev_gen;
                    self.in_async = prev_async;
                    self.in_derived_constructor = prev_ctor;
                    self.in_method = prev_method;
                    Self::apply_parameter_defaults(&mut body, defaults);
                    ctor_params = params;
                    ctor_body = body;
                    has_ctor = true;
                    continue;
                }
            }

            if self.check(&TokenKind::Lt) {
                self.skip_type_parameters()?;
            }

            if self.check(&TokenKind::LParen) {
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                let prev_gen = self.in_generator;
                let prev_async = self.in_async;
                self.in_method = true;
                self.in_derived_constructor = false;
                self.in_generator = is_generator;
                self.in_async = is_async;
                let (params, defaults) = self.parse_formal_parameters()?;
                if self.match_token(&TokenKind::Colon)? {
                    self.skip_type_annotation()?;
                }
                let mut body = self.parse_function_body(&params, None)?;
                self.in_generator = prev_gen;
                self.in_async = prev_async;
                self.in_derived_constructor = prev_ctor;
                self.in_method = prev_method;
                Self::apply_parameter_defaults(&mut body, defaults);
                let priv_func_name = if matches!(&key, PropertyKey::Literal(k) if k.starts_with('#'))
                {
                    let PropertyKey::Literal(k) = &key else { unreachable!() };
                    Some(k.split('@').next().unwrap_or(k).to_string())
                } else {
                    None
                };
                let method_expr = if is_async && is_generator {
                    Expression::AsyncGeneratorExpr { name: priv_func_name, params, body }
                } else if is_async {
                    Expression::AsyncFunctionExpr { name: priv_func_name, params, body }
                } else if is_generator {
                    Expression::GeneratorExpr { name: priv_func_name, params, body }
                } else {
                    Expression::MethodExpr { name: priv_func_name, params, body }
                };
                if !is_static && matches!(&key, PropertyKey::Literal(k) if k.starts_with('#')) {
                    if let PropertyKey::Literal(k) = &key {
                        instance_fields.push(Statement::Expression(
                            Expression::DefinePrivateMethod {
                                target: Box::new(Expression::This),
                                property: k.clone(),
                                value: Box::new(method_expr.clone()),
                            },
                        ));
                    }
                }
                elements.push(ClassElement {
                    is_static,
                    key,
                    kind: ClassElementKind::Method(method_expr),
                });
            } else {
                let _ = self.match_token(&TokenKind::Question)?;
                let _ = self.match_token(&TokenKind::Bang)?;
                if self.match_token(&TokenKind::Colon)? {
                    self.skip_type_annotation()?;
                }
                let init_opt = if self.match_token(&TokenKind::Eq)? {
                    let expr = self.parse_expression(0)?;
                    self.check_field_initializer(&expr)?;
                    Some(expr)
                } else {
                    None
                };
                if !self.match_token(&TokenKind::Semicolon)? {
                    if !self.check(&TokenKind::RBrace)
                        && !self.check(&TokenKind::Eof)
                        && !self.current.has_preceding_line_terminator
                    {
                        return Err(ParseError::UnexpectedToken(
                            self.current.kind.clone(),
                            self.current.line,
                            self.current.col,
                        ));
                    }
                }
                elements.push(ClassElement {
                    is_static,
                    key: key.clone(),
                    kind: ClassElementKind::Field(init_opt.clone()),
                });
                if !is_static {
                    let init_val = init_opt.unwrap_or(Expression::Literal(Literal::Undefined));
                    let init_stmt = match key {
                        PropertyKey::Literal(k) if k.starts_with('#') => {
                            Statement::Expression(Expression::DefineField {
                                target: Box::new(Expression::This),
                                property: k,
                                value: Box::new(init_val),
                            })
                        }
                        PropertyKey::Literal(k) => {
                            Statement::Expression(Expression::MemberAssignment {
                                object: Box::new(Expression::This),
                                property: k,
                                value: Box::new(init_val),
                            })
                        }
                        PropertyKey::Computed(expr) => {
                            Statement::Expression(Expression::IndexAssignment {
                                object: Box::new(Expression::This),
                                index: Box::new(expr),
                                value: Box::new(init_val),
                            })
                        }
                    };
                    instance_fields.push(init_stmt);
                }
            }
        }
        self.expect(TokenKind::RBrace)?;
        self.private_names_stack.pop();
        self.is_strict = prev_strict;
        self.in_static_block = prev_in_static;
        let _ = self.match_token(&TokenKind::Semicolon)?;

        if !instance_fields.is_empty() {
            if !has_ctor {
                ctor_body = instance_fields;
            } else if is_derived {
                let super_pos = ctor_body.iter().position(|stmt| {
                    if let Statement::Expression(Expression::Call { callee, .. }) = stmt {
                        matches!(**callee, Expression::Super)
                    } else {
                        false
                    }
                });
                if let Some(pos) = super_pos {
                    let mut new_body = Vec::new();
                    new_body.extend(ctor_body[..=pos].iter().cloned());
                    new_body.extend(instance_fields);
                    new_body.extend(ctor_body[pos + 1..].iter().cloned());
                    ctor_body = new_body;
                } else {
                    let mut new_body = instance_fields;
                    new_body.extend(ctor_body);
                    ctor_body = new_body;
                }
            } else {
                let mut new_body = instance_fields;
                new_body.extend(ctor_body);
                ctor_body = new_body;
            }
        }

        Ok((ctor_params, ctor_body, has_ctor, elements))
    }

    pub(crate) fn validate_static_block(&self, stmts: &[Statement]) -> Result<(), ParseError> {
        Self::validate_labels_and_jumps(stmts)?;
        for stmt in stmts {
            self.check_static_block_stmt(stmt)?;
        }
        Ok(())
    }

    fn check_static_block_stmt(&self, stmt: &Statement) -> Result<(), ParseError> {
        match stmt {
            Statement::Expression(expr) => self.check_static_block_expr(expr),
            Statement::Return(_) => Err(ParseError::UnexpectedToken(TokenKind::Return, 0, 0)),
            Statement::Block(inner) => {
                for s in inner {
                    self.check_static_block_stmt(s)?;
                }
                Ok(())
            }
            Statement::If { condition, then_branch, else_branch } => {
                self.check_static_block_expr(condition)?;
                self.check_static_block_stmt(then_branch)?;
                if let Some(eb) = else_branch {
                    self.check_static_block_stmt(eb)?;
                }
                Ok(())
            }
            Statement::While { condition, body } => {
                self.check_static_block_expr(condition)?;
                self.check_static_block_stmt(body)
            }
            Statement::DoWhile { body, condition } => {
                self.check_static_block_stmt(body)?;
                self.check_static_block_expr(condition)
            }
            Statement::For { init, condition, update, body } => {
                if let Some(init_s) = init {
                    self.check_static_block_stmt(init_s)?;
                }
                if let Some(c) = condition {
                    self.check_static_block_expr(c)?;
                }
                if let Some(u) = update {
                    self.check_static_block_expr(u)?;
                }
                self.check_static_block_stmt(body)
            }
            Statement::ForIn { left, right, body, .. } => {
                if left == "arguments" {
                    return Err(ParseError::General(
                        "Static initialization block cannot use 'arguments'".into(),
                    ));
                }
                self.check_static_block_expr(right)?;
                self.check_static_block_stmt(body)
            }
            Statement::ForOf { left, right, body, is_await, .. } => {
                if *is_await {
                    return Err(ParseError::UnexpectedToken(TokenKind::Await, 0, 0));
                }
                if left == "arguments" {
                    return Err(ParseError::General(
                        "Static initialization block cannot use 'arguments'".into(),
                    ));
                }
                self.check_static_block_expr(right)?;
                self.check_static_block_stmt(body)
            }
            Statement::VarDecl { declarations, .. } => {
                for d in declarations {
                    if d.name == "arguments" || d.name == "await" {
                        return Err(ParseError::General(format!(
                            "Static initialization block cannot declare '{}'",
                            d.name
                        )));
                    }
                    if let Some(ref init) = d.init {
                        self.check_static_block_expr(init)?;
                    }
                }
                Ok(())
            }
            Statement::Switch { discriminant, cases } => {
                self.check_static_block_expr(discriminant)?;
                for c in cases {
                    if let Some(ref test) = c.test {
                        self.check_static_block_expr(test)?;
                    }
                    for s in &c.consequent {
                        self.check_static_block_stmt(s)?;
                    }
                }
                Ok(())
            }
            Statement::TryCatch { try_block, catch_param, catch_block, finally_block } => {
                if catch_param.as_deref() == Some("arguments")
                    || catch_param.as_deref() == Some("await")
                {
                    return Err(ParseError::General(
                        "Static initialization block cannot bind 'arguments' or 'await'".into(),
                    ));
                }
                self.check_static_block_stmt(try_block)?;
                if let Some(cb) = catch_block {
                    self.check_static_block_stmt(cb)?;
                }
                if let Some(fb) = finally_block {
                    self.check_static_block_stmt(fb)?;
                }
                Ok(())
            }
            Statement::Throw(expr) => self.check_static_block_expr(expr),
            Statement::With { object, body } => {
                self.check_static_block_expr(object)?;
                self.check_static_block_stmt(body)
            }
            Statement::Labeled { label, body } => {
                if label == "await" {
                    return Err(ParseError::UnexpectedToken(TokenKind::Await, 0, 0));
                }
                self.check_static_block_stmt(body)
            }
            _ => Ok(()),
        }
    }

    fn check_static_block_expr(&self, expr: &Expression) -> Result<(), ParseError> {
        match expr {
            Expression::Identifier(name) if name == "arguments" => Err(ParseError::General(
                "Static initialization block cannot use 'arguments'".into(),
            )),
            Expression::Identifier(name) if name == "await" => {
                Err(ParseError::UnexpectedToken(TokenKind::Await, 0, 0))
            }
            Expression::Await(_) => Err(ParseError::UnexpectedToken(TokenKind::Await, 0, 0)),
            Expression::Yield(_) | Expression::YieldStar(_) => {
                Err(ParseError::UnexpectedToken(TokenKind::Yield, 0, 0))
            }
            Expression::Binary { left, right, .. } => {
                self.check_static_block_expr(left)?;
                self.check_static_block_expr(right)
            }
            Expression::Unary { expr, .. } => self.check_static_block_expr(expr),
            Expression::Assignment { target, value } => {
                if target == "arguments" || target == "await" {
                    return Err(ParseError::General(format!(
                        "Static initialization block cannot assign to '{}'",
                        target
                    )));
                }
                self.check_static_block_expr(value)
            }
            Expression::MemberAssignment { object, value, .. } => {
                self.check_static_block_expr(object)?;
                self.check_static_block_expr(value)
            }
            Expression::IndexAssignment { object, index, value } => {
                self.check_static_block_expr(object)?;
                self.check_static_block_expr(index)?;
                self.check_static_block_expr(value)
            }
            Expression::Call { callee, args } | Expression::New { callee, args } => {
                self.check_static_block_expr(callee)?;
                for a in args {
                    self.check_static_block_expr(a)?;
                }
                Ok(())
            }
            Expression::Member { object, .. } => self.check_static_block_expr(object),
            Expression::Index { object, index } => {
                self.check_static_block_expr(object)?;
                self.check_static_block_expr(index)
            }
            Expression::Array(elements) => {
                for el in elements {
                    self.check_static_block_expr(el)?;
                }
                Ok(())
            }
            Expression::Object(props) => {
                for prop in props {
                    match prop {
                        ObjectProperty::KeyValue(key, val) => {
                            if let PropertyKey::Computed(k) = key {
                                self.check_static_block_expr(k)?;
                            }
                            self.check_static_block_expr(val)?;
                        }
                        ObjectProperty::CoverInitializedName(_, val) => {
                            self.check_static_block_expr(val)?;
                        }
                        ObjectProperty::Spread(inner) => {
                            self.check_static_block_expr(inner)?;
                        }
                        _ => {}
                    }
                }
                Ok(())
            }
            Expression::Conditional { condition, then_expr, else_expr } => {
                self.check_static_block_expr(condition)?;
                self.check_static_block_expr(then_expr)?;
                self.check_static_block_expr(else_expr)
            }
            Expression::Sequence(exprs) => {
                for e in exprs {
                    self.check_static_block_expr(e)?;
                }
                Ok(())
            }
            Expression::Grouping(inner) => self.check_static_block_expr(inner),
            Expression::Spread(inner) => self.check_static_block_expr(inner),
            Expression::Update { expr, .. } => self.check_static_block_expr(expr),
            Expression::ClassExpr(class_def) => {
                for elem in &class_def.elements {
                    if let PropertyKey::Computed(k) = &elem.key {
                        self.check_static_block_expr(k)?;
                    }
                }
                Ok(())
            }
            Expression::ArrowExpr { params, body } => {
                for p in params {
                    if p == "arguments" {
                        return Err(ParseError::General("cannot use 'arguments'".into()));
                    }
                }
                fn arrow_contains_args_stmt(stmt: &Statement) -> bool {
                    match stmt {
                        Statement::Expression(e)
                        | Statement::Return(Some(e))
                        | Statement::Throw(e) => arrow_contains_args_expr(e),
                        Statement::Block(stmts) => stmts.iter().any(arrow_contains_args_stmt),
                        Statement::If { condition, then_branch, else_branch } => {
                            arrow_contains_args_expr(condition)
                                || arrow_contains_args_stmt(then_branch)
                                || else_branch
                                    .as_ref()
                                    .map_or(false, |b| arrow_contains_args_stmt(b))
                        }
                        Statement::While { condition, body }
                        | Statement::DoWhile { condition, body } => {
                            arrow_contains_args_expr(condition) || arrow_contains_args_stmt(body)
                        }
                        _ => false,
                    }
                }
                fn arrow_contains_args_expr(expr: &Expression) -> bool {
                    match expr {
                        Expression::Identifier(name) if name == "arguments" => true,
                        Expression::Binary { left, right, .. } => {
                            arrow_contains_args_expr(left) || arrow_contains_args_expr(right)
                        }
                        Expression::Unary { expr, .. } => arrow_contains_args_expr(expr),
                        Expression::Assignment { target, value } => {
                            target == "arguments" || arrow_contains_args_expr(value)
                        }
                        Expression::Call { callee, args } | Expression::New { callee, args } => {
                            arrow_contains_args_expr(callee)
                                || args.iter().any(arrow_contains_args_expr)
                        }
                        Expression::Member { object, .. } => arrow_contains_args_expr(object),
                        Expression::Index { object, index } => {
                            arrow_contains_args_expr(object) || arrow_contains_args_expr(index)
                        }
                        Expression::Array(elements) => {
                            elements.iter().any(arrow_contains_args_expr)
                        }
                        Expression::Object(props) => props.iter().any(|p| match p {
                            ObjectProperty::KeyValue(key, val) => {
                                (match key {
                                    PropertyKey::Computed(k) => arrow_contains_args_expr(k),
                                    _ => false,
                                }) || arrow_contains_args_expr(val)
                            }
                            ObjectProperty::CoverInitializedName(_, val) => {
                                arrow_contains_args_expr(val)
                            }
                            ObjectProperty::Spread(inner) => arrow_contains_args_expr(inner),
                            _ => false,
                        }),
                        Expression::Grouping(inner) => arrow_contains_args_expr(inner),
                        Expression::Conditional { condition, then_expr, else_expr } => {
                            arrow_contains_args_expr(condition)
                                || arrow_contains_args_expr(then_expr)
                                || arrow_contains_args_expr(else_expr)
                        }
                        _ => false,
                    }
                }
                for s in body {
                    if arrow_contains_args_stmt(s) {
                        return Err(ParseError::General("cannot use 'arguments'".into()));
                    }
                }
                Ok(())
            }
            Expression::TaggedTemplate { tag, exprs, .. } => {
                self.check_static_block_expr(tag)?;
                for e in exprs {
                    self.check_static_block_expr(e)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub(crate) fn check_field_initializer(&self, expr: &Expression) -> Result<(), ParseError> {
        self.check_field_expr(expr)
    }

    fn check_field_expr(&self, expr: &Expression) -> Result<(), ParseError> {
        match expr {
            Expression::Identifier(name) if name == "arguments" => {
                Err(ParseError::General("Class field initializer cannot use 'arguments'".into()))
            }
            Expression::Super => {
                Err(ParseError::General("Class field initializer cannot call super()".into()))
            }
            Expression::Binary { left, right, .. } => {
                self.check_field_expr(left)?;
                self.check_field_expr(right)
            }
            Expression::Unary { expr, .. } => self.check_field_expr(expr),
            Expression::Assignment { target, value } => {
                if target == "arguments" {
                    return Err(ParseError::General(
                        "Class field initializer cannot assign to 'arguments'".into(),
                    ));
                }
                self.check_field_expr(value)
            }
            Expression::MemberAssignment { object, value, .. } => {
                self.check_field_expr(object)?;
                self.check_field_expr(value)
            }
            Expression::IndexAssignment { object, index, value } => {
                self.check_field_expr(object)?;
                self.check_field_expr(index)?;
                self.check_field_expr(value)
            }
            Expression::Call { callee, args } | Expression::New { callee, args } => {
                if matches!(callee.as_ref(), Expression::Super) {
                    return Err(ParseError::General(
                        "Class field initializer cannot call super()".into(),
                    ));
                }
                self.check_field_expr(callee)?;
                for a in args {
                    self.check_field_expr(a)?;
                }
                Ok(())
            }
            Expression::Member { object, .. } => self.check_field_expr(object),
            Expression::Index { object, index } => {
                self.check_field_expr(object)?;
                self.check_field_expr(index)
            }
            Expression::Array(elements) => {
                for el in elements {
                    self.check_field_expr(el)?;
                }
                Ok(())
            }
            Expression::Object(props) => {
                for prop in props {
                    match prop {
                        ObjectProperty::KeyValue(key, val) => {
                            if let PropertyKey::Computed(k) = key {
                                self.check_field_expr(k)?;
                            }
                            self.check_field_expr(val)?;
                        }
                        ObjectProperty::Spread(inner) => {
                            self.check_field_expr(inner)?;
                        }
                        _ => {}
                    }
                }
                Ok(())
            }
            Expression::Conditional { condition, then_expr, else_expr } => {
                self.check_field_expr(condition)?;
                self.check_field_expr(then_expr)?;
                self.check_field_expr(else_expr)
            }
            Expression::Sequence(exprs) => {
                for e in exprs {
                    self.check_field_expr(e)?;
                }
                Ok(())
            }
            Expression::Grouping(inner) => self.check_field_expr(inner),
            Expression::Spread(inner) => self.check_field_expr(inner),
            Expression::Update { expr, .. } => self.check_field_expr(expr),
            Expression::ArrowExpr { params, body } => {
                for p in params {
                    if p == "arguments" {
                        return Err(ParseError::General(
                            "Class field initializer cannot use 'arguments'".into(),
                        ));
                    }
                }
                for s in body {
                    self.check_field_stmt(s)?;
                }
                Ok(())
            }
            Expression::TaggedTemplate { tag, exprs, .. } => {
                self.check_field_expr(tag)?;
                for e in exprs {
                    self.check_field_expr(e)?;
                }
                Ok(())
            }
            Expression::ClassExpr(class_def) => {
                for elem in &class_def.elements {
                    if let PropertyKey::Computed(k) = &elem.key {
                        self.check_field_expr(k)?;
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn check_field_stmt(&self, stmt: &Statement) -> Result<(), ParseError> {
        match stmt {
            Statement::Expression(expr) => self.check_field_expr(expr),
            Statement::Return(Some(expr)) => self.check_field_expr(expr),
            Statement::Block(inner) => {
                for s in inner {
                    self.check_field_stmt(s)?;
                }
                Ok(())
            }
            Statement::If { condition, then_branch, else_branch } => {
                self.check_field_expr(condition)?;
                self.check_field_stmt(then_branch)?;
                if let Some(eb) = else_branch {
                    self.check_field_stmt(eb)?;
                }
                Ok(())
            }
            Statement::While { condition, body } => {
                self.check_field_expr(condition)?;
                self.check_field_stmt(body)
            }
            Statement::DoWhile { body, condition } => {
                self.check_field_stmt(body)?;
                self.check_field_expr(condition)
            }
            Statement::For { init, condition, update, body } => {
                if let Some(init_s) = init {
                    self.check_field_stmt(init_s)?;
                }
                if let Some(c) = condition {
                    self.check_field_expr(c)?;
                }
                if let Some(u) = update {
                    self.check_field_expr(u)?;
                }
                self.check_field_stmt(body)
            }
            Statement::ForIn { left, right, body, .. } => {
                if left == "arguments" {
                    return Err(ParseError::General(
                        "Class field initializer cannot use 'arguments'".into(),
                    ));
                }
                self.check_field_expr(right)?;
                self.check_field_stmt(body)
            }
            Statement::ForOf { left, right, body, .. } => {
                if left == "arguments" {
                    return Err(ParseError::General(
                        "Class field initializer cannot use 'arguments'".into(),
                    ));
                }
                self.check_field_expr(right)?;
                self.check_field_stmt(body)
            }
            Statement::VarDecl { declarations, .. } => {
                for d in declarations {
                    if d.name == "arguments" {
                        return Err(ParseError::General(
                            "Class field initializer cannot declare 'arguments'".into(),
                        ));
                    }
                    if let Some(ref init) = d.init {
                        self.check_field_expr(init)?;
                    }
                }
                Ok(())
            }
            Statement::Switch { discriminant, cases } => {
                self.check_field_expr(discriminant)?;
                for c in cases {
                    if let Some(ref test) = c.test {
                        self.check_field_expr(test)?;
                    }
                    for s in &c.consequent {
                        self.check_field_stmt(s)?;
                    }
                }
                Ok(())
            }
            Statement::TryCatch { try_block, catch_param, catch_block, finally_block } => {
                if catch_param.as_deref() == Some("arguments") {
                    return Err(ParseError::General(
                        "Class field initializer cannot bind 'arguments'".into(),
                    ));
                }
                self.check_field_stmt(try_block)?;
                if let Some(cb) = catch_block {
                    self.check_field_stmt(cb)?;
                }
                if let Some(fb) = finally_block {
                    self.check_field_stmt(fb)?;
                }
                Ok(())
            }
            Statement::Throw(expr) => self.check_field_expr(expr),
            _ => Ok(()),
        }
    }
}
