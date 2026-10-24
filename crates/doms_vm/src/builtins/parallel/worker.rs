//! The `Worker` constructor for nested isolates.

use super::*;

pub(super) fn register_worker(vm: &mut Vm) {
    let worker_ctor = vm.alloc_native_function("Worker", 1, |vm, _this, args| {
        let script = args.first().map(|v| vm.to_string_value(*v)).unwrap_or_default();
        let channel = doms_parallel::MessageChannel::new();
        let child_port = channel.port2;

        let base_dir = vm.base_dir.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        std::thread::spawn(move || {
            let mut worker_vm = Vm::new();
            worker_vm.base_dir = Some(base_dir.clone());
            let parent_port = Arc::new(std::sync::Mutex::new(child_port));
            let post_msg = worker_vm.alloc_closure_native_function(
                "postMessage",
                1,
                Arc::new({
                    let p = Arc::clone(&parent_port);
                    move |vm: &mut Vm,
                          _this: JsValue,
                          args: &[JsValue]|
                          -> Result<JsValue, VmError> {
                        let msg = args.first().copied().unwrap_or_default();
                        let json = vm.to_json_string(msg);
                        let guard = p.lock().unwrap();
                        guard
                            .post_message(json.into_bytes())
                            .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                        Ok(JsValue::undefined())
                    }
                }),
            );
            worker_vm.globals.insert("postMessage".to_string(), post_msg);

            let mut resolved_safe_path = None;
            let target = base_dir.join(&script);
            if let (Ok(canonical_base), Ok(canonical_target)) = (base_dir.canonicalize(), target.canonicalize()) {
                if canonical_target.starts_with(&canonical_base) {
                    resolved_safe_path = Some(canonical_target);
                }
            }

            let code = if let Some(safe_path) = resolved_safe_path {
                if safe_path.exists() {
                    std::fs::read_to_string(&safe_path).unwrap_or(script.clone())
                } else {
                    script.clone()
                }
            } else {
                script.clone()
            };

            if let Ok(prog) = doms_parser::parse(&code) {
                if let Ok(chunk) = doms_compiler::compile(&prog) {
                    let _ = worker_vm.run(&chunk);
                }
            }
        });

        let port_arc = Arc::new(std::sync::Mutex::new(channel.port1));
        Ok(vm.alloc_worker(port_arc))
    });

    vm.globals.insert("Worker".to_string(), worker_ctor);
}
