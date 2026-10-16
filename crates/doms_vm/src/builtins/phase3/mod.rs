//! Phase 3 builtins: collections, `RegExp`, `Intl` and `Temporal` shims.

use super::*;

mod intl;
mod map;
mod regexp;
mod set;
mod temporal;
mod weak;

use intl::register_intl_builtins;
use map::register_map_builtins;
use regexp::register_regexp_builtins;
use set::register_set_builtins;
use temporal::register_temporal_builtins;
use weak::register_weak_builtins;

pub(super) fn register_phase3_builtins(vm: &mut Vm) {
    register_map_builtins(vm);
    register_set_builtins(vm);
    register_weak_builtins(vm);
    register_regexp_builtins(vm);
    register_intl_builtins(vm);
    register_temporal_builtins(vm);
}
