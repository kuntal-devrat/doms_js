use super::*;

impl<'a> Parser<'a> {
    pub(super) fn parse_template_expression(
        &mut self,
        quasis: Vec<Option<String>>,
        raw_exprs: Vec<String>,
        line: usize,
        col: usize,
    ) -> Result<Expression, ParseError> {
        if quasis.iter().any(|q| q.is_none()) {
            return Err(ParseError::UnexpectedToken(
                TokenKind::TemplateLiteral {
                    quasis: Vec::new(),
                    raw_quasis: Vec::new(),
                    raw_exprs: Vec::new(),
                },
                line,
                col,
            ));
        }
        let quasis: Vec<String> = quasis.into_iter().map(|q| q.unwrap_or_default()).collect();
        if raw_exprs.is_empty() {
            let s = quasis.into_iter().next().unwrap_or_default();
            return Ok(Expression::Literal(Literal::String {
                value: s,
                has_escape: false,
                has_legacy_escape: false,
            }));
        }

        let mut expr: Option<Expression> = None;
        for (i, quasi) in quasis.into_iter().enumerate() {
            if !quasi.is_empty() || expr.is_none() {
                let lit = Expression::Literal(Literal::String {
                    value: quasi,
                    has_escape: false,
                    has_legacy_escape: false,
                });
                expr = match expr {
                    Some(prev) => Some(Expression::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(prev),
                        right: Box::new(lit),
                    }),
                    None => Some(lit),
                };
            }
            if i < raw_exprs.len() {
                let sub_expr = Parser::new(&raw_exprs[i])?.parse_expression(0)?;
                expr = match expr {
                    Some(prev) => Some(Expression::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(prev),
                        right: Box::new(sub_expr),
                    }),
                    None => Some(sub_expr),
                };
            }
        }
        Ok(expr.unwrap_or(Expression::Literal(Literal::String {
            value: String::new(),
            has_escape: false,
            has_legacy_escape: false,
        })))
    }
}
