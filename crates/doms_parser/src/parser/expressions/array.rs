use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_array_literal(&mut self) -> Result<Expression, ParseError> {
        let prev_in = self.allow_in;
        let prev_spread = self.allow_spread;
        self.allow_in = true;
        self.allow_spread = true;
        let mut elements = Vec::new();
        while !self.check(&TokenKind::RBracket) && self.current.kind != TokenKind::Eof {
            if self.match_token(&TokenKind::Comma)? {
                elements.push(Expression::Literal(Literal::Hole));
                continue;
            }
            self.allow_spread = true;
            elements.push(self.parse_expression(1)?);
            let is_spread = matches!(elements.last(), Some(Expression::Spread(_)));
            if self.match_token(&TokenKind::Comma)? {
                if is_spread && self.check(&TokenKind::RBracket) {
                    self.trailing_comma_after_rest = true;
                }
                continue;
            } else {
                break;
            }
        }
        self.expect(TokenKind::RBracket)?;
        self.allow_in = prev_in;
        self.allow_spread = prev_spread;
        Ok(Expression::Array(elements))
    }
}
