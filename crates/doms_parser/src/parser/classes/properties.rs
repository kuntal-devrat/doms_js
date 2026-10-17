use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_property_name_meta(&mut self) -> Result<(PropertyKey, bool), ParseError> {
        if self.match_token(&TokenKind::LBracket)? {
            let expr = self.parse_expression(0)?;
            self.expect(TokenKind::RBracket)?;
            Ok((PropertyKey::Computed(expr), false))
        } else {
            let key_tok = self.advance()?;
            let is_ident = matches!(&key_tok.kind, TokenKind::Ident(_))
                || (!self.is_module && !self.in_async && !self.in_static_block && key_tok.kind == TokenKind::Await);
            let key = match &key_tok.kind {
                TokenKind::StringLit { value, .. } => value.clone(),
                TokenKind::Number(n) => n.to_string(),
                TokenKind::BigInt(b) => b.to_string(),
                TokenKind::PrivateIdent(id) => format!("#{}", id),
                other => match other.as_prop_name() {
                    Some(id) => id,
                    None => return Err(ParseError::UnexpectedToken(other.clone(), key_tok.line, key_tok.col)),
                },
            };
            Ok((PropertyKey::Literal(key), is_ident))
        }
    }

    pub(crate) fn parse_property_name(&mut self) -> Result<PropertyKey, ParseError> {
        self.parse_property_name_meta().map(|(k, _)| k)
    }
}
