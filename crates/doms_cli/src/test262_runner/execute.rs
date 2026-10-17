use super::*;

impl Test262Runner {
    /// Execute a single test file and return TestStatus with an instruction/deadline timeout guard.
    pub fn execute_file(&self, path: &Path) -> TestStatus {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.execute_file_inner(path)
        }));
        match result {
            Ok(status) => status,
            Err(_) => TestStatus::Fail("Panic during test execution".into()),
        }
    }

    fn execute_file_inner(&self, path: &Path) -> TestStatus {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => return TestStatus::Fail(format!("Failed to read file: {}", e)),
        };

        // Skip fixtures
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if name.contains("_FIXTURE") {
                return TestStatus::Skip("Fixture file".into());
            }
        }

        let meta = Self::parse_metadata(&content);
        for feat in &meta.features {
            if feat == "resizable-arraybuffer"
                || feat == "import-defer"
                || feat == "import-attributes"
                || feat == "import-bytes"
                || feat == "import-text"
                || feat == "regexp-modifiers"
                || feat == "source-phase-imports"
                || feat == "source-phase-imports-module-source"
                || feat == "explicit-resource-management"
            {
                return TestStatus::Skip(format!("Feature '{}' not supported", feat));
            }
        }
        let full_source = self.build_source(&content, &meta);

        // 1. Parsing Phase
        let parsed = if meta.is_module {
            doms_parser::parse_module(&full_source)
        } else if meta.is_only_strict {
            doms_parser::parse_strict(&full_source)
        } else {
            doms_parser::parse(&full_source)
        };
        if let Err(parse_err) = parsed {
            if meta.negative_phase.as_deref() == Some("parse") {
                return TestStatus::Pass;
            }
            return TestStatus::Fail(format!("Parse error: {}", parse_err));
        }

        let program = parsed.unwrap();
        if meta.negative_phase.as_deref() == Some("parse") {
            return TestStatus::Fail("Expected parse error, but parsed successfully".into());
        }

        if meta.is_module {
            let res_result = validate_module_resolution(&program, path.parent());
            if let Err(err) = res_result {
                if meta.negative_phase.as_deref() == Some("resolution") {
                    return TestStatus::Pass;
                }
                return TestStatus::Fail(format!("Resolution error: {}", err));
            } else if meta.negative_phase.as_deref() == Some("resolution") {
                return TestStatus::Fail("Expected resolution error, but resolved successfully".into());
            }
        }

        let is_top_level_strict = meta.is_only_strict
            || meta.is_module
            || program.body.iter()
                .take_while(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { .. }))))
                .any(|s| matches!(s, doms_parser::ast::Statement::Expression(doms_parser::ast::Expression::Literal(doms_parser::ast::Literal::String { value, has_escape: false, .. })) if value == "use strict"));

        // 2. Compilation Phase
        let compiled = if is_top_level_strict {
            doms_compiler::compile_strict(&program)
        } else {
            doms_compiler::compile(&program)
        };
        if let Err(comp_err) = compiled {
            if meta.negative_phase.as_deref() == Some("parse")
                || meta.negative_phase.as_deref() == Some("runtime")
            {
                return TestStatus::Pass;
            }
            return TestStatus::Fail(format!("Compile error: {}", comp_err));
        }

        let chunk = compiled.unwrap();


        // 3. Execution Phase
        let mut vm = doms_vm::Vm::new();
        vm.is_strict = is_top_level_strict;
        vm.is_module = meta.is_module;
        vm.base_dir = path.parent().map(|p| p.to_path_buf());
        vm.max_instructions = Some(80_000_000);
        vm.max_objects = Some(1_000_000);
        vm.max_evals = Some(100_000);
        vm.deadline = Some(Instant::now() + std::time::Duration::from_millis(3_000));

        if meta.is_module {
            let m = vm.alloc_object();
            vm.set_prototype(m, doms_vm::JsValue::null());
            if let Some(tag_sym) = vm.symbol_to_string_tag {
                let tag_val = vm.alloc_string("Module".to_string());
                vm.set_symbol_property(m, tag_sym, tag_val);
                vm.non_configurable_props.insert((m, "[Symbol.toStringTag]".to_string()));
                vm.non_enumerable_props.insert((m, "[Symbol.toStringTag]".to_string()));
                vm.non_writable_props.insert((m, "[Symbol.toStringTag]".to_string()));
            }
            vm.module_namespace_objects.insert(m);
            vm.non_extensible_objects.insert(m);
            vm.current_module_obj = Some(m);

            let canonical_key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
                .to_string_lossy().to_string();
            vm.modules.insert(canonical_key, m);
            if let Some(name) = path.file_name() {
                vm.modules.insert(format!("./{}", name.to_string_lossy()), m);
                vm.modules.insert(name.to_string_lossy().to_string(), m);
            }
            vm.current_module_obj = Some(m);
        }

        let result = vm.run(&chunk);

        match result {
            Ok(_) => {
                if meta.negative_phase.as_deref() == Some("runtime") {
                    TestStatus::Fail("Expected runtime error, but executed successfully".into())
                } else {
                    TestStatus::Pass
                }
            }
            Err(run_err) => {
                if meta.negative_phase.as_deref() == Some("runtime") {
                    TestStatus::Pass
                } else {
                    let msg = match run_err {
                        doms_vm::VmError::Thrown(val) => {
                            let str_val = vm.to_string_value(val);
                            let name_prop = vm.get_property(val, "name");
                            let message_prop = vm.get_property(val, "message");
                            if !message_prop.is_undefined() {
                                if !name_prop.is_undefined() {
                                    format!("{}: {}", vm.to_string_value(name_prop), vm.to_string_value(message_prop))
                                } else {
                                    format!("{}: {}", str_val, vm.to_string_value(message_prop))
                                }
                            } else {
                                str_val
                            }
                        }
                        other => other.to_string(),
                    };
                    TestStatus::Fail(format!("Runtime error: {}", msg))
                }
            }
        }
    }
}
