use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_async_prefix_expr(&mut self) -> Result<Expression, ParseError> {
                if self.match_token(&TokenKind::Function)? {
                    let is_generator = self.match_token(&TokenKind::Star)?;
                    let name = if let TokenKind::Ident(id) = &self.current.kind {
                        if self.is_strict && (is_strict_reserved_word(id) || id == "eval" || id == "arguments") {
                            return Err(ParseError::UnexpectedToken(TokenKind::Ident(id.clone()), self.current.line, self.current.col));
                        }
                        let n = id.clone();
                        self.advance()?;
                        Some(n)
                    } else {
                        None
                    };

                    let prev_gen = self.in_generator;
                    let prev_async = self.in_async;
                    if is_generator {
                        self.in_generator = true;
                    }
                    self.in_async = true;
                    let (params, defaults) = self.parse_formal_parameters()?;
                    let mut body = self.parse_function_body(&params, name.as_deref())?;
                    self.in_generator = prev_gen;
                    self.in_async = prev_async;
                    Self::apply_parameter_defaults(&mut body, defaults);

                    if is_generator {
                        Ok(Expression::AsyncGeneratorExpr { name, params, body })
                    } else {
                        Ok(Expression::AsyncFunctionExpr { name, params, body })
                    }
                } else {
                    let expr = self.parse_expression(20)?;
                    Ok(expr)
                }
    }
}
