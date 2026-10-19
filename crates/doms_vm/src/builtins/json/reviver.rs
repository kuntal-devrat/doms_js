//! `JSON.parse` reviver walk.

use super::*;

pub(super) fn json_walk(vm: &mut Vm, reviver: JsValue, holder: JsValue, name: &str) -> Result<JsValue, VmError> {
    let prev_temp = vm.temp_roots.len();
    vm.temp_roots.push(holder);
    vm.temp_roots.push(reviver);

    let val = vm.get_property_result(holder, name)?;
    vm.temp_roots.push(val);

    let res = (|| -> Result<JsValue, VmError> {
        let cur_val = vm.temp_roots[prev_temp + 2];
        if cur_val.is_js_object() {
            if vm.is_array_result(cur_val)? {
                let len_val = vm.get_property_result(cur_val, "length")?;
                let len = vm.to_number_value(len_val).unwrap_or(0.0).max(0.0) as usize;
                for i in 0..len {
                    let prop_str = i.to_string();
                    let cur_v = vm.temp_roots[prev_temp + 2];
                    let cur_r = vm.temp_roots[prev_temp + 1];
                    let new_elem = json_walk(vm, cur_r, cur_v, &prop_str)?;
                    let cur_v = vm.temp_roots[prev_temp + 2];
                    if new_elem.is_undefined() {
                        let _ = vm.delete_property(cur_v, &prop_str)?;
                    } else {
                        let _ = vm.create_data_property(cur_v, &prop_str, new_elem)?;
                    }
                }
            } else {
                let key_vals = crate::builtins::object::enumerable_own_property_names(
                    vm,
                    cur_val,
                    crate::builtins::object::EnumerableKind::Key,
                )?;
                let keys: Vec<String> = key_vals.into_iter().map(|k| vm.to_string_value(k)).collect();
                for k in keys {
                    let cur_v = vm.temp_roots[prev_temp + 2];
                    let cur_r = vm.temp_roots[prev_temp + 1];
                    let new_val = json_walk(vm, cur_r, cur_v, &k)?;
                    let cur_v = vm.temp_roots[prev_temp + 2];
                    if new_val.is_undefined() {
                        let _ = vm.delete_property(cur_v, &k)?;
                    } else {
                        let _ = vm.create_data_property(cur_v, &k, new_val)?;
                    }
                }
            }
        }

        let cur_h = vm.temp_roots[prev_temp];
        let cur_r = vm.temp_roots[prev_temp + 1];
        let cur_v = vm.temp_roots[prev_temp + 2];

        let name_val = vm.alloc_string(name.to_string());
        vm.call(cur_r, cur_h, &[name_val, cur_v])
    })();

    vm.temp_roots.truncate(prev_temp);
    res
}
