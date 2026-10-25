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
                if let Statement::Expression(Expression::Literal(Literal::String {
                    ref value,
                    has_escape,
                    has_legacy_escape,
                })) = stmt
                {
                    if has_legacy_escape && self.is_strict {
                        return Err(ParseError::UnexpectedToken(
                            TokenKind::StringLit {
                                value: value.clone(),
                                has_escape,
                                has_legacy_escape,
                                has_unpaired_surrogate: false,
                            },
                            self.current.line,
                            self.current.col,
                        ));
                    }
                    if value == "use strict" && !has_escape {
                        self.is_strict = true;
                        self.lexer.is_strict = true;
                        if body.iter().any(|s| {
                            matches!(
                                s,
                                Statement::Expression(Expression::Literal(Literal::String {
                                    has_legacy_escape: true,
                                    ..
                                }))
                            )
                        }) {
                            return Err(ParseError::UnexpectedToken(
                                TokenKind::StringLit {
                                    value: value.clone(),
                                    has_escape,
                                    has_legacy_escape,
                                    has_unpaired_surrogate: false,
                                },
                                self.current.line,
                                self.current.col,
                            ));
                        }
                    }
                } else {
                    in_directive_prologue = false;
                }
            }
            body.push(stmt);
        }
        Self::validate_labels_and_jumps(&body)?;
        if self.is_module {
            self.validate_module_early_errors(&body)?;
        } else {
            self.validate_script_early_errors(&body)?;
        }
        Ok(Program { body })
    }

    pub(crate) fn validate_script_early_errors(&self, body: &[Statement]) -> Result<(), ParseError> {
        let mut lex_names: Vec<String> = Vec::new();
        let mut var_names: Vec<String> = Vec::new();

        let add_lex = |names: &mut Vec<String>, name: String| -> Result<(), ParseError> {
            if names.contains(&name) {
                return Err(ParseError::General(format!(
                    "Identifier '{}' has already been declared",
                    name
                )));
            }
            names.push(name);
            Ok(())
        };

        for stmt in body {
            match stmt {
                Statement::VarDecl { kind, declarations } => {
                    if *kind == VarKind::Var {
                        for d in declarations {
                            if !var_names.contains(&d.name) {
                                var_names.push(d.name.clone());
                            }
                        }
                    } else {
                        for d in declarations {
                            add_lex(&mut lex_names, d.name.clone())?;
                        }
                    }
                }
                Statement::ClassDecl(class_def) => {
                    if let Some(ref name) = class_def.name {
                        add_lex(&mut lex_names, name.clone())?;
                    }
                }
                Statement::FunctionDecl { name, .. }
                | Statement::AsyncFunctionDecl { name, .. }
                | Statement::GeneratorDecl { name, .. }
                | Statement::AsyncGeneratorDecl { name, .. } => {
                    if !name.is_empty() && !var_names.contains(name) {
                        var_names.push(name.clone());
                    }
                }
                _ => {
                    Self::collect_var_declared_names(stmt, &mut var_names);
                }
            }
        }

        for lex in &lex_names {
            if var_names.contains(lex) {
                return Err(ParseError::General(format!(
                    "Identifier '{}' has already been declared",
                    lex
                )));
            }
        }

        Ok(())
    }
}
