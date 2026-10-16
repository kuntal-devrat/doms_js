//! Promise state and the `JsPromise` record.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromiseState {
    Pending,
    Fulfilled,
    Rejected,
}

#[derive(Debug, Clone)]
pub struct JsPromise {
    pub state: PromiseState,
    pub result: JsValue,
    pub fulfill_reactions: Vec<JsValue>,
    pub reject_reactions: Vec<JsValue>,
}
