//! The [`Vm`] struct and its lifecycle.

use super::*;

pub struct Vm {
    pub stack: Vec<JsValue>,
    pub frames: Vec<CallFrame>,
    pub globals: HashMap<String, JsValue>,
    pub heap: Heap,
    pub shapes: ShapeRegistry,
    pub inline_caches: Vec<InlineCacheSlot>,
    pub chunk_ics: HashMap<u64, Vec<InlineCacheSlot>>,
    pub objects: Vec<*mut JsObject>,
    pub catch_stack: Vec<CatchHandler>,
    pub microtasks: VecDeque<(JsValue, Vec<JsValue>)>,
    pub modules: HashMap<String, JsValue>,
    pub current_exports: HashMap<String, JsValue>,
    pub timers: BTreeMap<u32, TimerEntry>,
    pub next_timer_id: u32,
    pub instruction_count: usize,
    pub max_instructions: Option<usize>,
    pub deadline: Option<Instant>,
    pub max_objects: Option<usize>,
    pub eval_count: usize,
    pub max_evals: Option<usize>,
    pub oom: bool,
    pub gc_threshold: usize,
    pub temp_roots: Vec<JsValue>,
    pub pending_alloc: Option<*mut JsObject>,
    pub object_proto: Option<JsValue>,
    pub array_proto: Option<JsValue>,
    pub function_proto: Option<JsValue>,
    pub regexp_proto: Option<JsValue>,
    pub string_proto: Option<JsValue>,
    pub number_proto: Option<JsValue>,
    pub boolean_proto: Option<JsValue>,
    pub symbol_proto: Option<JsValue>,
    pub promise_proto: Option<JsValue>,
    pub date_proto: Option<JsValue>,
    pub iterator_proto: Option<JsValue>,
    pub array_iterator_proto: Option<JsValue>,
    pub map_iterator_proto: Option<JsValue>,
    pub set_iterator_proto: Option<JsValue>,
    pub string_iterator_proto: Option<JsValue>,
    pub generator_proto: Option<JsValue>,
    pub wrap_for_valid_iterator_proto: Option<JsValue>,
    pub iterator_helper_proto: Option<JsValue>,
    pub current_new_target: Option<JsValue>,
    pub global_object: JsValue,
    pub global_lexicals: HashMap<String, JsValue>,
    pub const_bindings: HashSet<String>,
    pub non_configurable_props: HashSet<(JsValue, String)>,
    pub non_writable_props: HashSet<(JsValue, String)>,
    pub non_enumerable_props: HashSet<(JsValue, String)>,
    pub non_extensible_objects: HashSet<JsValue>,
    pub var_names: HashSet<String>,
    pub script_var_names: HashSet<String>,
    pub in_eval: bool,
    pub in_direct_eval: bool,
    pub is_strict: bool,
    pub is_constructing: bool,
    pub symbol_registry: HashMap<String, JsValue>,
    pub symbol_iterator: Option<JsValue>,
    pub symbol_async_iterator: Option<JsValue>,
    pub symbol_to_string_tag: Option<JsValue>,
    pub symbol_is_concat_spreadable: Option<JsValue>,
    pub symbol_species: Option<JsValue>,
    pub symbol_to_primitive: Option<JsValue>,
    pub symbol_has_instance: Option<JsValue>,
    pub symbol_match: Option<JsValue>,
    pub symbol_match_all: Option<JsValue>,
    pub symbol_replace: Option<JsValue>,
    pub symbol_search: Option<JsValue>,
    pub symbol_split: Option<JsValue>,
    pub symbol_unscopables: Option<JsValue>,
    pub deleted_props: HashSet<(JsValue, String)>,
    pub is_module: bool,
    pub base_dir: Option<std::path::PathBuf>,
    pub module_namespace_objects: HashSet<JsValue>,
    pub current_module_obj: Option<JsValue>,
    pub imported_bindings: HashMap<String, (JsValue, String)>,
    pub star_exports: HashMap<String, JsValue>,
    pub module_reexports: HashMap<(JsValue, String), (JsValue, String)>,
    pub in_import_module: bool,
    pub loop_hotness: HashMap<(u64, usize), u32>,
    pub osr_cache: HashMap<(u64, usize), Option<crate::jit::JitFunction>>,
    pub regexp_last_match: RegExpLastMatch,
}

