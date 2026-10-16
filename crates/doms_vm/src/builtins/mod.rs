//! Built-in JavaScript objects and global functions.
//!
//! [`register_builtins`] runs once when a [`Vm`](crate::interpreter::Vm) is
//! created and populates the global object. The implementation is split into
//! one submodule per builtin area so contributors can extend a single builtin
//! without scrolling through a monolith:
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`console`] | `console.log` and friends |
//! | [`math`] | The `Math` namespace |
//! | [`json`] | `JSON.parse` / `JSON.stringify` |
//! | [`symbol`] | `Symbol` and the well-known symbols |
//! | [`object`] | `Object` statics and `Object.prototype` |
//! | [`array`] | `Array` statics and `Array.prototype` |
//! | [`function`] | `Function.prototype` (`call`/`apply`/`bind`) |
//! | [`uri`] | `encodeURI`, `decodeURIComponent`, … |
//! | [`globals`] | `parseInt`, `eval`, `String`, `Number`, `Boolean`, `Error*` and `Date` |
//! | [`promise`] | `Promise` and its prototype |
//! | [`typed_array`] | Typed array constructors |
//! | [`parallel`] | `SharedArrayBuffer`, `ArrayBuffer`, `DataView`, `Atomics`, `Worker` |
//! | [`phase3`] | Collections, `RegExp`, `Intl`, `Temporal` shims |
//! | [`test262_harness`] | The `$262` Test262 harness object |
//! | [`reflect`] | The `Reflect` namespace |
//! | [`proxy`] | The `Proxy` constructor |
//! | [`bigint`] | `BigInt` |
//! | [`helpers`] | Helpers shared by several registrars |
//!
//! Each `register_*` function is called from [`register_builtins`] below.

use crate::interpreter::{PropertyKeyVal, Vm, VmError};
use crate::object::{Accessor, JsObject, JsTypedArray, NativeFn, ObjectKind, TypedArrayKind};
use crate::value::JsValue;
use num_bigint::BigInt;
use num_traits::{FromPrimitive, One, Signed, Zero};
use std::sync::Arc;

mod array;
mod bigint;
mod console;
mod function;
mod globals;
mod helpers;
mod json;
mod math;
mod object;
mod parallel;
mod phase3;
mod promise;
mod proxy;
mod reflect;
mod symbol;
mod test262_harness;
mod typed_array;
mod uri;

use array::register_array_builtins;
use bigint::register_bigint;
use console::{native_console_log, register_console};
use function::register_function_builtins;
use globals::register_global_helpers;
use helpers::{define_property_on_module_namespace, get_enumerable_keys, iterate_to_vec};
use json::register_json;
use math::register_math;
use object::register_object_builtins;
use parallel::register_parallel_and_typed_arrays;
use phase3::register_phase3_builtins;
use promise::register_promise;
use proxy::register_proxy;
use reflect::register_reflect;
use symbol::register_symbol;
use test262_harness::register_test262_harness;
use typed_array::{IterKind, create_array_iterator};
use uri::{uri_decode, uri_encode};

pub(crate) use helpers::add_species_getter;

pub fn register_builtins(vm: &mut Vm) {
    register_symbol(vm);
    register_object_builtins(vm);
    register_function_builtins(vm);
    register_array_builtins(vm);
    register_console(vm);
    register_math(vm);
    register_json(vm);
    register_global_helpers(vm);
    register_promise(vm);
    register_parallel_and_typed_arrays(vm);
    register_phase3_builtins(vm);
    register_bigint(vm);
    register_reflect(vm);
    register_proxy(vm);
    register_test262_harness(vm);
    crate::web_apis::register_web_apis(vm);

    // Ensure all built-in properties on global_object are reflected and non-enumerable
    for (name, val) in &vm.globals.clone() {
        if vm.global_object.is_object() {
            vm.set_property(vm.global_object, name, *val);
            vm.non_enumerable_props.insert((vm.global_object, name.clone()));
            if name == "undefined" || name == "NaN" || name == "Infinity" {
                vm.non_configurable_props.insert((vm.global_object, name.clone()));
                vm.non_writable_props.insert((vm.global_object, name.clone()));
            }
        }
    }

    // Ensure all built-in prototype methods and constructor properties are non-enumerable
    let mut builtin_targets = Vec::new();
    for opt_proto in [
        vm.object_proto,
        vm.array_proto,
        vm.function_proto,
        vm.regexp_proto,
        vm.string_proto,
        vm.number_proto,
        vm.boolean_proto,
        vm.symbol_proto,
        vm.promise_proto,
    ] {
        if let Some(p) = opt_proto {
            builtin_targets.push(p);
        }
    }

    let global_values: Vec<JsValue> = vm.globals.values().copied().collect();
    for global_val in global_values {
        if global_val.is_object() {
            builtin_targets.push(global_val);
            let proto_val = vm.get_property(global_val, "prototype");
            if proto_val.is_object() {
                builtin_targets.push(proto_val);
            }
        }
    }

    for target in builtin_targets {
        if let Some(ptr) = target.as_object_ptr() {
            let obj = unsafe { &*(ptr as *mut JsObject) };
            for prop in vm.shapes.all_properties(obj.shape_id) {
                vm.non_enumerable_props.insert((target, prop));
            }
            if let Some(accs) = &obj.accessors {
                for prop in accs.keys() {
                    vm.non_enumerable_props.insert((target, prop.clone()));
                }
            }
        }
    }
}

