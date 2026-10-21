use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_identifier_expr(
        &mut self,
        s: String,
        line: usize,
        col: usize,
    ) -> Result<Expression, ParseError> {
        if self.is_strict && is_strict_reserved_word(&s) {
            return Err(ParseError::UnexpectedToken(TokenKind::Ident(s), line, col));
        }
        if (self.in_async || self.is_module || self.in_static_block) && s == "await" {
            return Err(ParseError::UnexpectedToken(TokenKind::Await, line, col));
        }
        if (self.in_generator || self.is_strict) && s == "yield" {
            return Err(ParseError::UnexpectedToken(TokenKind::Yield, line, col));
        }
        if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
            if self.is_strict && (is_strict_reserved_word(&s) || s == "eval" || s == "arguments") {
                return Err(ParseError::UnexpectedToken(TokenKind::Ident(s), line, col));
            }
            let prev_in_func = self.in_function;
            let prev_in_static = self.in_static_block;
            self.in_function = true;
            self.in_static_block = false;
            let body = if self.check(&TokenKind::LBrace) {
                self.parse_function_body(&[s.clone()], None)?
            } else {
                let expr = self.parse_expression(1)?;
                vec![Statement::Return(Some(expr))]
            };
            self.in_function = prev_in_func;
            self.in_static_block = prev_in_static;
            Ok(Expression::ArrowExpr { params: vec![s], body })
        } else {
            Ok(Expression::Identifier(s))
        }
    }
}
