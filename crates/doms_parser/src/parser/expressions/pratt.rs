use super::*;

impl<'a> Parser<'a> {
    pub fn parse_expression(&mut self, min_bp: u8) -> Result<Expression, ParseError> {
        let mut lhs = self.parse_prefix(min_bp)?;

        loop {
            // Unparenthesized Yield expressions cannot be followed by any postfix or infix operator except Comma
            if matches!(lhs, Expression::Yield(_) | Expression::YieldStar(_)) {
                if min_bp == 0 && self.current.kind == TokenKind::Comma {
                    // comma sequence expression allowed
                } else {
                    break;
                }
            }

            // Postfix / Infix operations
            if let TokenKind::TemplateLiteral { quasis, raw_quasis, raw_exprs } = &self.current.kind
            {
                if matches!(lhs, Expression::OptionalChain(_)) {
                    return Err(ParseError::UnexpectedToken(
                        self.current.kind.clone(),
                        self.current.line,
                        self.current.col,
                    ));
                }
                let quasis = quasis.clone();
                let raw_quasis = raw_quasis.clone();
                let raw_exprs = raw_exprs.clone();
                self.advance()?;
                let mut exprs = Vec::new();
                for raw in &raw_exprs {
                    let sub = Parser::new(raw)?.parse_expression(0)?;
                    exprs.push(sub);
                }
                lhs = Expression::TaggedTemplate { tag: Box::new(lhs), quasis, raw_quasis, exprs };
                continue;
            }

            if self.check(&TokenKind::QuestionDot) {
                self.advance()?; // consume ?.
                if matches!(self.current.kind, TokenKind::TemplateLiteral { .. }) {
                    return Err(ParseError::UnexpectedToken(
                        self.current.kind.clone(),
                        self.current.line,
                        self.current.col,
                    ));
                }

                let element = if self.check(&TokenKind::LParen) {
                    self.advance()?;
                    let args = self.parse_call_args()?;
                    ChainElement::Call { args, optional: true }
                } else if self.check(&TokenKind::LBracket) {
                    self.advance()?;
                    let index = self.parse_expression(0)?;
                    self.expect(TokenKind::RBracket)?;
                    ChainElement::Index { index: Box::new(index), optional: true }
                } else {
                    let prop_tok = self.advance()?;
                    let mut prop = match prop_tok.kind.as_prop_name() {
                        Some(id) => id,
                        None => {
                            return Err(ParseError::UnexpectedToken(
                                prop_tok.kind,
                                prop_tok.line,
                                prop_tok.col,
                            ));
                        }
                    };
                    if prop.starts_with('#') {
                        prop = match self.resolve_private_name(&prop) {
                            Some(scoped) => scoped,
                            None if self.is_eval => prop,
                            None => {
                                return Err(ParseError::UnexpectedToken(
                                    prop_tok.kind,
                                    prop_tok.line,
                                    prop_tok.col,
                                ));
                            }
                        };
                    }
                    ChainElement::Property { name: prop, optional: true }
                };

                match lhs {
                    Expression::OptionalChain(ref mut oc) => {
                        oc.chain.push(element);
                    }
                    _ => {
                        lhs = Expression::OptionalChain(OptionalChain {
                            base: Box::new(lhs),
                            chain: vec![element],
                        });
                    }
                }
                continue;
            }

            if self.check(&TokenKind::LParen) {
                if matches!(lhs, Expression::Super) && !self.in_derived_constructor {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::Super,
                        self.current.line,
                        self.current.col,
                    ));
                }
                // Function call
                self.advance()?;
                let args = self.parse_call_args()?;
                match lhs {
                    Expression::OptionalChain(ref mut oc) => {
                        oc.chain.push(ChainElement::Call { args, optional: false });
                    }
                    _ => {
                        lhs = Expression::Call { callee: Box::new(lhs), args };
                    }
                }
                continue;
            }

