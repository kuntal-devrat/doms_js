//! `for...in` key enumeration.

use super::*;

impl Vm {
    pub fn get_for_in_keys(&mut self, curr: JsValue) -> Result<Vec<String>, VmError> {
        if !curr.is_object() {
            if let Some(ptr) = curr.as_object_ptr() {
                let obj = unsafe { &*(ptr as *mut JsObject) };
                if let ObjectKind::String(ref s) | ObjectKind::StringObject(ref s) = obj.kind {
                    return Ok((0..s.chars().count()).map(|i| i.to_string()).collect());
                }
            }
            return Ok(Vec::new());
        }
        let mut visited: Vec<String> = Vec::new();
        let mut current_val = Some(curr);

        while let Some(obj_val) = current_val {
            if let Some(ptr) = obj_val.as_object_ptr() {
                let is_proxy = matches!(unsafe { &*(ptr as *mut JsObject) }.kind, ObjectKind::Proxy { .. });
                if is_proxy {
                    let keys = self.proxy_own_property_keys(obj_val)?;
                    for k in keys {
                        if let PropertyKeyVal::String(s) = k {
                            let desc = crate::builtins::object::descriptor::get_own_property_descriptor_internal(self, obj_val, &PropertyKeyVal::String(s.clone()))?;
                            if !desc.is_undefined() && self.get_property(desc, "enumerable").is_truthy() {
                                if !visited.contains(&s) {
                                    visited.push(s);
                                }
                            }
                        }
                    }
                    current_val = self.get_prototype_of_val(obj_val).ok().and_then(|p| if p.is_null() || !p.is_object() { None } else { Some(p) });
                    continue;
                }

                let obj = unsafe { &*(ptr as *mut JsObject) };
                let mut int_keys: Vec<(usize, String)> = Vec::new();
                let mut str_keys: Vec<String> = Vec::new();

                // 1. Array elements or string characters
                if let ObjectKind::Array(ref elements) = obj.kind {
                    for i in 0..elements.len() {
                        if elements[i].is_hole() {
                            continue;
                        }
                        let k = i.to_string();
                        if !self.non_enumerable_props.contains(&(obj_val, k.clone()))
                            && !visited.contains(&k)
                        {
                            int_keys.push((i, k));
                        }
                    }
                } else if let ObjectKind::String(ref s) | ObjectKind::StringObject(ref s) = obj.kind {
                    for (i, _) in s.chars().enumerate() {
                        let k = i.to_string();
                        if !self.non_enumerable_props.contains(&(obj_val, k.clone()))
                            && !visited.contains(&k)
                        {
                            int_keys.push((i, k));
                        }
                    }
                }

                // 2. Accessors
                if let Some(accs) = &obj.accessors {
                    for (k, acc) in accs.iter() {
                        if acc.enumerable
                            && !self.non_enumerable_props.contains(&(obj_val, k.clone()))
                            && !visited.contains(k)
                        {
                            if let Ok(idx) = k.parse::<usize>() {
                                if idx.to_string() == *k {
                                    if !int_keys.iter().any(|(_, ik)| ik == k) {
                                        int_keys.push((idx, k.clone()));
                                    }
                                    continue;
                                }
                            }
                            if !str_keys.contains(k) {
                                str_keys.push(k.clone());
                            }
                        }
                    }
                }

                // 3. Properties from shape in insertion order
                for (prop, _, attrs) in self.shapes.all_properties_with_attributes(obj.shape_id) {
                    if attrs.enumerable
                        && !self.non_enumerable_props.contains(&(obj_val, prop.clone()))
                        && !visited.contains(&prop)
                    {
                        if let Ok(idx) = prop.parse::<usize>() {
                            if idx.to_string() == prop {
                                if !int_keys.iter().any(|(_, ik)| ik == &prop) {
                                    int_keys.push((idx, prop));
                                }
                                continue;
                            }
                        }
                        if !str_keys.contains(&prop) {
                            str_keys.push(prop);
                        }
                    }
                }

                int_keys.sort_by_key(|(idx, _)| *idx);
                for (_, k) in int_keys {
                    visited.push(k);
                }
                for k in str_keys {
                    visited.push(k);
                }

                current_val = self.get_prototype_of_val(obj_val).ok().and_then(|p| if p.is_null() || !p.is_object() { None } else { Some(p) });
            } else {
                break;
            }
        }

        Ok(visited)
    }
}
