use super::{ParseError, Parser};
use crate::ast::*;
use crate::lexer::TokenKind;

impl<'a> Parser<'a> {
    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut body = Vec::new();
        let mut in_directive_prologue = true;
        while self.current.kind != TokenKind::Eof {
            self.is_top_level = true;
            let stmt = self.parse_statement()?;
            if in_directive_prologue {
                if let Statement::Expression(Expression::Literal(Literal::String { ref value, has_escape, has_legacy_escape })) = stmt {
                    if has_legacy_escape && self.is_strict {
                        return Err(ParseError::UnexpectedToken(TokenKind::StringLit { value: value.clone(), has_escape, has_legacy_escape, has_unpaired_surrogate: false }, self.current.line, self.current.col));
                    }
                    if value == "use strict" && !has_escape {
                        self.is_strict = true;
                        self.lexer.is_strict = true;
                        if body.iter().any(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { has_legacy_escape: true, .. })))) {
                            return Err(ParseError::UnexpectedToken(TokenKind::StringLit { value: value.clone(), has_escape, has_legacy_escape, has_unpaired_surrogate: false }, self.current.line, self.current.col));
                        }
                    }
                } else {
                    in_directive_prologue = false;
                }
            }
            body.push(stmt);
        }
        if self.is_module {
            self.validate_module_early_errors(&body)?;
        }
        Ok(Program { body })
    }
}
