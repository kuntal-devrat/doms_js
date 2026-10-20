use super::*;

impl<'a> Parser<'a> {
    pub(crate) fn validate_module_early_errors(
        &self,
        body: &[Statement],
    ) -> Result<(), ParseError> {
        let mut lex_names: Vec<String> = Vec::new();
        let mut var_names: Vec<String> = Vec::new();
        let mut exported_names: Vec<String> = Vec::new();
        let mut unresolvable_exports: Vec<String> = Vec::new();

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

        let add_export = |names: &mut Vec<String>, name: String| -> Result<(), ParseError> {
            if names.contains(&name) {
                return Err(ParseError::General(format!("Duplicate exported name '{}'", name)));
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
                Statement::FunctionDecl { name, .. }
                | Statement::AsyncFunctionDecl { name, .. }
                | Statement::GeneratorDecl { name, .. }
                | Statement::AsyncGeneratorDecl { name, .. } => {
                    if !name.is_empty() {
                        add_lex(&mut lex_names, name.clone())?;
                    }
                }
                Statement::ClassDecl(class_def) => {
                    if let Some(ref name) = class_def.name {
                        add_lex(&mut lex_names, name.clone())?;
                    }
                }
                Statement::Import { specifiers, .. } => {
                    for (name, alias) in specifiers {
                        let bound = alias.as_ref().unwrap_or(name);
                        add_lex(&mut lex_names, bound.clone())?;
                    }
                }
                Statement::Export { decl, is_default, specifiers, source, all_as, .. } => {
                    if *is_default {
                        add_export(&mut exported_names, "default".to_string())?;
                    }
                    if let Some(alias) = all_as {
                        add_export(&mut exported_names, alias.clone())?;
                    }
                    if source.is_some() {
                        for (name, alias) in specifiers {
                            let exp = alias.as_ref().unwrap_or(name);
                            add_export(&mut exported_names, exp.clone())?;
                        }
                    } else if !specifiers.is_empty() {
                        for (name, alias) in specifiers {
                            let exp = alias.as_ref().unwrap_or(name);
                            add_export(&mut exported_names, exp.clone())?;
                            unresolvable_exports.push(name.clone());
                        }
                    }
                    if let Some(inner) = decl {
                        match &**inner {
                            Statement::VarDecl { kind, declarations } => {
                                if *kind == VarKind::Var {
                                    for d in declarations {
                                        if !var_names.contains(&d.name) {
                                            var_names.push(d.name.clone());
                                        }
                                        add_export(&mut exported_names, d.name.clone())?;
                                    }
                                } else {
                                    for d in declarations {
                                        add_lex(&mut lex_names, d.name.clone())?;
                                        add_export(&mut exported_names, d.name.clone())?;
                                    }
                                }
                            }
                            Statement::FunctionDecl { name, .. }
                            | Statement::AsyncFunctionDecl { name, .. }
                            | Statement::GeneratorDecl { name, .. }
                            | Statement::AsyncGeneratorDecl { name, .. } => {
                                if !name.is_empty() {
                                    add_lex(&mut lex_names, name.clone())?;
                                    if !*is_default {
                                        add_export(&mut exported_names, name.clone())?;
                                    }
                                }
                            }
                            Statement::ClassDecl(class_def) => {
                                if let Some(ref name) = class_def.name {
                                    add_lex(&mut lex_names, name.clone())?;
                                    if !*is_default {
                                        add_export(&mut exported_names, name.clone())?;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {
                    Self::collect_var_declared_names(stmt, &mut var_names);
                }
            }
        }

        // Check 1: Disjoint LexicallyDeclaredNames and VarDeclaredNames
        for lex in &lex_names {
            if var_names.contains(lex) {
                return Err(ParseError::General(format!(
                    "Identifier '{}' has already been declared",
                    lex
                )));
            }
        }

        // Check 2: Unresolvable exports
        for exp_bind in unresolvable_exports {
            if !lex_names.contains(&exp_bind) && !var_names.contains(&exp_bind) {
                return Err(ParseError::General(format!(
                    "Exported identifier '{}' is not defined",
                    exp_bind
                )));
            }
        }

        // Check 3: Check labels and undefined break/continue targets
        Self::validate_labels_and_jumps(body)?;

        Ok(())
    }
}
