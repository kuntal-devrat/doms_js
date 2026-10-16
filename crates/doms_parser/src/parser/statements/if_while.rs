use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_if_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // if
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;

        let then_branch = Box::new(self.parse_statement()?);
        self.check_sub_statement(&then_branch, true)?;
        let else_branch = if self.match_token(&TokenKind::Else)? {
            let eb = Box::new(self.parse_statement()?);
            self.check_sub_statement(&eb, true)?;
            Some(eb)
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

        let body = Box::new(self.parse_statement()?);
        self.check_sub_statement(&body, false)?;
        Ok(Statement::While { condition, body })
    }
}
