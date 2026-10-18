//! ECMAScript keyed and weak collections: `Map`, `Set`, `WeakMap`, `WeakSet`, `WeakRef`, and `FinalizationRegistry`.

use super::*;

mod map;
mod set;
mod weak;

use map::register_map_builtins;
use set::register_set_builtins;
use weak::register_weak_builtins;

pub(super) fn register_collections(vm: &mut Vm) {
    register_map_builtins(vm);
    register_set_builtins(vm);
    register_weak_builtins(vm);
}
