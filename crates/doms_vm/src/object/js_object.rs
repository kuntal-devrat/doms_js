//! The `JsObject` record and its constructors/accessors.

use super::*;

pub const IN_OBJECT_SLOTS: usize = 4;
pub const OBJECT_FLAG_EXTENSIBLE: u8 = 1 << 0;
pub const OBJECT_FLAG_ARRAY_LENGTH_NON_WRITABLE: u8 = 1 << 1;

#[derive(Debug, Clone)]
pub struct JsObject {
    pub shape_id: ShapeId,
    pub prototype: Option<JsValue>,
    pub in_object_slots: [JsValue; IN_OBJECT_SLOTS],
    pub extra_slots: Vec<JsValue>,
    pub kind: ObjectKind,
    pub flags: u8,
    pub accessors: Option<Box<std::collections::HashMap<String, Accessor>>>,
    pub accessor_order: Option<Vec<String>>,
    pub prop_order: Option<Vec<String>>,
    pub symbol_properties: Option<Box<std::collections::HashMap<JsValue, JsValue>>>,
    pub symbol_accessors: Option<Box<std::collections::HashMap<JsValue, Accessor>>>,
    pub symbol_attributes: Option<Box<std::collections::HashMap<JsValue, PropertyAttributes>>>,
    pub symbol_order: Option<Vec<JsValue>>,
    pub private_elements: Option<Box<std::collections::HashMap<String, PrivateElement>>>,
}

impl JsObject {
    pub fn new_with_kind(shape_id: ShapeId, prototype: Option<JsValue>, kind: ObjectKind) -> Self {
        Self {
            shape_id,
            prototype,
            in_object_slots: [JsValue::hole(); IN_OBJECT_SLOTS],
            extra_slots: Vec::new(),
            kind,
            flags: OBJECT_FLAG_EXTENSIBLE,
            accessors: None,
            accessor_order: None,
            prop_order: None,
            symbol_properties: None,
            symbol_accessors: None,
            symbol_attributes: None,
            symbol_order: None,
            private_elements: None,
        }
    }

    #[inline]
    pub fn is_extensible(&self) -> bool {
        (self.flags & OBJECT_FLAG_EXTENSIBLE) != 0
    }

    #[inline]
    pub fn prevent_extensions(&mut self) {
        self.flags &= !OBJECT_FLAG_EXTENSIBLE;
    }

    #[inline]
    pub fn is_array_length_writable(&self) -> bool {
        (self.flags & OBJECT_FLAG_ARRAY_LENGTH_NON_WRITABLE) == 0
    }

    #[inline]
    pub fn set_array_length_non_writable(&mut self) {
        self.flags |= OBJECT_FLAG_ARRAY_LENGTH_NON_WRITABLE;
    }

    #[inline]
    pub fn set_array_length_writable(&mut self) {
        self.flags &= !OBJECT_FLAG_ARRAY_LENGTH_NON_WRITABLE;
    }

    pub fn insert_private_element(&mut self, name: String, val: JsValue) {
        let privs = self.private_elements.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        privs.insert(name, PrivateElement::Value(val));
    }

    pub fn insert_private_getter(&mut self, name: String, getter: JsValue) {
        let privs = self.private_elements.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let entry = privs.entry(name).or_insert_with(|| PrivateElement::Accessor(Accessor::default()));
        match entry {
            PrivateElement::Accessor(acc) => acc.getter = Some(getter),
            PrivateElement::Value(_) => *entry = PrivateElement::Accessor(Accessor::with_getter(getter)),
        }
    }

    pub fn insert_private_setter(&mut self, name: String, setter: JsValue) {
        let privs = self.private_elements.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let entry = privs.entry(name).or_insert_with(|| PrivateElement::Accessor(Accessor::default()));
        match entry {
            PrivateElement::Accessor(acc) => acc.setter = Some(setter),
            PrivateElement::Value(_) => *entry = PrivateElement::Accessor(Accessor::with_setter(setter)),
        }
    }

    pub fn get_private_element(&self, name: &str) -> Option<&PrivateElement> {
        self.private_elements.as_ref().and_then(|p| p.get(name))
    }

    pub fn contains_private_element(&self, name: &str) -> bool {
        self.private_elements.as_ref().map_or(false, |p| p.contains_key(name))
    }

    pub fn insert_symbol_property(&mut self, sym: JsValue, val: JsValue) {
        let syms = self.symbol_properties.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        syms.insert(sym, val);
        let order = self.symbol_order.get_or_insert_with(Vec::new);
        if !order.contains(&sym) {
            order.push(sym);
        }
    }

