//! The `JsObject` record and its constructors/accessors.

use super::*;

#[derive(Debug, Clone)]
pub struct JsObject {
    pub shape_id: ShapeId,
    pub prototype: Option<JsValue>,
    pub slots: Vec<JsValue>,
    pub kind: ObjectKind,
    pub accessors: Option<Box<std::collections::HashMap<String, Accessor>>>,
    pub symbol_properties: Option<Box<std::collections::HashMap<JsValue, JsValue>>>,
    pub symbol_accessors: Option<Box<std::collections::HashMap<JsValue, Accessor>>>,
    pub symbol_order: Option<Vec<JsValue>>,
}

impl JsObject {
    pub fn new_with_kind(shape_id: ShapeId, prototype: Option<JsValue>, kind: ObjectKind) -> Self {
        Self {
            shape_id,
            prototype,
            slots: Vec::new(),
            kind,
            accessors: None,
            symbol_properties: None,
            symbol_accessors: None,
            symbol_order: None,
        }
    }

    pub fn insert_symbol_property(&mut self, sym: JsValue, val: JsValue) {
        let syms = self.symbol_properties.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        syms.insert(sym, val);
        let order = self.symbol_order.get_or_insert_with(Vec::new);
        if !order.contains(&sym) {
            order.push(sym);
        }
    }

    pub fn insert_symbol_accessor_getter(&mut self, sym: JsValue, getter: JsValue) {
        let accs = self.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        accs.entry(sym).or_insert(Accessor { getter: None, setter: None }).getter = Some(getter);
        let order = self.symbol_order.get_or_insert_with(Vec::new);
        if !order.contains(&sym) {
            order.push(sym);
        }
    }

    pub fn insert_symbol_accessor_setter(&mut self, sym: JsValue, setter: JsValue) {
        let accs = self.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        accs.entry(sym).or_insert(Accessor { getter: None, setter: None }).setter = Some(setter);
        let order = self.symbol_order.get_or_insert_with(Vec::new);
        if !order.contains(&sym) {
            order.push(sym);
        }
    }

    pub fn new_ordinary(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Ordinary)
    }

    pub fn new_array(shape_id: ShapeId, prototype: Option<JsValue>, elements: Vec<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Array(elements))
    }

    pub fn new_function(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        func: JsFunction,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Function(func))
    }

    pub fn new_native_function(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        native: NativeFunction,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::NativeFunction(native))
    }

    pub fn new_string(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        content: String,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::String(content))
    }

    pub fn new_proxy(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        target: JsValue,
        handler: JsValue,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Proxy { target, handler })
    }

    pub fn new_string_object(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        content: String,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::StringObject(content))
    }

    pub fn new_number_object(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        value: f64,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::NumberObject(value))
    }

    pub fn new_boolean_object(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        value: bool,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::BooleanObject(value))
    }

    pub fn new_error(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        name: String,
        message: String,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Error { name, message })
    }

    pub fn new_promise(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        state: PromiseState,
        result: JsValue,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Promise(JsPromise {
            state,
            result,
            fulfill_reactions: Vec::new(),
            reject_reactions: Vec::new(),
        }))
    }

    pub fn new_array_buffer(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        ab: Arc<doms_parallel::SharedArrayBuffer>,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::ArrayBuffer(ab))
    }

    pub fn new_data_view(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        buffer: Arc<doms_parallel::SharedArrayBuffer>,
        byte_offset: usize,
        byte_length: usize,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::DataView { buffer, byte_offset, byte_length })
    }

    pub fn new_shared_array_buffer(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        sab: Arc<doms_parallel::SharedArrayBuffer>,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::SharedArrayBuffer(sab))
    }

    pub fn new_typed_array(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        typed_array: JsTypedArray,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::TypedArray(typed_array))
    }

    pub fn new_generator(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        generator: JsGenerator,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Generator(generator))
    }

    pub fn new_worker(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        port: Arc<std::sync::Mutex<doms_parallel::MessagePort>>,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Worker(port))
    }

    pub fn new_map(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Map(Vec::new()))
    }

    pub fn new_set(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Set(Vec::new()))
    }

    pub fn new_weak_map(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::WeakMap(Vec::new()))
    }

    pub fn new_weak_set(shape_id: ShapeId, prototype: Option<JsValue>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::WeakSet(Vec::new()))
    }

    pub fn new_weak_ref(shape_id: ShapeId, prototype: Option<JsValue>, target: JsValue) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::WeakRef(target))
    }

    pub fn new_finalization_registry(shape_id: ShapeId, prototype: Option<JsValue>, callback: JsValue) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::FinalizationRegistry { callback, entries: Vec::new() })
    }

    pub fn new_regexp(shape_id: ShapeId, prototype: Option<JsValue>, pattern: String, flags: String) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::RegExp { pattern, flags })
    }

    pub fn new_date(shape_id: ShapeId, prototype: Option<JsValue>, time_ms: f64) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Date(time_ms))
    }

    pub fn new_symbol(shape_id: ShapeId, prototype: Option<JsValue>, desc: Option<String>) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::Symbol(desc))
    }

    pub fn new_bigint(shape_id: ShapeId, prototype: Option<JsValue>, val: BigInt) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::BigInt(val))
    }

    pub fn define_getter(&mut self, key: String, getter: JsValue) {
        let accs = self.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let acc = accs.entry(key).or_default();
        acc.getter = Some(getter);
    }

    pub fn define_setter(&mut self, key: String, setter: JsValue) {
        let accs = self.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let acc = accs.entry(key).or_default();
        acc.setter = Some(setter);
    }

    pub fn define_accessor(&mut self, key: String, getter: Option<JsValue>, setter: Option<JsValue>) {
        let accs = self.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let acc = accs.entry(key).or_default();
        if getter.is_some() {
            acc.getter = getter;
        }
        if setter.is_some() {
            acc.setter = setter;
        }
    }

    #[inline]
    pub fn get_slot(&self, offset: u32) -> Option<JsValue> {
        self.slots.get(offset as usize).copied()
    }

    #[inline]
    pub fn set_slot(&mut self, offset: u32, val: JsValue) {
        let idx = offset as usize;
        if idx >= self.slots.len() {
            self.slots.resize(idx + 1, JsValue::undefined());
        }
        self.slots[idx] = val;
    }
}
