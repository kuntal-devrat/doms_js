//! Bytecode instruction set and chunk representation.

use std::sync::Arc;
use num_bigint::BigInt;

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub local_names: Vec<String>,
    pub lexical_local_names: Vec<String>,
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
    LoadTrue,
    LoadFalse,
    This,
    LoadSuper,

    // Objects & Arrays
    CreateObject,
    CreateArray(u16),
    ArrayPush,
    ArraySpreadPush,
    RequireObjectCoercible,
    CreateRegExp(u16, u16),
    GetKeys,
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
    PopCatch,

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ConstantKey {
    Number(u64),
    String(String),
    Boolean(bool),
    Function(usize),
    BigInt(BigInt),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BytecodeChunk {
    pub instructions: Vec<OpCode>,
    pub constants: Vec<Constant>,
    pub lines: Vec<usize>,
    pub top_lex_names: Vec<String>,
    pub top_var_names: Vec<String>,
    pub top_func_names: Vec<String>,
    pub imported_bindings: std::collections::HashSet<String>,
    pub export_names: Vec<String>,
    constant_indices: HashMap<ConstantKey, u16>,
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
            _ => panic!("Attempted to patch non-jump instruction"),
        }
    }
}
