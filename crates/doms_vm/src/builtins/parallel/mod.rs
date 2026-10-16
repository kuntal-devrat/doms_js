//! Parallelism builtins: buffers, `DataView`, `Atomics` and `Worker`.

use super::*;

mod atomics;
mod buffers;
mod dataview;
mod worker;

use atomics::register_atomics;
use buffers::register_buffers;
use dataview::register_dataview;
use typed_array::register_typed_arrays;
use worker::register_worker;

pub(super) fn register_parallel_and_typed_arrays(vm: &mut Vm) {
    register_buffers(vm);
    register_dataview(vm);
    register_typed_arrays(vm);
    register_atomics(vm);
    register_worker(vm);
}
