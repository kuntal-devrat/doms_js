use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_do_while_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // do
        let body = Box::new(self.parse_statement()?);
        self.check_sub_statement(&body, false)?;
        self.expect(TokenKind::While)?;
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;
        let _ = self.match_token(&TokenKind::Semicolon)?;
        Ok(Statement::DoWhile { body, condition })
    }
}
