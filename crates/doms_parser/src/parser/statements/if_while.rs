use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_if_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // if
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;

        let then_branch = Box::new(self.parse_sub_statement(true)?);
        let else_branch = if self.match_token(&TokenKind::Else)? {
            Some(Box::new(self.parse_sub_statement(true)?))
        } else {
            None
        };

        Ok(Statement::If { condition, then_branch, else_branch })
    }

    pub(crate) fn parse_while_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // while
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;

        let body = Box::new(self.parse_sub_statement(false)?);
        Ok(Statement::While { condition, body })
    }
}