            if self.check(&TokenKind::Dot) {
                if matches!(lhs, Expression::Super)
                    && !self.in_method
                    && !self.in_derived_constructor
                {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::Super,
                        self.current.line,
                        self.current.col,
                    ));
                }
                self.advance()?;
                let prop_tok = self.advance()?;
                let mut prop = match prop_tok.kind.as_prop_name() {
                    Some(id) => id,
                    None => {
                        return Err(ParseError::UnexpectedToken(
                            prop_tok.kind,
                            prop_tok.line,
                            prop_tok.col,
                        ));
                    }
                };
                if prop.starts_with('#') {
                    if matches!(&lhs, Expression::Super) {
                        return Err(ParseError::UnexpectedToken(
                            prop_tok.kind,
                            prop_tok.line,
                            prop_tok.col,
                        ));
                    }
                    prop = match self.resolve_private_name(&prop) {
                        Some(scoped) => scoped,
                        None if self.is_eval => prop,
                        None => {
                            return Err(ParseError::UnexpectedToken(
                                prop_tok.kind,
                                prop_tok.line,
                                prop_tok.col,
                            ));
                        }
                    };
                }
                match lhs {
                    Expression::OptionalChain(ref mut oc) => {
                        oc.chain.push(ChainElement::Property { name: prop, optional: false });
                    }
                    _ => {
                        lhs = Expression::Member { object: Box::new(lhs), property: prop };
                    }
                }
                continue;
            }

            if self.check(&TokenKind::LBracket) {
                if matches!(lhs, Expression::Super)
                    && !self.in_method
                    && !self.in_derived_constructor
                {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::Super,
                        self.current.line,
                        self.current.col,
                    ));
                }
                // Index access: lhs[expr]
                self.advance()?;
                let index = self.parse_expression(0)?;
                self.expect(TokenKind::RBracket)?;
                match lhs {
                    Expression::OptionalChain(ref mut oc) => {
                        oc.chain
                            .push(ChainElement::Index { index: Box::new(index), optional: false });
                    }
                    _ => {
                        lhs = Expression::Index { object: Box::new(lhs), index: Box::new(index) };
                    }
                }
                continue;
            }

            if matches!(lhs, Expression::Super) {
                return Err(ParseError::UnexpectedToken(
                    TokenKind::Super,
                    self.current.line,
                    self.current.col,
                ));
            }

            // TypeScript non-null assertion: expr!
            if self.check(&TokenKind::Bang) && !self.current.has_preceding_line_terminator {
                self.advance()?;
                continue;
            }

            // TypeScript type assertion: expr as Type or expr satisfies Type
            if (self.check(&TokenKind::As)
                || matches!(&self.current.kind, TokenKind::Ident(id) if id == "satisfies"))
                && !self.current.has_preceding_line_terminator
            {
                self.advance()?;
                self.skip_type_annotation()?;
                continue;
            }

            if self.check(&TokenKind::Question) {
                if min_bp > 1 {
                    break;
                }
                self.advance()?; // ?
                let then_expr = self.parse_expression(1)?;
                self.expect(TokenKind::Colon)?;
                let else_expr = self.parse_expression(1)?;
                lhs = Expression::Conditional {
                    condition: Box::new(lhs),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                };
                continue;
            }

            if self.check(&TokenKind::PlusPlus) && !self.current.has_preceding_line_terminator {
                self.advance()?;
                let target = unwrap_grouping(&lhs);
                if !matches!(
                    target,
                    Expression::Identifier(_)
                        | Expression::Member { .. }
                        | Expression::Index { .. }
                ) {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::PlusPlus,
                        self.current.line,
                        self.current.col,
                    ));
                }
                if let Expression::Identifier(name) = target {
                    if self.is_strict && (name == "eval" || name == "arguments") {
                        return Err(ParseError::UnexpectedToken(
                            TokenKind::Ident(name.clone()),
                            self.current.line,
                            self.current.col,
                        ));
                    }
                }
                lhs =
                    Expression::Update { op: UpdateOp::Inc, is_prefix: false, expr: Box::new(lhs) };
                continue;
            }

            if self.check(&TokenKind::MinusMinus) && !self.current.has_preceding_line_terminator {
                self.advance()?;
                let target = unwrap_grouping(&lhs);
                if !matches!(
                    target,
                    Expression::Identifier(_)
                        | Expression::Member { .. }
                        | Expression::Index { .. }
                ) {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::MinusMinus,
                        self.current.line,
                        self.current.col,
                    ));
                }
                if let Expression::Identifier(name) = target {
                    if self.is_strict && (name == "eval" || name == "arguments") {
                        return Err(ParseError::UnexpectedToken(
                            TokenKind::Ident(name.clone()),
                            self.current.line,
                            self.current.col,
                        ));
                    }
                }
                lhs =
                    Expression::Update { op: UpdateOp::Dec, is_prefix: false, expr: Box::new(lhs) };
                continue;
            }

            let compound_bin_op = match self.current.kind {
                TokenKind::Eq => Some(None),
                TokenKind::PlusEq => Some(Some(BinaryOp::Add)),
                TokenKind::MinusEq => Some(Some(BinaryOp::Sub)),
                TokenKind::StarEq => Some(Some(BinaryOp::Mul)),
                TokenKind::SlashEq => Some(Some(BinaryOp::Div)),
                TokenKind::PercentEq => Some(Some(BinaryOp::Mod)),
                TokenKind::LtLtEq => Some(Some(BinaryOp::Shl)),
                TokenKind::GtGtEq => Some(Some(BinaryOp::Shr)),
                TokenKind::GtGtGtEq => Some(Some(BinaryOp::Ushr)),
                TokenKind::StarStarEq => Some(Some(BinaryOp::Exp)),
                TokenKind::AmpEq => Some(Some(BinaryOp::BitwiseAnd)),
                TokenKind::PipeEq => Some(Some(BinaryOp::BitwiseOr)),
                TokenKind::CaretEq => Some(Some(BinaryOp::BitwiseXor)),
                TokenKind::AmpAmpEq => Some(Some(BinaryOp::LogicalAnd)),
                TokenKind::PipePipeEq => Some(Some(BinaryOp::LogicalOr)),
                TokenKind::QuestionQuestionEq => Some(Some(BinaryOp::NullishCoalescing)),
                _ => None,
            };

            if let Some(opt_bin_op) = compound_bin_op {
                if min_bp > 1 {
                    break;
                }
                self.advance()?; // =, +=, -=, etc.
                let mut value = self.parse_expression(1)?;
                if let Some(bin_op) = opt_bin_op {
                    value = Expression::Binary {
                        op: bin_op,
                        left: Box::new(lhs.clone()),
                        right: Box::new(value),
                    };
                }
                let mut target_pattern = lhs;
                while let Expression::Grouping(inner) = target_pattern {
                    target_pattern = *inner;
                }
                match target_pattern {
                    Expression::Identifier(name) => {
                        if self.is_strict && (name == "eval" || name == "arguments") {
                            return Err(ParseError::UnexpectedToken(
                                TokenKind::Ident(name),
                                self.current.line,
                                self.current.col,
                            ));
                        }
                        lhs = Expression::Assignment { target: name, value: Box::new(value) };
                    }
                    Expression::Member { object, property } => {
                        lhs = Expression::MemberAssignment {
                            object,
                            property,
                            value: Box::new(value),
                        };
                    }
                    Expression::Index { object, index } => {
                        lhs = Expression::IndexAssignment { object, index, value: Box::new(value) };
                    }
                    p @ Expression::Array(_) | p @ Expression::Object(_)
                        if opt_bin_op.is_none() =>
                    {
                        self.validate_assignment_pattern(&p)?;
                        lhs = Expression::DestructuringAssignment {
                            pattern: Box::new(p),
                            value: Box::new(value),
                        };
                    }
                    _ => {
                        return Err(ParseError::InvalidAssignmentTarget(
                            self.current.line,
                            self.current.col,
                        ));
                    }
                }
                continue;
            }

            if min_bp == 0 && self.current.kind == TokenKind::Comma {
                self.advance()?;
                let next_expr = self.parse_expression(1)?;
                lhs = match lhs {
                    Expression::Sequence(mut exprs) => {
                        exprs.push(next_expr);
                        Expression::Sequence(exprs)
                    }
                    single => Expression::Sequence(vec![single, next_expr]),
                };
                continue;
            }

            let (op, l_bp, r_bp) = match self.current.kind {
                TokenKind::QuestionQuestion => (BinaryOp::NullishCoalescing, 2, 3),
                TokenKind::PipePipe => (BinaryOp::LogicalOr, 2, 3),
                TokenKind::AmpAmp => (BinaryOp::LogicalAnd, 4, 5),
                TokenKind::Pipe => (BinaryOp::BitwiseOr, 6, 7),
                TokenKind::Caret => (BinaryOp::BitwiseXor, 8, 9),
                TokenKind::Amp => (BinaryOp::BitwiseAnd, 10, 11),
                TokenKind::EqEq => (BinaryOp::Eq, 12, 13),
                TokenKind::NotEq => (BinaryOp::NotEq, 12, 13),
                TokenKind::EqEqEq => (BinaryOp::StrictEq, 12, 13),
                TokenKind::NotEqEq => (BinaryOp::StrictNotEq, 12, 13),
                TokenKind::Lt => (BinaryOp::Lt, 14, 15),
                TokenKind::LtEq => (BinaryOp::Lte, 14, 15),
                TokenKind::Gt => (BinaryOp::Gt, 14, 15),
                TokenKind::GtEq => (BinaryOp::Gte, 14, 15),
                TokenKind::Instanceof => (BinaryOp::Instanceof, 14, 15),
                TokenKind::In if self.allow_in => (BinaryOp::In, 14, 15),
                TokenKind::LtLt => (BinaryOp::Shl, 15, 16),
                TokenKind::GtGt => (BinaryOp::Shr, 15, 16),
                TokenKind::GtGtGt => (BinaryOp::Ushr, 15, 16),
                TokenKind::Plus => (BinaryOp::Add, 16, 17),
                TokenKind::Minus => (BinaryOp::Sub, 16, 17),
                TokenKind::Star => (BinaryOp::Mul, 18, 19),
                TokenKind::Slash => (BinaryOp::Div, 18, 19),
                TokenKind::Percent => (BinaryOp::Mod, 18, 19),
                TokenKind::StarStar => (BinaryOp::Exp, 21, 20),
                _ => break,
            };

            if l_bp < min_bp {
                break;
            }

            if op == BinaryOp::Exp && matches!(lhs, Expression::Unary { .. }) {
                return Err(ParseError::UnexpectedToken(
                    TokenKind::StarStar,
                    self.current.line,
                    self.current.col,
                ));
            }

            // ES Spec: cannot mix ?? with && or || without parentheses
            if op == BinaryOp::NullishCoalescing {
                if matches!(
                    lhs,
                    Expression::Binary { op: BinaryOp::LogicalAnd | BinaryOp::LogicalOr, .. }
                ) {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::QuestionQuestion,
                        self.current.line,
                        self.current.col,
                    ));
                }
            } else if op == BinaryOp::LogicalAnd || op == BinaryOp::LogicalOr {
                if matches!(lhs, Expression::Binary { op: BinaryOp::NullishCoalescing, .. }) {
                    let tok = if op == BinaryOp::LogicalAnd {
                        TokenKind::AmpAmp
                    } else {
                        TokenKind::PipePipe
                    };
                    return Err(ParseError::UnexpectedToken(
                        tok,
                        self.current.line,
                        self.current.col,
                    ));
                }
            }

            self.advance()?;
            let rhs = self.parse_expression(r_bp)?;

            if op == BinaryOp::NullishCoalescing {
                if matches!(
                    rhs,
                    Expression::Binary { op: BinaryOp::LogicalAnd | BinaryOp::LogicalOr, .. }
                ) {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::QuestionQuestion,
                        self.current.line,
                        self.current.col,
                    ));
                }
            } else if op == BinaryOp::LogicalAnd || op == BinaryOp::LogicalOr {
                if matches!(rhs, Expression::Binary { op: BinaryOp::NullishCoalescing, .. }) {
                    let tok = if op == BinaryOp::LogicalAnd {
                        TokenKind::AmpAmp
                    } else {
                        TokenKind::PipePipe
                    };
                    return Err(ParseError::UnexpectedToken(
                        tok,
                        self.current.line,
                        self.current.col,
                    ));
                }
            }

            if matches!(rhs, Expression::PrivateIdentifier(_)) {
                return Err(ParseError::UnexpectedToken(
                    TokenKind::In,
                    self.current.line,
                    self.current.col,
                ));
            }
            if matches!(lhs, Expression::PrivateIdentifier(_))
                && matches!(rhs, Expression::ArrowExpr { .. })
            {
                return Err(ParseError::UnexpectedToken(
                    TokenKind::FatArrow,
                    self.current.line,
                    self.current.col,
                ));
            }
            lhs = Expression::Binary { op, left: Box::new(lhs), right: Box::new(rhs) };
        }

        if matches!(lhs, Expression::PrivateIdentifier(_)) {
            return Err(ParseError::UnexpectedToken(
                self.current.kind.clone(),
                self.current.line,
                self.current.col,
            ));
        }

        Ok(lhs)
    }

    pub(crate) fn parse_call_args(&mut self) -> Result<Vec<Expression>, ParseError> {
        let mut args = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let prev_spread = self.allow_spread;
                self.allow_spread = true;
                let arg = self.parse_expression(1)?;
                self.allow_spread = prev_spread;
                args.push(arg);
                if !self.match_token(&TokenKind::Comma)? {
                    break;
                }
                if self.check(&TokenKind::RParen) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok(args)
    }
}
