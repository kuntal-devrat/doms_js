//! `ObjectKind` variants and property accessors.

use super::*;

#[derive(Debug, Clone)]
pub enum ObjectKind {
    Ordinary,
    Array(Vec<JsValue>),
    Function(JsFunction),
    NativeFunction(NativeFunction),
    String(String),
    Error { name: String, message: String },
    Promise(JsPromise),
    Generator(JsGenerator),
    ArrayBuffer(Arc<doms_parallel::SharedArrayBuffer>),
    SharedArrayBuffer(Arc<doms_parallel::SharedArrayBuffer>),
    DataView {
        buffer: Arc<doms_parallel::SharedArrayBuffer>,
        byte_offset: usize,
        byte_length: usize,
    },
    TypedArray(JsTypedArray),
    Worker(Arc<std::sync::Mutex<doms_parallel::MessagePort>>),
    Map(Vec<(JsValue, JsValue)>),
    Set(Vec<JsValue>),
    WeakMap(Vec<(JsValue, JsValue)>),
    WeakSet(Vec<JsValue>),
    WeakRef(JsValue),
    FinalizationRegistry {
        callback: JsValue,
        entries: Vec<(JsValue, JsValue, Option<JsValue>)>,
    },
    RegExp {
        pattern: String,
        flags: String,
    },
    Date(f64),
    Symbol(Option<String>),
    BigInt(BigInt),
    Proxy {
        target: JsValue,
        handler: JsValue,
    },
    StringObject(String),
    NumberObject(f64),
    BooleanObject(bool),
    BigIntObject(BigInt),
}

#[derive(Debug, Clone, Default)]
pub struct Accessor {
    pub getter: Option<JsValue>,
    pub setter: Option<JsValue>,
}
