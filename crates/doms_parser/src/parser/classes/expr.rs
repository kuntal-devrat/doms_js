use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_class_expr(&mut self) -> Result<Expression, ParseError> {
        let prev_strict = self.is_strict;
        let prev_lexer_strict = self.lexer.is_strict;
        self.is_strict = true;
        self.lexer.is_strict = true;
        let res = self.parse_class_expr_inner();
        self.is_strict = prev_strict;
        self.lexer.is_strict = prev_lexer_strict;
        res
    }

    fn parse_class_expr_inner(&mut self) -> Result<Expression, ParseError> {
        let name = if self.check(&TokenKind::Extends) || self.check(&TokenKind::LBrace) {
            None
        } else {
            Some(self.parse_class_binding_identifier()?)
        };
        let super_class = if self.match_token(&TokenKind::Extends)? {
            let expr = self.parse_expression(1)?;
            if matches!(&expr, Expression::ArrowExpr { .. }) {
                return Err(ParseError::UnexpectedToken(
                    TokenKind::FatArrow,
                    self.current.line,
                    self.current.col,
                ));
            }
            Some(Box::new(expr))
        } else {
            None
        };
        let (ctor_params, ctor_body, has_ctor, elements) =
            self.parse_class_elements(super_class.is_some())?;
        Ok(Expression::ClassExpr(Box::new(ClassDefinition {
            name,
            super_class,
            ctor_params,
            ctor_body,
            has_constructor: has_ctor,
            elements,
        })))
    }
}
