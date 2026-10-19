//! Bytecode instruction set and chunk representation.

use std::sync::Arc;
use num_bigint::BigInt;

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub self_slot: Option<u16>,
    pub local_names: Arc<Vec<String>>,
    pub lexical_local_names: Arc<Vec<String>>,
    pub chunk: BytecodeChunk,
    pub is_strict: bool,
    pub has_param_exprs: bool,
    pub is_generator: bool,
    pub is_arrow: bool,
    pub is_method: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Number(f64),
    String(String),
    Boolean(bool),
    Function(Arc<FunctionDef>),
    BigInt(BigInt),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    // Constant & Literal Loading
    LoadConst(u16),
    LoadNull,
    LoadUndefined,
    LoadHole,
    LoadTrue,
    LoadFalse,
    This,
    LoadSuper,
    SetThis,
    LoadUninitialized,

    // Objects & Arrays
    CreateObject,
    CreateArray(u16),
    ArrayPush,
    ArraySpreadPush,
    RequireObjectCoercible,
    CreateRegExp(u16, u16),
    GetKeys,
    GetIterator,
    IteratorNext(i32),
    IteratorClose(bool),
    GetAsyncIterator,
    AsyncIteratorNext(i32),
    AsyncIteratorClose(bool),
    GetProp(u16),
    SetProp(u16),
    DefineGetter(u16),
    DefineSetter(u16),
    DefineComputedGetter,
    DefineComputedSetter,
    DefineClassElement(u8),
    InheritClass,
    GetElem,
    SetElem,
    SetSuperProp(u16),
    SetSuperElem,
    ObjectSpread,
    ObjectRest(u16),
    DefinePrivateField(u16),
    DefinePrivateGetter(u16),
    DefinePrivateSetter(u16),

    // Functions, Calls & Construction
    CreateFunction(u16),
    CreateAsyncFunction(u16),
    CreateGenerator(u16),
    CreateAsyncGenerator(u16),
    GeneratorStart,
    Call(u8), // argc
    CallWithThis(u8), // argc with explicit this_val on stack
    CallDirectEval(u8),
    Construct(u8), // new Ctor(args) with argc
    CallSpread,
    CallWithThisSpread,
    ConstructSpread,
    GetNewTarget,
    Return,
    Await,
    Yield,
    SetFunctionName(u16),

    // Modules
    Export(u16),
    ExportAll,
    ImportModule(u16),
    DynamicImport,
    BindImport(u16, u16),
    ReExport(u16, u16),

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Exp,

    // Unary
    Pos,
    Neg,
    Not,
    BitNot,
    Typeof,
    Void,
    Delete,

    // Comparison & Equality
    Eq,
    NotEq,
    StrictEq,
    StrictNotEq,
    Lt,
    Lte,
    Gt,
    Gte,
    Instanceof,
    In,
    PrivateIn(u16),

    // Bitwise & Shift
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Ushr,

    // Control Flow
    Jump(i32),
    JumpIfFalse(i32),
    JumpIfTrue(i32),

    // Exceptions
    Throw,
    PushCatch(i32),
    PushFinally(i32, i32, u16),
    PopCatch,
    EndFinally(u16),

    // Variable Access
    GetLocal(u16),
    SetLocal(u16),
    GetGlobal(u16), // index into constants pool for identifier name
    GetGlobalSafe(u16),
    SetGlobal(u16),
    SetGlobalDirect(u16),
    DefGlobal(u16),
    DefLexical(u16),
    DefConst(u16),
    InitConst(u16),
    DeleteProp(u16),
    DeleteElem,
    DeleteIdent(u16),

    // Scopes
    PushScope,
    PopScope,
    PushCatchScope(u16),
    EnterFunctionBody,

    // Stack Management
    Pop,
    Dup,
    Halt,
}

use std::collections::HashMap;

/// Type feedback bitmask for speculative optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TypeMask(pub u8);

impl TypeMask {
    pub const NONE: u8 = 0;
    pub const INT32: u8 = 1 << 0;
    pub const DOUBLE: u8 = 1 << 1;
    pub const BOOLEAN: u8 = 1 << 2;
    pub const STRING: u8 = 1 << 3;
    pub const OBJECT: u8 = 1 << 4;
    pub const OTHER: u8 = 1 << 5;
    pub const NUMBER: u8 = Self::INT32 | Self::DOUBLE;

    #[inline]
    pub fn is_int32(self) -> bool {
        self.0 == Self::INT32
    }

    #[inline]
    pub fn is_number(self) -> bool {
        self.0 != 0 && (self.0 & !Self::NUMBER) == 0
    }
}

