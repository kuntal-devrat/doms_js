use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_var_decl(&mut self, kind: VarKind) -> Result<Statement, ParseError> {
        self.advance()?; // let / const / var
        let mut declarations = Vec::new();

        loop {
            if self.match_token(&TokenKind::LBracket)? {
                let tmp_name = format!("@destruct_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let mut destruct_decls = Vec::new();
                self.parse_array_destructuring_into(&tmp_name, &mut destruct_decls)?;
                let init = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                declarations.push(VarDeclarator { name: tmp_name, init });
                declarations.extend(destruct_decls);
            } else if self.match_token(&TokenKind::LBrace)? {
                let tmp_name = format!("@destruct_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let mut destruct_decls = Vec::new();
                self.parse_object_destructuring_into(&tmp_name, &mut destruct_decls)?;
                let init = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                declarations.push(VarDeclarator { name: tmp_name, init });
                declarations.extend(destruct_decls);
            } else {
                let name = self.expect_identifier()?;
                let init = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                declarations.push(VarDeclarator { name, init });
            }

            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }

        self.consume_semicolon()?;
        Ok(Statement::VarDecl { kind, declarations })
    }
}
