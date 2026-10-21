use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_prefix_increment(
        &mut self,
        line: usize,
        col: usize,
    ) -> Result<Expression, ParseError> {
        let expr = self.parse_expression(22)?;
        let target = unwrap_grouping(&expr);
        if !matches!(
            target,
            Expression::Identifier(_) | Expression::Member { .. } | Expression::Index { .. }
        ) {
            return Err(ParseError::UnexpectedToken(TokenKind::PlusPlus, line, col));
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
        Ok(Expression::Update { op: UpdateOp::Inc, is_prefix: true, expr: Box::new(expr) })
    }

    pub(super) fn parse_prefix_decrement(
        &mut self,
        line: usize,
        col: usize,
    ) -> Result<Expression, ParseError> {
        let expr = self.parse_expression(22)?;
        let target = unwrap_grouping(&expr);
        if !matches!(
            target,
            Expression::Identifier(_) | Expression::Member { .. } | Expression::Index { .. }
        ) {
            return Err(ParseError::UnexpectedToken(TokenKind::MinusMinus, line, col));
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
        Ok(Expression::Update { op: UpdateOp::Dec, is_prefix: true, expr: Box::new(expr) })
    }
}