/// An inline type feedback slot for profiling ICs.
#[derive(Debug, Clone, PartialEq)]
pub enum FeedbackSlot {
    Uninitialized,
    Monomorphic { shape_id: u32, offset: u32 },
    Polymorphic { entries: Vec<(u32, u32)> },
    Megamorphic,
    BinaryOp { left: u8, right: u8 },
    ArrayLength,
    ArrayPush,
}

impl Default for FeedbackSlot {
    fn default() -> Self {
        FeedbackSlot::Uninitialized
    }
}

/// A type feedback vector associated with a bytecode chunk.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FeedbackVector {
    pub slots: Vec<FeedbackSlot>,
}

impl FeedbackVector {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![FeedbackSlot::Uninitialized; capacity],
        }
    }

    pub fn record_shape(&mut self, slot_idx: usize, shape_id: u32, offset: u32) {
        if slot_idx >= self.slots.len() {
            self.slots.resize(slot_idx + 1, FeedbackSlot::Uninitialized);
        }
        match &mut self.slots[slot_idx] {
            FeedbackSlot::Uninitialized => {
                self.slots[slot_idx] = FeedbackSlot::Monomorphic { shape_id, offset };
            }
            FeedbackSlot::Monomorphic { shape_id: s, offset: o } => {
                if *s == shape_id {
                    *o = offset;
                } else {
                    let prev_s = *s;
                    let prev_o = *o;
                    self.slots[slot_idx] = FeedbackSlot::Polymorphic {
                        entries: vec![(prev_s, prev_o), (shape_id, offset)],
                    };
                }
            }
            FeedbackSlot::Polymorphic { entries } => {
                if let Some(entry) = entries.iter_mut().find(|(s, _)| *s == shape_id) {
                    entry.1 = offset;
                } else if entries.len() < 4 {
                    entries.push((shape_id, offset));
                } else {
                    self.slots[slot_idx] = FeedbackSlot::Megamorphic;
                }
            }
            FeedbackSlot::Megamorphic => {}
            FeedbackSlot::BinaryOp { .. } => {
                self.slots[slot_idx] = FeedbackSlot::Monomorphic { shape_id, offset };
            }
            FeedbackSlot::ArrayLength | FeedbackSlot::ArrayPush => {}
        }
    }

    pub fn record_binary_op(&mut self, slot_idx: usize, left_mask: u8, right_mask: u8) {
        if slot_idx >= self.slots.len() {
            self.slots.resize(slot_idx + 1, FeedbackSlot::Uninitialized);
        }
        match &mut self.slots[slot_idx] {
            FeedbackSlot::Uninitialized => {
                self.slots[slot_idx] = FeedbackSlot::BinaryOp { left: left_mask, right: right_mask };
            }
            FeedbackSlot::BinaryOp { left, right } => {
                *left |= left_mask;
                *right |= right_mask;
            }
            _ => {}
        }
    }

    pub fn record_array_length(&mut self, slot_idx: usize) {
        if slot_idx >= self.slots.len() {
            self.slots.resize(slot_idx + 1, FeedbackSlot::Uninitialized);
        }
        self.slots[slot_idx] = FeedbackSlot::ArrayLength;
    }

    pub fn record_array_push(&mut self, slot_idx: usize) {
        if slot_idx >= self.slots.len() {
            self.slots.resize(slot_idx + 1, FeedbackSlot::Uninitialized);
        }
        self.slots[slot_idx] = FeedbackSlot::ArrayPush;
    }

    pub fn get_slot(&self, slot_idx: usize) -> Option<&FeedbackSlot> {
        self.slots.get(slot_idx)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ConstantKey {
    Number(u64),
    String(String),
    Boolean(bool),
    Function(usize),
    BigInt(BigInt),
}

static CHUNK_ID_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct BytecodeChunk {
    pub id: u64,
    pub instructions: Vec<OpCode>,
    pub constants: Vec<Constant>,
    pub lines: Vec<usize>,
    pub top_lex_names: Vec<String>,
    pub top_var_names: Vec<String>,
    pub top_func_names: Vec<String>,
    pub imported_bindings: std::collections::HashSet<String>,
    pub export_names: Vec<String>,
    pub export_aliases: HashMap<String, Vec<String>>,
    pub local_names: Arc<Vec<String>>,
    pub lexical_local_names: Arc<Vec<String>>,
    pub feedback_vector: Arc<std::sync::RwLock<FeedbackVector>>,
    pub self_slot: Option<u16>,
    constant_indices: HashMap<ConstantKey, u16>,
}

impl PartialEq for BytecodeChunk {
    fn eq(&self, other: &Self) -> bool {
        self.instructions == other.instructions
            && self.constants == other.constants
            && self.lines == other.lines
            && self.top_lex_names == other.top_lex_names
            && self.top_var_names == other.top_var_names
            && self.top_func_names == other.top_func_names
            && self.imported_bindings == other.imported_bindings
            && self.export_names == other.export_names
            && self.export_aliases == other.export_aliases
            && self.local_names == other.local_names
            && self.lexical_local_names == other.lexical_local_names
            && self.self_slot == other.self_slot
    }
}

impl Default for BytecodeChunk {
    fn default() -> Self {
        Self {
            id: CHUNK_ID_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            instructions: Vec::new(),
            constants: Vec::new(),
            lines: Vec::new(),
            top_lex_names: Vec::new(),
            top_var_names: Vec::new(),
            top_func_names: Vec::new(),
            imported_bindings: std::collections::HashSet::new(),
            export_names: Vec::new(),
            export_aliases: HashMap::new(),
            local_names: Arc::new(Vec::new()),
            lexical_local_names: Arc::new(Vec::new()),
            feedback_vector: Arc::new(std::sync::RwLock::new(FeedbackVector::default())),
            self_slot: None,
            constant_indices: HashMap::new(),
        }
    }
}

impl BytecodeChunk {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit(&mut self, op: OpCode, line: usize) -> usize {
        let idx = self.instructions.len();
        self.instructions.push(op);
        self.lines.push(line);
        idx
    }

    #[inline]
    pub fn record_feedback_shape(&self, ip: usize, shape_id: u32, offset: u32) {
        if let Ok(mut fv) = self.feedback_vector.write() {
            fv.record_shape(ip, shape_id, offset);
        }
    }

    #[inline]
    pub fn record_feedback_binary_op(&self, ip: usize, left: u8, right: u8) {
        if let Ok(mut fv) = self.feedback_vector.write() {
            fv.record_binary_op(ip, left, right);
        }
    }

    #[inline]
    pub fn record_feedback_array_length(&self, ip: usize) {
        if let Ok(mut fv) = self.feedback_vector.write() {
            fv.record_array_length(ip);
        }
    }

    #[inline]
    pub fn record_feedback_array_push(&self, ip: usize) {
        if let Ok(mut fv) = self.feedback_vector.write() {
            fv.record_array_push(ip);
        }
    }

    #[inline]
    pub fn get_feedback_slot(&self, ip: usize) -> Option<FeedbackSlot> {
        self.feedback_vector.read().ok().and_then(|fv| fv.get_slot(ip).cloned())
    }

    pub fn add_constant(&mut self, val: Constant) -> u16 {
        let key = match &val {
            Constant::Number(n) => ConstantKey::Number(n.to_bits()),
            Constant::String(s) => ConstantKey::String(s.clone()),
            Constant::Boolean(b) => ConstantKey::Boolean(*b),
            Constant::Function(f) => ConstantKey::Function(Arc::as_ptr(f) as usize),
            Constant::BigInt(bi) => ConstantKey::BigInt(bi.clone()),
        };

        if let Some(&idx) = self.constant_indices.get(&key) {
            return idx;
        }

        let idx = self.constants.len() as u16;
        self.constants.push(val);
        self.constant_indices.insert(key, idx);
        idx
    }

    pub fn patch_jump(&mut self, index: usize, target: i32) {
        match &mut self.instructions[index] {
            OpCode::Jump(offset) => *offset = target,
            OpCode::JumpIfFalse(offset) => *offset = target,
            OpCode::JumpIfTrue(offset) => *offset = target,
            OpCode::PushCatch(offset) => *offset = target,
            OpCode::PushFinally(offset, _, _) => *offset = target,
            OpCode::IteratorNext(offset) => *offset = target,
            OpCode::AsyncIteratorNext(offset) => *offset = target,
            _ => panic!("Attempted to patch non-jump instruction"),
        }
    }

    pub fn patch_finally_start(&mut self, index: usize, start: i32) {
        if let OpCode::PushFinally(_, finally_start, _) = &mut self.instructions[index] {
            *finally_start = start;
        }
    }

    /// Emit a forward jump placeholder instruction and return a type-safe `Label` to be bound later.
    #[inline]
    pub fn forward_jump(&mut self, op_builder: impl FnOnce(i32) -> OpCode, line: usize) -> Label {
        let idx = self.emit(op_builder(0), line);
        Label(idx)
    }

    /// Bind a forward jump `Label` to the current instruction offset.
    #[inline]
    pub fn bind_label(&mut self, label: Label) {
        let target = self.instructions.len() as i32;
        self.patch_jump(label.0, target);
    }

    /// Bind a forward jump `Label` to a specific instruction target offset.
    #[inline]
    pub fn bind_label_to(&mut self, label: Label, target: i32) {
        self.patch_jump(label.0, target);
    }
}

/// Type-safe jump target label for control-flow patching in the compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Label(pub usize);

