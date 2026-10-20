use super::*;

fn resolve_module_export(
    path: &Path,
    name: &str,
    visited: &mut std::collections::HashSet<(std::path::PathBuf, String)>,
) -> Result<Option<(std::path::PathBuf, String)>, String> {
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if !visited.insert((canonical.clone(), name.to_string())) {
        return Ok(None);
    }

    let content = fs::read_to_string(&canonical).map_err(|e| e.to_string())?;
    let prog = doms_parser::parse_module(&content).map_err(|e| e.to_string())?;

    let mut found = false;
    let mut star_sources = Vec::new();

    for stmt in &prog.body {
        match stmt {
            doms_parser::ast::Statement::Export {
                is_default,
                decl,
                specifiers,
                source,
                is_all,
                all_as,
            } => {
                if *is_default && name == "default" {
                    found = true;
                }
                if let Some(inner) = decl {
                    match &**inner {
                        doms_parser::ast::Statement::VarDecl { declarations, .. } => {
                            for d in declarations {
                                if d.name == name {
                                    found = true;
                                }
                            }
                        }
                        doms_parser::ast::Statement::ClassDecl(c) => {
                            if c.name.as_deref() == Some(name) {
                                found = true;
                            }
                        }
                        doms_parser::ast::Statement::FunctionDecl { name: fn_name, .. }
                        | doms_parser::ast::Statement::AsyncFunctionDecl {
                            name: fn_name, ..
                        }
                        | doms_parser::ast::Statement::GeneratorDecl { name: fn_name, .. }
                        | doms_parser::ast::Statement::AsyncGeneratorDecl {
                            name: fn_name, ..
                        } => {
                            if fn_name == name {
                                found = true;
                            }
                        }
                        _ => {}
                    }
                }
                if source.is_none() {
                    for (orig, alias) in specifiers {
                        let exp = alias.as_deref().unwrap_or(orig);
                        if exp == name {
                            let mut imported_from = None;
                            for s in &prog.body {
                                if let doms_parser::ast::Statement::Import {
                                    specifiers: imp_specs,
                                    source: imp_src,
                                    is_all,
                                    ..
                                } = s
                                {
                                    for (imp_orig, imp_alias) in imp_specs {
                                        let local_name = imp_alias.as_deref().unwrap_or(imp_orig);
                                        if local_name == orig {
                                            let sub_path =
                                                canonical.parent().unwrap().join(imp_src);
                                            if *is_all {
                                                let canonical_sub = match sub_path.canonicalize() {
                                                    Ok(p) => p,
                                                    Err(_) => sub_path,
                                                };
                                                imported_from = Some(Ok(Some((
                                                    canonical_sub,
                                                    "*".to_string(),
                                                ))));
                                            } else {
                                                imported_from = Some(resolve_module_export(
                                                    &sub_path, imp_orig, visited,
                                                ));
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                            if let Some(res) = imported_from {
                                return res;
                            }
                            found = true;
                        }
                    }
                } else if let Some(src) = source {
                    if *is_all {
                        if let Some(alias) = all_as {
                            if alias == name {
                                let sub_path = canonical.parent().unwrap().join(src);
                                let canonical_sub = match sub_path.canonicalize() {
                                    Ok(p) => p,
                                    Err(_) => sub_path,
                                };
                                return Ok(Some((canonical_sub, "*".to_string())));
                            }
                        } else if name != "default" {
                            star_sources.push(src.clone());
                        }
                    } else {
                        for (orig, alias) in specifiers {
                            let exp = alias.as_deref().unwrap_or(orig);
                            if exp == name {
                                let sub_path = canonical.parent().unwrap().join(src);
                                return resolve_module_export(&sub_path, orig, visited);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if found {
        return Ok(Some((canonical, name.to_string())));
    }

    let mut resolved_star: Option<(std::path::PathBuf, String)> = None;
    for src in star_sources {
        let sub_path = canonical.parent().unwrap().join(&src);
        if let Some(target) = resolve_module_export(&sub_path, name, visited)? {
            if let Some(ref existing) = resolved_star {
                if existing != &target {
                    return Err(format!("Ambiguous export '{}'", name));
                }
            } else {
                resolved_star = Some(target);
            }
        }
    }

    if let Some(target) = resolved_star {
        return Ok(Some(target));
    }

    Ok(None)
}

pub(crate) fn validate_module_resolution(
    program: &doms_parser::ast::Program,
    base_dir: Option<&Path>,
) -> Result<(), String> {
    let base = match base_dir {
        Some(b) => b,
        None => return Ok(()),
    };

    for stmt in &program.body {
        match stmt {
            doms_parser::ast::Statement::Import { specifiers, source, is_default, is_all } => {
                let mod_path = base.join(source);
                let content = fs::read_to_string(&mod_path).map_err(|e| e.to_string())?;
                let mod_prog = doms_parser::parse_module(&content).map_err(|e| e.to_string())?;
                if *is_all {
                    for s in &mod_prog.body {
                        if let doms_parser::ast::Statement::Export {
                            specifiers,
                            source: Some(src),
                            is_all: false,
                            ..
                        } = s
                        {
                            for (name, _) in specifiers {
                                let src_path = mod_path.parent().unwrap().join(src);
                                let mut visited = std::collections::HashSet::new();
                                if resolve_module_export(&src_path, name, &mut visited)?.is_none() {
                                    return Err(format!(
                                        "Unresolvable star namespace export '{}'",
                                        name
                                    ));
                                }
                            }
                        }
                    }
                } else {
                    for (i, (name, _)) in specifiers.iter().enumerate() {
                        let exp_name =
                            if *is_default && i == 0 { "default" } else { name.as_str() };
                        let mut visited = std::collections::HashSet::new();
                        if resolve_module_export(&mod_path, exp_name, &mut visited)?.is_none() {
                            return Err(format!(
                                "Named import '{}' not found in '{}'",
                                exp_name, source
                            ));
                        }
                    }
                }
            }
            doms_parser::ast::Statement::Export {
                specifiers,
                source: Some(source),
                is_all,
                all_as,
                ..
            } => {
                let mod_path = base.join(source);
                let content = fs::read_to_string(&mod_path).map_err(|e| e.to_string())?;
                let _mod_prog = doms_parser::parse_module(&content).map_err(|e| e.to_string())?;
                if !*is_all || all_as.is_some() {
                    for (name, _) in specifiers {
                        let mut visited = std::collections::HashSet::new();
                        if resolve_module_export(&mod_path, name, &mut visited)?.is_none() {
                            return Err(format!(
                                "Indirect export '{}' not found in '{}'",
                                name, source
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
