//! The `$262` harness object used by the Test262 suite.

use super::*;

pub(super) fn register_test262_harness(vm: &mut Vm) {
    let print_fn = vm.alloc_native_function("print", 1, native_console_log);
    vm.globals.insert("print".to_string(), print_fn);

    let test262_obj = vm.alloc_object();
    vm.set_property(test262_obj, "global", vm.global_object);

    let eval_script_fn = vm.alloc_native_function("evalScript", 1, |vm, _this, args| {
        let code = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let program = doms_parser::parse(&code).map_err(|e| VmError::SyntaxError(e.to_string()))?;
        let chunk = doms_compiler::compile(&program).map_err(|e| VmError::SyntaxError(e.to_string()))?;
        vm.in_eval = false;
        vm.run(&chunk)
    });
    vm.set_property(test262_obj, "evalScript", eval_script_fn);

    let gc_fn = vm.alloc_native_function("gc", 0, |vm, _this, _args| {
        vm.scavenge_nursery();
        let _ = vm.gc_collect();
        Ok(JsValue::undefined())
    });
    vm.set_property(test262_obj, "gc", gc_fn);

    let detach_fn = vm.alloc_native_function("detachArrayBuffer", 1, |_vm, _this, _args| {
        Ok(JsValue::undefined())
    });
    vm.set_property(test262_obj, "detachArrayBuffer", detach_fn);

    let create_realm_fn = vm.alloc_native_function("createRealm", 0, |vm, _this, _args| {
        let realm_obj = vm.alloc_object();
        let realm_glob = vm.alloc_object();
        if let Some(ptr) = realm_glob.as_object_ptr() {
            let obj = unsafe { &mut *(ptr as *mut JsObject) };
            obj.prototype = Some(vm.global_object);
        }
        vm.set_property(realm_obj, "global", realm_glob);

        let realm_eval_fn = vm.alloc_closure_native_function("eval", 1, std::sync::Arc::new(move |vm, _this, args| {
            let code_val = args.first().copied().unwrap_or(JsValue::undefined());
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
            let program = doms_parser::parse_eval(&code_str, false, false, false)
                .map_err(|e| VmError::SyntaxError(e.to_string()))?;
            let has_use_strict = program.body.iter()
                .take_while(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { .. }))))
                .any(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { value, has_escape: false, .. })) if value == "use strict"));
            let chunk = doms_compiler::compile_eval(&program, has_use_strict)
                .map_err(|e| VmError::SyntaxError(e.to_string()))?;

            let eval_scope = vm.alloc_object();
            let mut has_eval_scope = false;
            for name in &chunk.top_lex_names {
                vm.set_property(eval_scope, name, JsValue::uninitialized());
                has_eval_scope = true;
            }
            if has_use_strict {
                for name in &chunk.top_var_names {
                    vm.set_property(eval_scope, name, JsValue::undefined());
                    has_eval_scope = true;
                }
                for name in &chunk.top_func_names {
                    vm.set_property(eval_scope, name, JsValue::undefined());
                    has_eval_scope = true;
                }
            } else {
                for name in &chunk.top_var_names {
                    vm.set_property(realm_glob, name, JsValue::undefined());
                }
                for name in &chunk.top_func_names {
                    vm.set_property(realm_glob, name, JsValue::undefined());
                }
            }

            let mut eval_scopes = Vec::new();
            eval_scopes.push(realm_glob);
            if has_eval_scope {
                eval_scopes.push(eval_scope);
            }

            let root_chunk = Arc::new(chunk);
            let target_depth = vm.frames.len();
            vm.frames.push(crate::interpreter::CallFrame {
                chunk: root_chunk,
                ip: 0,
                base_sp: vm.stack.len(),
                this_val: realm_glob,
                gen_val: None,
                locals: Vec::new(),
                local_names: Vec::new(),
                lexical_local_names: Vec::new(),
                scopes: eval_scopes.clone(),
                upvalues_count: eval_scopes.len(),
                env_obj: None,
                arguments: None,
                is_strict: has_use_strict,
                new_target: None,
                is_function: false,
                is_method: false,
                is_arrow: false,
                has_param_exprs: false,
                in_param_init: false,
                is_indirect_eval: true,
                callee_func: None,
                callee_val: None,
                mapped_arguments: None,
                home_object: None,
            });
            vm.run_until_depth(target_depth)
        }));

        vm.set_property(realm_glob, "eval", realm_eval_fn);
        vm.set_property(realm_obj, "evalScript", realm_eval_fn);
        Ok(realm_obj)
    });
    vm.set_property(test262_obj, "createRealm", create_realm_fn);
    vm.set_property(test262_obj, "global", vm.global_object);
    let agent_obj = vm.alloc_object();
    vm.set_property(test262_obj, "agent", agent_obj);

    vm.globals.insert("$262".to_string(), test262_obj);

    let do_not_eval = vm.alloc_native_function("$DONOTEVALUATE", 0, |_vm, _this, _args| {
        Err(VmError::ReferenceError("Test262: This statement should not be evaluated.".into()))
    });
    vm.globals.insert("$DONOTEVALUATE".to_string(), do_not_eval);

    let t262_err = vm.alloc_native_function("Test262Error", 1, |vm, this, args| {
        let msg = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let target = if this.is_object() { this } else { vm.alloc_object() };
        let name_val = vm.alloc_string("Test262Error".into());
        let msg_val = vm.alloc_string(msg);
        vm.set_property(target, "name", name_val);
        vm.set_property(target, "message", msg_val);
        Ok(target)
    });
    vm.globals.insert("Test262Error".to_string(), t262_err);
}
