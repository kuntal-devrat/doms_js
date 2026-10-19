//! The global `eval` function.

use super::*;

pub(super) fn register_eval(vm: &mut Vm) {
    // eval
    let eval_fn = vm.alloc_native_function("eval", 1, |vm, _this, args| {
        if args.is_empty() {
            return Ok(JsValue::undefined());
        }
        let code_val = args[0];
        let code_str = if let Some(ptr) = code_val.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::String(ref s) = obj.kind {
                s.clone()
            } else {
                return Ok(code_val);
            }
        } else {
            return Ok(code_val);
        };

        if let Some(max) = vm.max_evals {
            vm.eval_count += 1;
            if vm.eval_count > max {
                return Err(VmError::RangeError(
                    "Execution limit exceeded in eval (too many calls)".to_string(),
                ));
            }
        }
        if let Some(deadline) = vm.deadline {
            if std::time::Instant::now() > deadline {
                return Err(VmError::RuntimeError(
                    "Execution timeout exceeded in eval".to_string(),
                ));
            }
        }
        if vm.oom {
            return Err(VmError::RangeError("Out of memory in eval".to_string()));
        }

        let is_direct = std::mem::take(&mut vm.in_direct_eval);
        let (caller_strict, caller_in_method, caller_in_function, caller_is_fn, caller_home_obj) = if is_direct {
            if let Some(f) = vm.frames.last() {
                (f.is_strict, f.is_method, f.is_function && !f.is_arrow, f.is_function, f.home_object)
            } else {
                (vm.is_strict, false, false, false, None)
            }
        } else {
            (false, false, false, false, None)
        };

        let program = doms_parser::parse_eval(&code_str, caller_strict, caller_in_method, caller_in_function)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;

        let has_use_strict = program.body.iter()
            .take_while(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { .. }))))
            .any(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { value, has_escape: false, .. })) if value == "use strict"));

        let is_eval_strict = caller_strict || has_use_strict;

        let chunk = doms_compiler::compile_eval(&program, is_eval_strict)
            .map_err(|e| VmError::SyntaxError(e.to_string()))?;

        vm.in_eval = true;
        let res = (|| {
            let eval_scope = vm.alloc_object();
            let mut has_eval_scope = false;

            for name in &chunk.top_lex_names {
                vm.set_property(eval_scope, name, JsValue::uninitialized());
                has_eval_scope = true;
            }

            let caller_local_names = vm.frames.last().map(|f| f.local_names.clone()).unwrap_or_default();
            let caller_locals = vm.frames.last().map(|f| f.locals.clone()).unwrap_or_default();
            let caller_env_obj = vm.frames.last().and_then(|f| f.env_obj);

            if is_eval_strict {
                for name in &chunk.top_var_names {
                    vm.set_property(eval_scope, name, JsValue::undefined());
                    has_eval_scope = true;
                }
                for name in &chunk.top_func_names {
                    vm.set_property(eval_scope, name, JsValue::undefined());
                    has_eval_scope = true;
                }
            } else {
                if is_direct {
                    let caller_lexical_names = vm.frames.last().map(|f| f.lexical_local_names.clone()).unwrap_or_default();
                    let caller_in_param_init = vm.frames.last().map(|f| f.in_param_init).unwrap_or(false);
                    let caller_is_arrow = vm.frames.last().map(|f| f.is_arrow).unwrap_or(false);
                    let caller_params: Vec<String> = vm.frames.last()
                        .and_then(|f| f.callee_func.as_ref().map(|func| func.params.clone()))
                        .unwrap_or_default();

                    for name in chunk.top_var_names.iter().chain(chunk.top_func_names.iter()) {
                        if caller_in_param_init {
                            let clean_params: Vec<&str> = caller_params.iter().map(|p| {
                                let trimmed = p.split('=').next().unwrap().trim();
                                trimmed.strip_prefix("...").unwrap_or(trimmed)
                            }).collect();
                            if clean_params.contains(&name.as_str()) {
                                return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                            }
                            if name == "arguments" && !caller_is_arrow {
                                return Err(VmError::SyntaxError("Cannot declare 'arguments' in function with parameter expressions".into()));
                            }
                        } else {
                            if caller_lexical_names.contains(name) {
                                return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                            }
                        }
                    }
                }
                for name in chunk.top_var_names.iter().chain(chunk.top_func_names.iter()) {
                    if vm.global_lexicals.contains_key(name) {
                        return Err(VmError::SyntaxError(format!("Identifier '{}' has already been declared", name)));
                    }
                }

                let caller_has_fn = is_direct && caller_is_fn;
                if caller_has_fn {
                    let caller_env = if let Some(env) = caller_env_obj {
                        env
                    } else {
                        let env = vm.alloc_object();
                        for (i, name) in caller_local_names.iter().enumerate() {
                            let val = caller_locals.get(i).copied().unwrap_or(JsValue::undefined());
                            vm.set_property(env, name, val);
                            vm.non_configurable_props.insert((env, name.clone()));
                        }
                        if let Some(caller_f) = vm.frames.last_mut() {
                            if !caller_f.in_param_init {
                                caller_f.env_obj = Some(env);
                            }
                            let insert_pos = caller_f.upvalues_count.min(caller_f.scopes.len());
                            caller_f.scopes.insert(insert_pos, env);
                        }
                        env
                    };
                    for name in &chunk.top_var_names {
                        if !vm.has_property(caller_env, name).unwrap_or(false) {
                            vm.set_property(caller_env, name, JsValue::undefined());
                        }
                    }
                    for name in &chunk.top_func_names {
                        if !vm.has_property(caller_env, name).unwrap_or(false) {
                            vm.set_property(caller_env, name, JsValue::undefined());
                        }
                    }
                } else {
                    let is_extensible = vm.is_extensible_object(vm.global_object);
                    if !is_extensible {
                        for name in chunk.top_var_names.iter().chain(chunk.top_func_names.iter()) {
                            let has = if let Some(ptr) = vm.global_object.as_object_ptr() {
                                let obj = unsafe { &*(ptr as *mut JsObject) };
                                vm.shapes.has_property(obj.shape_id, name)
                            } else {
                                false
                            };
                            if !has {
                                return Err(VmError::TypeError(format!("Cannot define variable '{}' on non-extensible global object", name)));
                            }
                        }
                    }
                    for name in &chunk.top_var_names {
                        vm.var_names.insert(name.clone());
                        let has = if let Some(ptr) = vm.global_object.as_object_ptr() {
                            let obj = unsafe { &*(ptr as *mut JsObject) };
                            vm.shapes.has_property(obj.shape_id, name)
                        } else {
                            false
                        };
                        if !has {
                            vm.globals.insert(name.clone(), JsValue::undefined());
                            if vm.global_object.is_object() {
                                vm.set_property(vm.global_object, name, JsValue::undefined());
                            }
                        }
                    }
                    for name in &chunk.top_func_names {
                        vm.var_names.insert(name.clone());
                        if !vm.globals.contains_key(name) {
                            vm.globals.insert(name.clone(), JsValue::undefined());
                            if vm.global_object.is_object() {
                                vm.set_property(vm.global_object, name, JsValue::undefined());
                            }
                        }
                    }
                }
            }

            let effective_this = if is_direct {
                vm.frames.last().map(|f| f.this_val).unwrap_or(vm.global_object)
            } else {
                vm.global_object
            };
            let mut eval_scopes = if is_direct {
                vm.frames.last().map(|f| f.scopes.clone()).unwrap_or_default()
            } else {
                Vec::new()
            };
            if has_eval_scope {
                eval_scopes.push(eval_scope);
            }

            let new_target = if is_direct {
                vm.frames.last().and_then(|f| f.new_target)
            } else {
                None
            };

            let target_depth = vm.frames.len();
            let base_sp = vm.stack.len();
            vm.frames.push(crate::interpreter::CallFrame {
                chunk: Arc::new(chunk),
                ip: 0,
                base_sp,
                this_val: effective_this,
                gen_val: None,
                locals: Vec::new(),
                local_names: std::sync::Arc::new(Vec::new()),
                lexical_local_names: std::sync::Arc::new(Vec::new()),
                scopes: eval_scopes.clone(),
                upvalues_count: eval_scopes.len(),
                env_obj: None,
                arguments: None,
                raw_args: None,
                is_strict: is_eval_strict,
                new_target,
                is_function: false,
                is_method: false,
                is_arrow: false,
                has_param_exprs: false,
                in_param_init: false,
                is_indirect_eval: !is_direct,
                callee_func: None,
                callee_val: None,
                mapped_arguments: None,
                home_object: caller_home_obj,
            });
            vm.run_until_depth(target_depth)
        })();
        vm.in_eval = false;
        res
    });
    vm.globals.insert("eval".to_string(), eval_fn);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "eval", eval_fn);
    }
}
