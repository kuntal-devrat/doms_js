//! The `JsFunction` record.

use super::*;

#[derive(Debug, Clone)]
pub struct JsFunction {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub local_names: Vec<String>,
    pub lexical_local_names: Vec<String>,
    pub chunk: Arc<BytecodeChunk>,
    pub upvalues: Vec<JsValue>,
    pub is_async: bool,
    pub is_generator: bool,
    pub call_count: u32,
    pub jit_entry: Option<crate::jit::JitFunction>,
    pub is_strict: bool,
    pub has_param_exprs: bool,
    pub is_method: bool,
    pub is_arrow: bool,
    pub home_object: Option<JsValue>,
}
