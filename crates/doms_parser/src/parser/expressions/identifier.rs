use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_identifier_expr(&mut self, s: String, line: usize, col: usize) -> Result<Expression, ParseError> {
                if self.is_strict && is_strict_reserved_word(&s) {
                    return Err(ParseError::UnexpectedToken(TokenKind::Ident(s), line, col));
                }
                if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
                    let prev_in_func = self.in_function;
                    self.in_function = true;
                    let body = if self.check(&TokenKind::LBrace) {
                        match self.parse_statement()? {
                            Statement::Block(stmts) => stmts,
                            other => vec![other],
                        }
                    } else {
                        let expr = self.parse_expression(1)?;
                        vec![Statement::Return(Some(expr))]
                    };
                    self.in_function = prev_in_func;
                    Ok(Expression::ArrowExpr {
                        params: vec![s],
                        body,
                    })
                } else {
                    Ok(Expression::Identifier(s))
                }
    }
}
