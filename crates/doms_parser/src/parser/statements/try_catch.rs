use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_try_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // try
        let try_block = Box::new(self.parse_block_statement()?);
        let mut catch_param = None;
        let mut catch_block = None;
        if self.match_token(&TokenKind::Catch)? {
            let mut pattern_decls = Vec::new();
            let mut catch_bound_names = Vec::new();
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
                    let id = self.expect_identifier()?;
                    catch_param = Some(id.clone());
                    catch_bound_names.push(id);
                }
                self.expect(TokenKind::RParen)?;
            }
            for d in &pattern_decls {
                if !d.name.starts_with('@') && !d.name.starts_with("__") {
                    catch_bound_names.push(d.name.clone());
                }
            }
            let mut seen_catch = std::collections::HashSet::new();
            for name in &catch_bound_names {
                if !seen_catch.insert(name) {
                    return Err(ParseError::General(format!(
                        "Duplicate catch parameter '{}'",
                        name
                    )));
                }
            }
            let mut block = self.parse_block_statement()?;
            if let Statement::Block(ref stmts) = block {
                for stmt in stmts {
                    match stmt {
                        Statement::VarDecl { kind, declarations } if *kind != VarKind::Var => {
                            for decl in declarations {
                                if !decl.name.starts_with("__") && !decl.name.starts_with('@') {
                                    if seen_catch.contains(&decl.name) {
                                        return Err(ParseError::General(format!(
                                            "Identifier '{}' has already been declared",
                                            decl.name
                                        )));
                                    }
                                }
                            }
                        }
                        Statement::ClassDecl(class_def) => {
                            if let Some(ref name) = class_def.name {
                                if seen_catch.contains(name) {
                                    return Err(ParseError::General(format!(
                                        "Identifier '{}' has already been declared",
                                        name
                                    )));
                                }
                            }
                        }
                        Statement::FunctionDecl { name, .. }
                        | Statement::AsyncFunctionDecl { name, .. }
                        | Statement::GeneratorDecl { name, .. }
                        | Statement::AsyncGeneratorDecl { name, .. } => {
                            if seen_catch.contains(name) {
                                return Err(ParseError::General(format!(
                                    "Identifier '{}' has already been declared",
                                    name
                                )));
                            }
                        }
                        _ => {}
                    }
                }
            }
            if !pattern_decls.is_empty() {
                block = Statement::Block(vec![
                    Statement::VarDecl { kind: VarKind::Let, declarations: pattern_decls },
                    block,
                ]);
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
