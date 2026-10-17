use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
        let ret_tok = self.advance()?; // return
        if !self.in_function {
            return Err(ParseError::UnexpectedToken(TokenKind::Return, ret_tok.line, ret_tok.col));
        }
        if self.check(&TokenKind::Semicolon)
            || self.check(&TokenKind::RBrace)
            || self.check(&TokenKind::Eof)
            || self.current.has_preceding_line_terminator
        {
            self.match_token(&TokenKind::Semicolon)?;
            Ok(Statement::Return(None))
        } else {
            let expr = self.parse_expression(0)?;
            self.consume_semicolon()?;
            Ok(Statement::Return(Some(expr)))
        }
    }

    pub(crate) fn parse_block_statement(&mut self) -> Result<Statement, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let prev_single = self.in_single_statement_context;
        self.in_single_statement_context = false;
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            stmts.push(self.parse_statement()?);
        }
        self.expect(TokenKind::RBrace)?;
        self.in_single_statement_context = prev_single;
        self.validate_block_declarations(&stmts)?;
        Ok(Statement::Block(stmts))
    }
}
