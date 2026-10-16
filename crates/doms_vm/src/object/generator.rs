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
pub struct JsGenerator {
    pub function: JsFunction,
    pub ip: usize,
    pub stack: Vec<JsValue>,
    pub state: GeneratorState,
    pub this_val: JsValue,
    pub arguments: Option<JsValue>,
    pub mapped_arguments: Option<Vec<bool>>,
}