#[derive(Debug, Clone, Default)]
pub struct RegExpLastMatch {
    pub input: String,
    pub last_match: String,
    pub last_paren: String,
    pub left_context: String,
    pub right_context: String,
    pub parens: [String; 9],
}

impl Vm {
    pub fn new() -> Self {
        let mut vm = Self {
            stack: Vec::with_capacity(4096),
            frames: Vec::with_capacity(256),
            globals: HashMap::new(),
            heap: Heap::with_defaults(),
            shapes: ShapeRegistry::new(),
            inline_caches: Vec::new(),
            chunk_ics: HashMap::new(),
            objects: Vec::with_capacity(128),
            catch_stack: Vec::new(),
            microtasks: VecDeque::new(),
            modules: HashMap::new(),
            current_exports: HashMap::new(),
            timers: BTreeMap::new(),
            next_timer_id: 1,
            instruction_count: 0,
            max_instructions: None,
            deadline: None,
            max_objects: None,
            eval_count: 0,
            max_evals: None,
            oom: false,
            gc_threshold: 4096,
            temp_roots: Vec::new(),
            pending_alloc: None,
            object_proto: None,
            array_proto: None,
            function_proto: None,
            regexp_proto: None,
            string_proto: None,
            number_proto: None,
            boolean_proto: None,
            symbol_proto: None,
            promise_proto: None,
            date_proto: None,
            iterator_proto: None,
            array_iterator_proto: None,
            map_iterator_proto: None,
            set_iterator_proto: None,
            string_iterator_proto: None,
            generator_proto: None,
            wrap_for_valid_iterator_proto: None,
            iterator_helper_proto: None,
            current_new_target: None,
            global_object: JsValue::undefined(),
            global_lexicals: HashMap::new(),
            const_bindings: HashSet::new(),
            non_configurable_props: HashSet::new(),
            non_writable_props: HashSet::new(),
            non_enumerable_props: HashSet::new(),
            non_extensible_objects: HashSet::new(),
            var_names: HashSet::new(),
            script_var_names: HashSet::new(),
            in_eval: false,
            in_direct_eval: false,
            is_strict: false,
            is_constructing: false,
            symbol_registry: HashMap::new(),
            symbol_iterator: None,
            symbol_async_iterator: None,
            symbol_to_string_tag: None,
            symbol_is_concat_spreadable: None,
            symbol_species: None,
            symbol_to_primitive: None,
            symbol_has_instance: None,
            symbol_match: None,
            symbol_match_all: None,
            symbol_replace: None,
            symbol_search: None,
            symbol_split: None,
            symbol_unscopables: None,
            deleted_props: HashSet::new(),
            is_module: false,
            base_dir: None,
            module_namespace_objects: HashSet::new(),
            current_module_obj: None,
            imported_bindings: HashMap::new(),
            star_exports: HashMap::new(),
            module_reexports: HashMap::new(),
            in_import_module: false,
            loop_hotness: HashMap::new(),
            osr_cache: HashMap::new(),
            regexp_last_match: RegExpLastMatch::default(),
        };
        let global_obj = vm.alloc_object_old_gen();
        vm.global_object = global_obj;
        vm.globals.insert("globalThis".to_string(), global_obj);
        register_builtins(&mut vm);
        vm
    }

    pub fn push(&mut self, val: JsValue) {
        self.stack.push(val);
    }

    pub fn pop(&mut self) -> Result<JsValue, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    pub fn peek(&self) -> Result<&JsValue, VmError> {
        self.stack.last().ok_or(VmError::StackUnderflow)
    }

}

impl Drop for Vm {
    fn drop(&mut self) {
        let mut seen = std::collections::HashSet::new();
        for &ptr in &self.objects {
            if !ptr.is_null() && seen.insert(ptr) {
                unsafe {
                    std::ptr::drop_in_place(ptr);
                    if !self.heap.contains(ptr as *const u8) && !self.heap.nursery.to_space_contains(ptr as *const u8) {
                        drop(Box::from_raw(ptr));
                    }
                }
            }
        }
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}
