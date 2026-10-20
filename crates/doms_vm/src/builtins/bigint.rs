//! The `BigInt` constructor and BigInt helpers.

use super::*;

fn to_bigint_value(vm: &mut Vm, val: JsValue) -> Result<BigInt, VmError> {
    if val.is_nullish() {
        return Err(VmError::TypeError("Cannot convert undefined or null to a BigInt".into()));
    }
    if val.is_symbol() {
        return Err(VmError::TypeError("Cannot convert a Symbol value to a BigInt".into()));
    }
    let prim = vm.to_primitive(val, crate::interpreter::PreferredType::Number)?;
    if let Some(ptr) = prim.as_object_ptr() {
        let obj = unsafe { &*(ptr as *mut JsObject) };
        if let ObjectKind::BigInt(b) | ObjectKind::BigIntObject(b) = &obj.kind {
            return Ok(b.clone());
        }
    }
    if prim.is_boolean() {
        return Ok(if prim.as_boolean() == Some(true) { BigInt::one() } else { BigInt::zero() });
    }
    if prim.is_number() {
        let n = prim.to_number().unwrap_or(0.0);
        if !n.is_finite() {
            return Err(VmError::RangeError(format!("Cannot convert {} to a BigInt", n)));
        }
        if n.fract() != 0.0 {
            return Err(VmError::RangeError(format!(
                "The number {} cannot be converted to a BigInt because it is not an integer",
                n
            )));
        }
        return BigInt::from_f64(n)
            .ok_or_else(|| VmError::RangeError(format!("Cannot convert {} to a BigInt", n)));
    }
    if prim.is_string()
        || (prim.is_object()
            && prim.as_object_ptr().is_some_and(|p| {
                matches!(unsafe { &(*(p as *mut JsObject)).kind }, ObjectKind::String(_))
            }))
    {
        let s = vm.to_string_value(prim);
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Ok(BigInt::zero());
        }
        let (is_neg, rest) = if let Some(stripped) = trimmed.strip_prefix('+') {
            (false, stripped)
        } else if let Some(stripped) = trimmed.strip_prefix('-') {
            (true, stripped)
        } else {
            (false, trimmed)
        };
        let bi_res = if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X"))
        {
            if is_neg {
                return Err(VmError::SyntaxError(
                    "Cannot convert string with sign and hex prefix to a BigInt".into(),
                ));
            }
            BigInt::parse_bytes(hex.as_bytes(), 16)
        } else if let Some(bin) = rest.strip_prefix("0b").or_else(|| rest.strip_prefix("0B")) {
            if is_neg {
                return Err(VmError::SyntaxError(
                    "Cannot convert string with sign and binary prefix to a BigInt".into(),
                ));
            }
            BigInt::parse_bytes(bin.as_bytes(), 2)
        } else if let Some(oct) = rest.strip_prefix("0o").or_else(|| rest.strip_prefix("0O")) {
            if is_neg {
                return Err(VmError::SyntaxError(
                    "Cannot convert string with sign and octal prefix to a BigInt".into(),
                ));
            }
            BigInt::parse_bytes(oct.as_bytes(), 8)
        } else {
            if rest.contains('.') || rest.contains('e') || rest.contains('E') || rest.contains('n')
            {
                return Err(VmError::SyntaxError(
                    "Cannot convert string with decimal or exponent to a BigInt".into(),
                ));
            }
            BigInt::parse_bytes(rest.as_bytes(), 10).map(|mut bi| {
                if is_neg {
                    bi = -bi;
                }
                bi
            })
        };
        return bi_res.ok_or_else(|| {
            VmError::SyntaxError(format!("Cannot convert string '{}' to a BigInt", s))
        });
    }
    Err(VmError::TypeError("Cannot convert value to a BigInt".into()))
}

