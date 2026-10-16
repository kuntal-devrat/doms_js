use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn expect_export_name(&mut self) -> Result<String, ParseError> {
        let tok = self.advance()?;
        match tok.kind {
            TokenKind::Ident(s) => Ok(s),
            TokenKind::StringLit { value, has_unpaired_surrogate, .. } => {
                if has_unpaired_surrogate {
                    return Err(ParseError::General("Export name contains unpaired surrogate".to_string()));
                }
                Ok(value)
            }
            TokenKind::Default => Ok("default".to_string()),
            TokenKind::As => Ok("as".to_string()),
            TokenKind::From => Ok("from".to_string()),
            TokenKind::Let => Ok("let".to_string()),
            TokenKind::Const => Ok("const".to_string()),
            TokenKind::Var => Ok("var".to_string()),
            TokenKind::Function => Ok("function".to_string()),
            TokenKind::Class => Ok("class".to_string()),
            TokenKind::If => Ok("if".to_string()),
            TokenKind::Else => Ok("else".to_string()),
            TokenKind::While => Ok("while".to_string()),
            TokenKind::For => Ok("for".to_string()),
            TokenKind::Do => Ok("do".to_string()),
            TokenKind::Break => Ok("break".to_string()),
            TokenKind::Continue => Ok("continue".to_string()),
            TokenKind::Return => Ok("return".to_string()),
            TokenKind::Switch => Ok("switch".to_string()),
            TokenKind::Case => Ok("case".to_string()),
            TokenKind::Throw => Ok("throw".to_string()),
            TokenKind::Try => Ok("try".to_string()),
            TokenKind::Catch => Ok("catch".to_string()),
            TokenKind::Finally => Ok("finally".to_string()),
            TokenKind::New => Ok("new".to_string()),
            TokenKind::This => Ok("this".to_string()),
            TokenKind::Super => Ok("super".to_string()),
            TokenKind::Null => Ok("null".to_string()),
            TokenKind::True => Ok("true".to_string()),
            TokenKind::False => Ok("false".to_string()),
            TokenKind::In => Ok("in".to_string()),
            TokenKind::Instanceof => Ok("instanceof".to_string()),
            TokenKind::Typeof => Ok("typeof".to_string()),
            TokenKind::Void => Ok("void".to_string()),
            TokenKind::Delete => Ok("delete".to_string()),
            TokenKind::With => Ok("with".to_string()),
            TokenKind::Yield => Ok("yield".to_string()),
            TokenKind::Await => Ok("await".to_string()),
            TokenKind::Import => Ok("import".to_string()),
            TokenKind::Export => Ok("export".to_string()),
            TokenKind::Debugger => Ok("debugger".to_string()),
            other => Err(ParseError::UnexpectedToken(other, tok.line, tok.col)),
        }
    }
}
