use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_prefix(&mut self) -> Result<Expression, ParseError> {
        let tok = self.advance()?;
        match tok.kind {
            TokenKind::Number(n) => Ok(Expression::Literal(Literal::Number(n))),
            TokenKind::BigInt(b) => Ok(Expression::Literal(Literal::BigInt(b))),
            TokenKind::StringLit { value, has_escape, has_legacy_escape, .. } => {
                Ok(Expression::Literal(Literal::String { value, has_escape, has_legacy_escape }))
            }
            TokenKind::TemplateLiteral { quasis, raw_exprs } => {
                self.parse_template_expression(quasis, raw_exprs)
            }
            TokenKind::RegExp { pattern, flags } => {
                Ok(Expression::Literal(Literal::RegExp { pattern, flags }))
            }
            TokenKind::True => Ok(Expression::Literal(Literal::Boolean(true))),
            TokenKind::False => Ok(Expression::Literal(Literal::Boolean(false))),
            TokenKind::Null => Ok(Expression::Literal(Literal::Null)),
            TokenKind::Undefined => Ok(Expression::Literal(Literal::Undefined)),
            TokenKind::Ident(s) => {
                if tok.has_escape && crate::lexer::is_reserved_word(&s) {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::Ident(s),
                        tok.line,
                        tok.col,
                    ));
                }
                self.parse_identifier_expr(s, tok.line, tok.col)
            }
            TokenKind::This => Ok(Expression::This),
            TokenKind::Super => {
                if self.check(&TokenKind::LParen) {
                    if !self.in_derived_constructor {
                        return Err(ParseError::UnexpectedToken(
                            TokenKind::Super,
                            tok.line,
                            tok.col,
                        ));
                    }
                } else if self.check(&TokenKind::Dot) || self.check(&TokenKind::LBracket) {
                    if !self.in_method && !self.in_derived_constructor {
                        return Err(ParseError::UnexpectedToken(
                            TokenKind::Super,
                            tok.line,
                            tok.col,
                        ));
                    }
                } else {
                    return Err(ParseError::UnexpectedToken(TokenKind::Super, tok.line, tok.col));
                }
                Ok(Expression::Super)
            }
            TokenKind::LBracket => self.parse_array_literal(),
            TokenKind::LBrace => self.parse_object_literal(),
            TokenKind::Function => self.parse_function_expr(),
            TokenKind::Class => self.parse_class_expr(),
            TokenKind::PlusPlus => self.parse_prefix_increment(tok.line, tok.col),
            TokenKind::MinusMinus => self.parse_prefix_decrement(tok.line, tok.col),
            TokenKind::Plus => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::Pos, expr: Box::new(expr) })
            }
            TokenKind::Minus => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::Neg, expr: Box::new(expr) })
            }
            TokenKind::Bang => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::Not, expr: Box::new(expr) })
            }
            TokenKind::Tilde => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::BitNot, expr: Box::new(expr) })
            }
            TokenKind::Typeof => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::Typeof, expr: Box::new(expr) })
            }
            TokenKind::Void => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::Void, expr: Box::new(expr) })
            }
            TokenKind::Delete => {
                let expr = self.parse_expression(22)?;
                if self.is_strict && matches!(&expr, Expression::Identifier(_)) {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::Delete,
                        self.current.line,
                        self.current.col,
                    ));
                }
                Ok(Expression::Unary { op: UnaryOp::Delete, expr: Box::new(expr) })
            }
            TokenKind::LParen => self.parse_paren_or_arrow(tok.line, tok.col),
            TokenKind::Async => self.parse_async_prefix_expr(),
            TokenKind::New => self.parse_new_expr(tok.line, tok.col),
            TokenKind::As => Ok(Expression::Identifier("as".to_string())),
            TokenKind::From => Ok(Expression::Identifier("from".to_string())),
            TokenKind::Implements
            | TokenKind::Interface
            | TokenKind::Package
            | TokenKind::Private
            | TokenKind::Protected
            | TokenKind::Public
            | TokenKind::Static
            | TokenKind::Let => self.parse_reserved_word_expr(tok.kind.clone(), tok.line, tok.col),
            TokenKind::Await => {
                let can_await =
                    (self.in_async || (self.is_module && !self.in_function)) && !self.in_param;
                if can_await {
                    let expr = self.parse_expression(20)?;
                    Ok(Expression::Await(Box::new(expr)))
                } else if self.is_module || self.in_async || self.in_static_block {
                    Err(ParseError::UnexpectedToken(TokenKind::Await, tok.line, tok.col))
                } else {
                    Ok(Expression::Identifier("await".to_string()))
                }
            }
            TokenKind::Yield => {
                if !self.in_generator {
                    if self.is_strict {
                        return Err(ParseError::UnexpectedToken(
                            TokenKind::Yield,
                            tok.line,
                            tok.col,
                        ));
                    }
                    return Ok(Expression::Identifier("yield".to_string()));
                }
                if self.match_token(&TokenKind::Star)? {
                    let expr = self.parse_expression(0)?;
                    Ok(Expression::YieldStar(Box::new(expr)))
                } else if self.check(&TokenKind::Semicolon)
                    || self.check(&TokenKind::RParen)
                    || self.check(&TokenKind::RBracket)
                    || self.check(&TokenKind::RBrace)
                    || self.check(&TokenKind::Comma)
                    || self.check(&TokenKind::Colon)
                    || self.check(&TokenKind::Eof)
                {
                    Ok(Expression::Yield(None))
                } else {
                    let expr = self.parse_expression(0)?;
                    Ok(Expression::Yield(Some(Box::new(expr))))
                }
            }
            TokenKind::Import => {
                self.expect(TokenKind::LParen)?;
                let spec = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                Ok(Expression::DynamicImport(Box::new(spec)))
            }
            TokenKind::DotDotDot => {
                let expr = self.parse_expression(1)?;
                Ok(Expression::Spread(Box::new(expr)))
            }
            TokenKind::PrivateIdent(s) => {
                let prop = format!("#{}", s);
                let scoped = match self.resolve_private_name(&prop) {
                    Some(n) => n,
                    None => {
                        return Err(ParseError::UnexpectedToken(
                            TokenKind::PrivateIdent(s),
                            tok.line,
                            tok.col,
                        ));
                    }
                };
                if !self.allow_in || !self.check(&TokenKind::In) {
                    return Err(ParseError::UnexpectedToken(
                        self.current.kind.clone(),
                        self.current.line,
                        self.current.col,
                    ));
                }
                Ok(Expression::PrivateIdentifier(scoped))
            }
            other => Err(ParseError::UnexpectedToken(other, tok.line, tok.col)),
        }
    }
}
