use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_try_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // try
        let try_block = Box::new(self.parse_block_statement()?);
        let mut catch_param = None;
        let mut catch_block = None;
        if self.match_token(&TokenKind::Catch)? {
            let mut pattern_decls = Vec::new();
            if self.match_token(&TokenKind::LParen)? {
                if self.match_token(&TokenKind::LBrace)? {
                    let tmp = format!("__catch_err_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    catch_param = Some(tmp.clone());
                    self.parse_object_destructuring_into(&tmp, &mut pattern_decls)?;
                } else if self.match_token(&TokenKind::LBracket)? {
                    let tmp = format!("__catch_err_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    catch_param = Some(tmp.clone());
                    self.parse_array_destructuring_into(&tmp, &mut pattern_decls)?;
                } else {
                    let p_tok = self.advance()?;
                    match p_tok.kind {
                        TokenKind::Ident(id) => catch_param = Some(id),
                        other => return Err(ParseError::UnexpectedToken(other, p_tok.line, p_tok.col)),
                    }
                }
                self.expect(TokenKind::RParen)?;
            }
            let mut block = self.parse_block_statement()?;
            if !pattern_decls.is_empty() {
                if let Statement::Block(ref mut stmts) = block {
                    stmts.insert(0, Statement::VarDecl { kind: VarKind::Let, declarations: pattern_decls });
                }
            }
            catch_block = Some(Box::new(block));
        }
        let mut finally_block = None;
        if self.match_token(&TokenKind::Finally)? {
            finally_block = Some(Box::new(self.parse_block_statement()?));
        }
        Ok(Statement::TryCatch { try_block, catch_param, catch_block, finally_block })
    }
}
