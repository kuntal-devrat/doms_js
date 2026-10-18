//! Core interpreter types: [`VmError`], call frames and value wrappers.

use super::*;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum VmError {
    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Undefined global variable: {0}")]
    UndefinedGlobal(String),
    #[error("Syntax error: {0}")]
    SyntaxError(String),
    #[error("Reference error: {0}")]
    ReferenceError(String),
    #[error("Type error: {0}")]
    TypeError(String),
    #[error("Range error: {0}")]
    RangeError(String),
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    #[error("Invalid instruction at ip: {0}")]
    InvalidInstruction(usize),
    #[error("URI error: {0}")]
    UriError(String),
    #[error("Thrown JavaScript exception")]
    Thrown(JsValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferredType {
    Default,
    Number,
    String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyKeyVal {
    String(String),
    Symbol(JsValue),
}

/// Call frame on the execution stack.
#[derive(Debug, Clone)]
pub struct CallFrame {
    pub chunk: Arc<BytecodeChunk>,
    pub ip: usize,
    pub base_sp: usize,
    pub this_val: JsValue,
    pub gen_val: Option<JsValue>,
    pub locals: Vec<JsValue>,
    pub local_names: Arc<Vec<String>>,
    pub lexical_local_names: Arc<Vec<String>>,
    pub scopes: Vec<JsValue>,
    pub upvalues_count: usize,
    pub env_obj: Option<JsValue>,
    pub arguments: Option<JsValue>,
    pub raw_args: Option<Vec<JsValue>>,
    pub is_strict: bool,
    pub new_target: Option<JsValue>,
    pub is_function: bool,
    pub is_method: bool,
    pub is_arrow: bool,
    pub has_param_exprs: bool,
    pub in_param_init: bool,
    pub is_indirect_eval: bool,
    pub callee_func: Option<JsFunction>,
    pub callee_val: Option<JsValue>,
    pub mapped_arguments: Option<Vec<bool>>,
    pub home_object: Option<JsValue>,
}

/// Compatibility enum for earlier Phase 0 API callers.
#[derive(Debug, Clone, PartialEq)]
pub enum VmValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
}

impl fmt::Display for VmValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmValue::Undefined => write!(f, "undefined"),
            VmValue::Null => write!(f, "null"),
            VmValue::Boolean(b) => write!(f, "{}", b),
            VmValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e16 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            VmValue::String(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CatchHandler {
    pub frame_idx: usize,
    pub target_ip: usize,
    pub stack_len: usize,
    pub scopes_len: usize,
    pub is_finally: bool,
    pub finally_start: usize,
    pub kind_slot: u16,
}

#[derive(Clone)]
pub struct TimerEntry {
    pub id: u32,
    pub callback: JsValue,
    pub args: Vec<JsValue>,
    pub due_time: Instant,
    pub interval: Option<Duration>,
}
