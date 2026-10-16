//! Isolate message-passing FFI.

use super::*;

pub type doms_message_handler = unsafe extern "C" fn(
    isolate: *mut doms_isolate_t,
    data: *const u8,
    len: usize,
    userdata: *mut std::ffi::c_void,
);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_isolate_post_message(
    target: *mut doms_isolate_t,
    data: *const u8,
    _len: usize,
) -> doms_status_t {
    if target.is_null() || data.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    doms_status_t::Ok
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn doms_isolate_set_message_handler(
    isolate: *mut doms_isolate_t,
    _handler: doms_message_handler,
    _userdata: *mut std::ffi::c_void,
) -> doms_status_t {
    if isolate.is_null() {
        return doms_status_t::ErrorInvalidArg;
    }
    doms_status_t::Ok
}
