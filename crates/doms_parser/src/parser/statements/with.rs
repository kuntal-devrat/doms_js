use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_with_statement(&mut self) -> Result<Statement, ParseError> {
        let tok = self.advance()?; // with
        if self.is_strict {
            return Err(ParseError::UnexpectedToken(TokenKind::With, tok.line, tok.col));
        }
        self.expect(TokenKind::LParen)?;
        let object = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;
        let body = Box::new(self.parse_statement()?);
        self.check_sub_statement(&body, false)?;
        Ok(Statement::With { object, body })
    }
}