pub(super) fn register_bigint(vm: &mut Vm) {
    let bigint_proto = vm.alloc_object();

    let to_string_fn = vm.alloc_native_function("toString", 0, |vm, this, args| {
        let bi = if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if let ObjectKind::BigInt(val) = &obj.kind {
                val.clone()
            } else {
                return Err(VmError::TypeError(
                    "BigInt.prototype.toString requires that 'this' be a BigInt".into(),
                ));
            }
        } else {
            return Err(VmError::TypeError(
                "BigInt.prototype.toString requires that 'this' be a BigInt".into(),
            ));
        };
        let radix = if let Some(r_val) = args.first() {
            if r_val.is_undefined() {
                10
            } else {
                if r_val.is_symbol()
                    || (r_val.is_object()
                        && r_val.as_object_ptr().is_some_and(|p| {
                            matches!(
                                unsafe { &(*(p as *mut JsObject)).kind },
                                ObjectKind::BigInt(_)
                            )
                        }))
                {
                    return Err(VmError::TypeError(
                        "Cannot convert Symbol or BigInt to number".into(),
                    ));
                }
                let r = vm
                    .to_number_value(*r_val)
                    .map_err(|_| VmError::TypeError("radix must be convertible to number".into()))?
                    as i32;
                if !(2..=36).contains(&r) {
                    return Err(VmError::RangeError(
                        "radix must be an integer at least 2 and no greater than 36".into(),
                    ));
                }
                r
            }
        } else {
            10
        };
        let s = bi.to_str_radix(radix as u32);
        Ok(vm.alloc_string(s))
    });

    let value_of_fn = vm.alloc_native_function("valueOf", 0, |_vm, this, _args| {
        if let Some(ptr) = this.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            if matches!(obj.kind, ObjectKind::BigInt(_)) {
                return Ok(this);
            }
        }
        Err(VmError::TypeError("BigInt.prototype.valueOf requires that 'this' be a BigInt".into()))
    });

    let to_locale_string_fn = vm.alloc_native_function("toLocaleString", 0, |vm, this, args| {
        let to_str = vm.get_property(this, "toString");
        if vm.is_callable(to_str) {
            vm.call(to_str, this, args)
        } else {
            Err(VmError::TypeError("toString is not callable".into()))
        }
    });

    vm.set_property(bigint_proto, "toString", to_string_fn);
    vm.set_property_non_enumerable(bigint_proto, "toString".to_string());
    vm.set_property(bigint_proto, "valueOf", value_of_fn);
    vm.set_property_non_enumerable(bigint_proto, "valueOf".to_string());
    vm.set_property(bigint_proto, "toLocaleString", to_locale_string_fn);
    vm.set_property_non_enumerable(bigint_proto, "toLocaleString".to_string());

    if let Some(tag_sym) = vm.symbol_to_string_tag {
        let tag_val = vm.alloc_string("BigInt".to_string());
        vm.set_symbol_property(bigint_proto, tag_sym, tag_val);
    }

    let bigint_ctor = vm.alloc_native_function("BigInt", 1, |vm, _this, args| {
        if vm.is_constructing {
            return Err(VmError::TypeError("BigInt is not a constructor".into()));
        }
        let val = args.first().copied().unwrap_or(JsValue::undefined());
        let bi_val = to_bigint_value(vm, val)?;
        Ok(vm.alloc_bigint(bi_val))
    });

    vm.set_property(bigint_proto, "constructor", bigint_ctor);
    vm.set_property_non_enumerable(bigint_proto, "constructor".to_string());
    vm.set_property(bigint_ctor, "prototype", bigint_proto);
    vm.set_property_non_enumerable(bigint_ctor, "prototype".to_string());
    vm.set_property_non_writable(bigint_ctor, "prototype".to_string());
    vm.set_property_non_configurable(bigint_ctor, "prototype".to_string());

    let as_int_n = vm.alloc_native_function("asIntN", 2, |vm, _this, args| {
        let bits_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let bits_f = vm.to_number_value(bits_arg).unwrap_or(0.0);
        let bits = if bits_f <= 0.0 { 0 } else { bits_f as usize };
        let bi_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let bi = to_bigint_value(vm, bi_arg)?;
        if bits == 0 {
            return Ok(vm.alloc_bigint(BigInt::zero()));
        }
        if bits >= 100_000 {
            if !bi.is_negative() && (bi.bits() as usize) < bits {
                return Ok(vm.alloc_bigint(bi));
            }
            if bi.is_negative() && ((-&bi).bits() as usize) < bits {
                return Ok(vm.alloc_bigint(bi));
            }
            return Err(VmError::RangeError(
                "BigInt bit length exceeds maximum supported size".into(),
            ));
        }
        let two_to_bits = BigInt::one() << bits;
        let mut mod_val = bi % &two_to_bits;
        if mod_val.is_negative() {
            mod_val += &two_to_bits;
        }
        let half = &two_to_bits >> 1;
        if mod_val >= half {
            mod_val -= two_to_bits;
        }
        Ok(vm.alloc_bigint(mod_val))
    });

    let as_uint_n = vm.alloc_native_function("asUintN", 2, |vm, _this, args| {
        let bits_arg = args.first().copied().unwrap_or(JsValue::undefined());
        let bits_f = vm.to_number_value(bits_arg).unwrap_or(0.0);
        let bits = if bits_f <= 0.0 { 0 } else { bits_f as usize };
        let bi_arg = args.get(1).copied().unwrap_or(JsValue::undefined());
        let bi = to_bigint_value(vm, bi_arg)?;
        if bits == 0 {
            return Ok(vm.alloc_bigint(BigInt::zero()));
        }
        if bits >= 100_000 {
            if !bi.is_negative() && (bi.bits() as usize) <= bits {
                return Ok(vm.alloc_bigint(bi));
            }
            return Err(VmError::RangeError(
                "BigInt bit length exceeds maximum supported size".into(),
            ));
        }
        let two_to_bits = BigInt::one() << bits;
        let mut mod_val = bi % &two_to_bits;
        if mod_val.is_negative() {
            mod_val += &two_to_bits;
        }
        Ok(vm.alloc_bigint(mod_val))
    });

    vm.set_property(bigint_ctor, "asIntN", as_int_n);
    vm.set_property_non_enumerable(bigint_ctor, "asIntN".to_string());
    vm.set_property(bigint_ctor, "asUintN", as_uint_n);
    vm.set_property_non_enumerable(bigint_ctor, "asUintN".to_string());

    vm.globals.insert("BigInt".to_string(), bigint_ctor);
    if vm.global_object.is_object() {
        vm.set_property(vm.global_object, "BigInt", bigint_ctor);
    }
}
