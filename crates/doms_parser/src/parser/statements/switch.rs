use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_switch_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // switch
        self.expect(TokenKind::LParen)?;
        let discriminant = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::LBrace)?;

        let mut cases = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            if self.match_token(&TokenKind::Case)? {
                let test = self.parse_expression(0)?;
                self.expect(TokenKind::Colon)?;
                let mut consequent = Vec::new();
                while !self.check(&TokenKind::Case)
                    && !self.check(&TokenKind::Default)
                    && !self.check(&TokenKind::RBrace)
                    && self.current.kind != TokenKind::Eof
                {
                    consequent.push(self.parse_statement()?);
                }
                cases.push(crate::ast::SwitchCase {
                    test: Some(test),
                    consequent,
                });
            } else if self.match_token(&TokenKind::Default)? {
                self.expect(TokenKind::Colon)?;
                let mut consequent = Vec::new();
                while !self.check(&TokenKind::Case)
                    && !self.check(&TokenKind::Default)
                    && !self.check(&TokenKind::RBrace)
                    && self.current.kind != TokenKind::Eof
                {
                    consequent.push(self.parse_statement()?);
                }
                cases.push(crate::ast::SwitchCase {
                    test: None,
                    consequent,
                });
            } else {
                let tok = self.advance()?;
                return Err(ParseError::UnexpectedToken(tok.kind, tok.line, tok.col));
            }
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Statement::Switch { discriminant, cases })
    }
}
