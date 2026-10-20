use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_switch_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // switch
        self.expect(TokenKind::LParen)?;
        let discriminant = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::LBrace)?;

        let mut cases = Vec::new();
        let mut has_default = false;
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
                if has_default {
                    return Err(ParseError::General("More than one default clause in switch statement".into()));
                }
                has_default = true;
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

        let mut all_stmts = Vec::new();
        for case in &cases {
            all_stmts.extend(case.consequent.clone());
        }
        self.validate_block_declarations(&all_stmts)?;

        Ok(Statement::Switch { discriminant, cases })
    }
}
