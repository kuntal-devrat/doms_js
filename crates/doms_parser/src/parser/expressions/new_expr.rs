use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_new_expr(
        &mut self,
        line: usize,
        col: usize,
    ) -> Result<Expression, ParseError> {
        if self.match_token(&TokenKind::Dot)? {
            let ident = self.expect_identifier()?;
            if ident == "target" {
                if !self.in_non_arrow_function && !self.in_static_block {
                    return Err(ParseError::UnexpectedToken(TokenKind::Ident(ident), line, col));
                }
                return Ok(Expression::NewTarget);
            } else {
                return Err(ParseError::UnexpectedToken(TokenKind::Ident(ident), line, col));
            }
        }
        let mut callee = self.parse_prefix()?;
        loop {
            if self.check(&TokenKind::Dot) {
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
                    prop = match self.resolve_private_name(&prop) {
                        Some(scoped) => scoped,
                        None => {
                            return Err(ParseError::UnexpectedToken(
                                prop_tok.kind,
                                prop_tok.line,
                                prop_tok.col,
                            ));
                        }
                    };
                }
                callee = Expression::Member { object: Box::new(callee), property: prop };
            } else if self.check(&TokenKind::LBracket) {
                self.advance()?;
                let index = self.parse_expression(0)?;
                self.expect(TokenKind::RBracket)?;
                callee = Expression::Index { object: Box::new(callee), index: Box::new(index) };
            } else if let TokenKind::TemplateLiteral { quasis, raw_quasis, raw_exprs } = &self.current.kind {
                let quasis = quasis.clone();
                let raw_quasis = raw_quasis.clone();
                let raw_exprs = raw_exprs.clone();
                self.advance()?;
                let mut exprs = Vec::new();
                for raw in &raw_exprs {
                    let sub = Parser::new(raw)?.parse_expression(0)?;
                    exprs.push(sub);
                }
                callee = Expression::TaggedTemplate {
                    tag: Box::new(callee),
                    quasis,
                    raw_quasis,
                    exprs,
                };
            } else {
                break;
            }
        }
        let mut args = Vec::new();
        if self.match_token(&TokenKind::LParen)? {
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
        }
        Ok(Expression::New { callee: Box::new(callee), args })
    }
}
