//! Web Standards Host APIs (Timers, TextEncoder/Decoder, URL, Fetch, EventTarget, Crypto, Streams).

use crate::interpreter::{TimerEntry, Vm, VmError};
use crate::object::{JsObject, JsTypedArray, ObjectKind, PromiseState, TypedArrayKind};
use crate::value::JsValue;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod crypto;
mod encoding;
mod events;
mod fetch;
mod streams;
mod timers;
mod url;

#[cfg(test)]
mod tests;

use crypto::register_crypto;
use encoding::register_encoding;
use events::register_events;
use fetch::register_fetch;
use streams::register_streams;
use timers::register_timers;
use url::register_url;

pub fn register_web_apis(vm: &mut Vm) {
    register_timers(vm);
    register_encoding(vm);
    register_url(vm);
    register_events(vm);
    register_crypto(vm);
    register_streams(vm);
    register_fetch(vm);
}
