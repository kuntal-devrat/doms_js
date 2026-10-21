use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_property_name_meta(&mut self) -> Result<(PropertyKey, bool), ParseError> {
        if self.match_token(&TokenKind::LBracket)? {
            let prev_in = self.allow_in;
            self.allow_in = true;
            let expr = self.parse_expression(1)?;
            self.allow_in = prev_in;
            self.expect(TokenKind::RBracket)?;
            Ok((PropertyKey::Computed(expr), false))
        } else {
            let key_tok = self.advance()?;
            let is_ident = match &key_tok.kind {
                TokenKind::Ident(id) => {
                    !(key_tok.has_escape
                        && (crate::lexer::is_reserved_word(id)
                            || is_strict_reserved_word(id)
                            || ((self.in_async || self.is_module || self.in_static_block)
                                && id == "await")
                            || ((self.in_generator || self.is_strict) && id == "yield")))
                        && !(self.is_strict && is_strict_reserved_word(id))
                        && !(id == "await"
                            && (self.in_async || self.is_module || self.in_static_block))
                        && !(id == "yield" && (self.in_generator || self.is_strict))
                }
                TokenKind::Undefined | TokenKind::As | TokenKind::From | TokenKind::Async => true,
                TokenKind::Await => !self.is_module && !self.in_async && !self.in_static_block,
                TokenKind::Yield => !self.is_strict && !self.in_generator,
                TokenKind::Let => !self.is_strict,
                TokenKind::Implements
                | TokenKind::Interface
                | TokenKind::Package
                | TokenKind::Private
                | TokenKind::Protected
                | TokenKind::Public
                | TokenKind::Static => !self.is_strict,
                _ => false,
            };
            let key = match &key_tok.kind {
                TokenKind::StringLit { value, .. } => value.clone(),
                TokenKind::Number(n) => format_js_number(*n),
                TokenKind::BigInt(b) => b.to_string(),
                TokenKind::PrivateIdent(id) => format!("#{}", id),
                other => match other.as_prop_name() {
                    Some(id) => id,
                    None => {
                        return Err(ParseError::UnexpectedToken(
                            other.clone(),
                            key_tok.line,
                            key_tok.col,
                        ));
                    }
                },
            };
            Ok((PropertyKey::Literal(key), is_ident))
        }
    }

    pub(crate) fn parse_property_name(&mut self) -> Result<PropertyKey, ParseError> {
        self.parse_property_name_meta().map(|(k, _)| k)
    }
}

pub(crate) fn format_js_number(d: f64) -> String {
    if d.is_nan() {
        "NaN".to_string()
    } else if d == 0.0 {
        "0".to_string()
    } else if d.is_infinite() {
        if d > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }
    } else {
        let abs_d = d.abs();
        if abs_d >= 1e21 || abs_d < 1e-6 {
            let s = format!("{:e}", d);
            if let Some(pos) = s.find('e') {
                let after = &s[pos + 1..];
                if !after.starts_with('-') && !after.starts_with('+') {
                    format!("{}e+{}", &s[..pos], after)
                } else {
                    s
                }
            } else {
                s
            }
        } else if d.fract() == 0.0 && abs_d < 1e16 {
            format!("{}", d as i64)
        } else {
            format!("{}", d)
        }
    }
}
