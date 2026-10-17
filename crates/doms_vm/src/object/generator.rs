//! Generator state and the `JsGenerator` record.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorState {
    SuspendedStart,
    SuspendedYield,
    Executing,
    Completed,
}

#[derive(Debug, Clone)]
pub struct SavedCatchHandler {
    pub frame_offset: usize,
    pub target_ip: usize,
    pub stack_len_offset: usize,
    pub scopes_len: usize,
    pub is_finally: bool,
    pub finally_start: usize,
    pub kind_slot: u16,
}

#[derive(Debug, Clone)]
pub enum AsyncGenRequestKind {
    Next(JsValue),
    Return(JsValue),
    Throw(JsValue),
}

#[derive(Debug, Clone)]
pub struct AsyncGenRequest {
    pub kind: AsyncGenRequestKind,
    pub promise: JsValue,
}

#[derive(Debug, Clone)]
pub struct JsGenerator {
    pub function: JsFunction,
    pub ip: usize,
    pub stack: Vec<JsValue>,
    pub locals: Vec<JsValue>,
    pub scopes: Vec<JsValue>,
    pub env_obj: Option<JsValue>,
    pub catch_handlers: Vec<SavedCatchHandler>,
    pub state: GeneratorState,
    pub this_val: JsValue,
    pub arguments: Option<JsValue>,
    pub mapped_arguments: Option<Vec<bool>>,
    pub async_queue: Vec<AsyncGenRequest>,
}
