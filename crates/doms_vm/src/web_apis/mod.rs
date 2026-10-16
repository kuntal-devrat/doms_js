//! Web Standards Host APIs (Timers, TextEncoder/Decoder, URL, Fetch, EventTarget).

use crate::interpreter::{Vm, VmError, TimerEntry};
use crate::object::{JsObject, ObjectKind, TypedArrayKind, JsTypedArray, PromiseState};
use crate::value::JsValue;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod encoding;
mod events;
mod fetch;
mod timers;
mod url;

#[cfg(test)]
mod tests;

use encoding::register_encoding;
use events::register_events;
use fetch::register_fetch;
use timers::register_timers;
use url::register_url;

pub fn register_web_apis(vm: &mut Vm) {
    register_timers(vm);
    register_encoding(vm);
    register_url(vm);
    register_events(vm);
    register_fetch(vm);
}
