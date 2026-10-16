use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_array_literal(&mut self) -> Result<Expression, ParseError> {
        let mut elements = Vec::new();
        while !self.check(&TokenKind::RBracket) && self.current.kind != TokenKind::Eof {
            if self.match_token(&TokenKind::Comma)? {
                elements.push(Expression::Literal(Literal::Hole));
                continue;
            }
            elements.push(self.parse_expression(1)?);
            if self.match_token(&TokenKind::Comma)? {
                continue;
            } else {
                break;
            }
        }
        self.expect(TokenKind::RBracket)?;
        Ok(Expression::Array(elements))
    }
}