    pub fn set_symbol_attributes(&mut self, sym: JsValue, attrs: PropertyAttributes) {
        let attrs_map = self.symbol_attributes.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        attrs_map.insert(sym, attrs);
    }

    pub fn insert_symbol_accessor_getter(&mut self, sym: JsValue, getter: JsValue) {
        let accs = self.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        accs.entry(sym).or_default().getter = Some(getter);
        let order = self.symbol_order.get_or_insert_with(Vec::new);
        if !order.contains(&sym) {
            order.push(sym);
        }
    }

    pub fn insert_symbol_accessor_setter(&mut self, sym: JsValue, setter: JsValue) {
        let accs = self.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        accs.entry(sym).or_default().setter = Some(setter);
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

    pub fn new_cons_string(
        shape_id: ShapeId,
        prototype: Option<JsValue>,
        left: JsValue,
        right: JsValue,
        length: usize,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::ConsString { left, right, length })
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
        buffer_val: JsValue,
        buffer: Arc<doms_parallel::SharedArrayBuffer>,
        byte_offset: usize,
        byte_length: usize,
    ) -> Self {
        Self::new_with_kind(shape_id, prototype, ObjectKind::DataView { buffer_val, buffer, byte_offset, byte_length })
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

    pub fn add_prop_order(&mut self, key: String) {
        let order = self.prop_order.get_or_insert_with(Vec::new);
        if !order.contains(&key) {
            order.push(key);
        }
    }

    pub fn remove_prop_order(&mut self, key: &str) {
        if let Some(order) = &mut self.prop_order {
            order.retain(|k| k != key);
        }
    }

    pub fn define_getter(&mut self, key: String, getter: JsValue) {
        let accs = self.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let acc = accs.entry(key.clone()).or_default();
        acc.getter = if getter.is_undefined() { None } else { Some(getter) };
        let order = self.accessor_order.get_or_insert_with(Vec::new);
        if !order.contains(&key) {
            order.push(key.clone());
        }
        self.add_prop_order(key);
    }

    pub fn define_setter(&mut self, key: String, setter: JsValue) {
        let accs = self.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let acc = accs.entry(key.clone()).or_default();
        acc.setter = if setter.is_undefined() { None } else { Some(setter) };
        let order = self.accessor_order.get_or_insert_with(Vec::new);
        if !order.contains(&key) {
            order.push(key.clone());
        }
        self.add_prop_order(key);
    }

    pub fn define_accessor(&mut self, key: String, getter: Option<JsValue>, setter: Option<JsValue>) {
        let accs = self.accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let acc = accs.entry(key.clone()).or_default();
        if getter.is_some() {
            acc.getter = getter;
        }
        if setter.is_some() {
            acc.setter = setter;
        }
        let order = self.accessor_order.get_or_insert_with(Vec::new);
        if !order.contains(&key) {
            order.push(key.clone());
        }
        self.add_prop_order(key);
    }

    pub fn define_symbol_accessor(&mut self, sym: JsValue, getter: Option<JsValue>, setter: Option<JsValue>) {
        let accs = self.symbol_accessors.get_or_insert_with(|| Box::new(std::collections::HashMap::new()));
        let acc = accs.entry(sym).or_default();
        if getter.is_some() {
            acc.getter = getter;
        }
        if setter.is_some() {
            acc.setter = setter;
        }
        let order = self.symbol_order.get_or_insert_with(Vec::new);
        if !order.contains(&sym) {
            order.push(sym);
        }
    }

    #[inline]
    pub fn get_slot(&self, offset: u32) -> Option<JsValue> {
        let idx = offset as usize;
        if idx < IN_OBJECT_SLOTS {
            let v = self.in_object_slots[idx];
            if !v.is_hole() {
                Some(v)
            } else {
                None
            }
        } else {
            self.extra_slots.get(idx - IN_OBJECT_SLOTS).copied()
        }
    }

    #[inline]
    pub fn set_slot(&mut self, offset: u32, val: JsValue) {
        let idx = offset as usize;
        if idx < IN_OBJECT_SLOTS {
            self.in_object_slots[idx] = val;
        } else {
            let extra_idx = idx - IN_OBJECT_SLOTS;
            if extra_idx >= self.extra_slots.len() {
                self.extra_slots.resize(extra_idx + 1, JsValue::undefined());
            }
            self.extra_slots[extra_idx] = val;
        }
    }
}
