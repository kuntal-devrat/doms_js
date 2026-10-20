//! # DOMS Parallel
//!
//! Concurrency primitives, SharedArrayBuffer, Atomics, and scheduler for DOMS.

#![allow(clippy::type_complexity)]

pub mod atomics;
pub mod memory_cage;
pub mod message_channel;
pub mod scheduler;
pub mod shared_array_buffer;

pub use atomics::{Atomics, AtomicsError};
pub use memory_cage::{CagedAllocation, CagedBuffer, DEFAULT_CAGE_SIZE, MemoryCage};
pub use message_channel::{ChannelError, MessageChannel, MessageData, MessagePort};
pub use scheduler::{Scheduler, TaskHandle, TaskPriority};
pub use shared_array_buffer::SharedArrayBuffer;
