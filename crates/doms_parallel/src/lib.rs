//! # DOMS Parallel
//!
//! Concurrency primitives, SharedArrayBuffer, Atomics, and scheduler for DOMS.

pub mod atomics;
pub mod memory_cage;
pub mod message_channel;
pub mod scheduler;
pub mod shared_array_buffer;

pub use atomics::{Atomics, AtomicsError};
pub use memory_cage::{CagedAllocation, CagedBuffer, MemoryCage, DEFAULT_CAGE_SIZE};
pub use message_channel::{ChannelError, MessageChannel, MessageData, MessagePort};
pub use scheduler::Scheduler;
pub use shared_array_buffer::SharedArrayBuffer;
