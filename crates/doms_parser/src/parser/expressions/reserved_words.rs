use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_reserved_word_expr(&mut self, kind: TokenKind, line: usize, col: usize) -> Result<Expression, ParseError> {
                if self.is_strict {
                    return Err(ParseError::UnexpectedToken(kind, line, col));
                }
                let s = kind.as_prop_name().unwrap_or_default();
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
