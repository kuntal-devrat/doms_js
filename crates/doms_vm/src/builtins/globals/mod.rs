//! Global builtins: functions, `String`, `Number`, `Boolean`, `Error`, `eval` and `Date`.
//!
//! [`register_global_helpers`] is a thin orchestrator; each submodule owns one
//! global. The call order matters (it matches the order the globals were
//! registered historically) so keep it stable when adding new globals.

use super::*;

mod boolean;
mod date;
mod errors;
mod eval;
mod functions;
mod number;
mod string;

use boolean::register_boolean_global;
use date::register_date_global;
use errors::register_error_types;
use eval::register_eval;
use functions::register_global_functions;
use number::register_number_global;
use string::register_string_global;

pub(super) fn register_global_helpers(vm: &mut Vm) {
    register_global_functions(vm);
    register_string_global(vm);
    register_number_global(vm);
    register_boolean_global(vm);
    register_error_types(vm);
    register_eval(vm);
    register_date_global(vm);
}
