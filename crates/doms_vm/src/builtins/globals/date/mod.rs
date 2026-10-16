//! The `Date` global object and calendar helpers.

use super::*;

mod accessors;
mod calendar;
mod constructor;
mod strings;

pub(super) fn register_date_global(vm: &mut Vm) {
    // Date
    let date_proto = vm.alloc_object();

    let mut date_methods = accessors::create_accessors(vm);
    date_methods.extend(strings::create_string_methods(vm));
    for (name, fn_val) in date_methods {
        vm.set_property(date_proto, name, fn_val);
        vm.non_enumerable_props.insert((date_proto, name.to_string()));
    }

    constructor::register_constructor_and_statics(vm, date_proto);
}
