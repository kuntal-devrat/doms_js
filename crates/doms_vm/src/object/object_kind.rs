//! `ObjectKind` variants and property accessors.

use super::*;

#[derive(Debug, Clone)]
pub enum ObjectKind {
    Ordinary,
    Array(Vec<JsValue>),
    Function(JsFunction),
    NativeFunction(NativeFunction),
    String(String),
    ConsString {
        left: JsValue,
        right: JsValue,
        length: usize,
    },
    Error {
        name: String,
        message: String,
    },
    Promise(JsPromise),
    Generator(JsGenerator),
    ArrayBuffer(Arc<doms_parallel::SharedArrayBuffer>),
    SharedArrayBuffer(Arc<doms_parallel::SharedArrayBuffer>),
    DataView {
        buffer_val: JsValue,
        buffer: Arc<doms_parallel::SharedArrayBuffer>,
        byte_offset: usize,
        byte_length: usize,
    },
    TypedArray(JsTypedArray),
    Worker(Arc<std::sync::Mutex<doms_parallel::MessagePort>>),
    Map(Vec<Option<(JsValue, JsValue)>>),
    Set(Vec<Option<JsValue>>),
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
    IteratorRecord {
        iterator: JsValue,
        next_method: JsValue,
        done: bool,
    },
    AsyncIteratorRecord {
        iterator: JsValue,
        next_method: JsValue,
        done: bool,
    },
    MapIterator {
        map: JsValue,
        index: usize,
        kind: MapIterationKind,
    },
    SetIterator {
        set: JsValue,
        index: usize,
        kind: SetIterationKind,
    },
    ArrayIterator {
        target: JsValue,
        index: usize,
        kind: ArrayIterationKind,
    },
    WrapForValidIterator {
        iterator: JsValue,
        next_method: JsValue,
        done: bool,
    },
    IteratorHelper {
        kind: Box<IteratorHelperKind>,
        iterator: JsValue,
        next_method: JsValue,
        done: bool,
        index: usize,
        executing: bool,
    },
    StringIterator {
        string: String,
        index: usize,
    },
}

#[derive(Debug, Clone)]
pub enum IteratorHelperKind {
    Map {
        mapper: JsValue,
    },
    Filter {
        predicate: JsValue,
    },
    Take {
        remaining: f64,
    },
    Drop {
        remaining: f64,
    },
    FlatMap {
        mapper: JsValue,
        inner_iter: Option<JsValue>,
        inner_next: Option<JsValue>,
    },
    Concat {
        iterables: Vec<JsValue>,
        current_idx: usize,
        current_iter: Option<JsValue>,
        current_next: Option<JsValue>,
    },
    Zip {
        others: Vec<JsValue>,
        mode: ZipMode,
        padding: Option<JsValue>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipMode {
    Shortest,
    Longest,
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayIterationKind {
    Keys,
    Values,
    Entries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapIterationKind {
    Key,
    Value,
    KeyValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetIterationKind {
    Value,
    KeyValue,
}

#[derive(Debug, Clone)]
pub struct Accessor {
    pub getter: Option<JsValue>,
    pub setter: Option<JsValue>,
    pub enumerable: bool,
    pub configurable: bool,
}

impl Accessor {
    pub fn new(getter: Option<JsValue>, setter: Option<JsValue>) -> Self {
        Self { getter, setter, enumerable: true, configurable: true }
    }

    pub fn with_getter(getter: JsValue) -> Self {
        Self { getter: Some(getter), setter: None, enumerable: true, configurable: true }
    }

    pub fn with_setter(setter: JsValue) -> Self {
        Self { getter: None, setter: Some(setter), enumerable: true, configurable: true }
    }
}

impl Default for Accessor {
    fn default() -> Self {
        Self { getter: None, setter: None, enumerable: true, configurable: true }
    }
}

#[derive(Debug, Clone)]
pub enum PrivateElement {
    Field(JsValue),
    Method(JsValue),
    Accessor(Accessor),
}
