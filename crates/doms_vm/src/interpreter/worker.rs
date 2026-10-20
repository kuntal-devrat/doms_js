//! Worker `postMessage`/`onmessage` plumbing.

use super::*;

impl Vm {
    pub(crate) fn get_worker_post_message(
        &mut self,
        port: Arc<std::sync::Mutex<doms_parallel::MessagePort>>,
    ) -> Option<JsValue> {
        Some(self.alloc_closure_native_function(
            "postMessage",
            1,
            Arc::new(
                move |vm: &mut Vm, _this: JsValue, args: &[JsValue]| -> Result<JsValue, VmError> {
                    let msg = args.first().copied().unwrap_or_default();
                    let json = vm.to_json_string(msg);
                    let guard = port.lock().unwrap();
                    guard
                        .post_message(json.into_bytes())
                        .map_err(|e| VmError::RuntimeError(e.to_string()))?;
                    Ok(JsValue::undefined())
                },
            ),
        ))
    }

    pub(crate) fn get_worker_receive_message(
        &mut self,
        port: Arc<std::sync::Mutex<doms_parallel::MessagePort>>,
    ) -> Option<JsValue> {
        Some(self.alloc_closure_native_function(
            "receiveMessage",
            0,
            Arc::new(
                move |vm: &mut Vm, _this: JsValue, _args: &[JsValue]| -> Result<JsValue, VmError> {
                    let mut data_opt = {
                        let guard = port.lock().unwrap();
                        guard.try_recv().ok()
                    };

                    if data_opt.is_none() {
                        for _ in 0..50 {
                            std::thread::sleep(std::time::Duration::from_millis(2));
                            let guard = port.lock().unwrap();
                            if let Ok(data) = guard.try_recv() {
                                data_opt = Some(data);
                                break;
                            }
                        }
                    }

                    match data_opt {
                        Some(data) => {
                            let s = String::from_utf8_lossy(&data.payload).to_string();
                            if let Ok(n) = s.parse::<f64>() {
                                Ok(JsValue::from_double(n))
                            } else if s == "true" {
                                Ok(JsValue::from_boolean(true))
                            } else if s == "false" {
                                Ok(JsValue::from_boolean(false))
                            } else {
                                let mut clean = s;
                                if clean.starts_with('"')
                                    && clean.ends_with('"')
                                    && clean.len() >= 2
                                {
                                    clean = clean[1..clean.len() - 1].to_string();
                                }
                                Ok(vm.alloc_string(clean))
                            }
                        }
                        None => Ok(JsValue::null()),
                    }
                },
            ),
        ))
    }
}
