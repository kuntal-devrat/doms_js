//! Bytecode instruction set and chunk representation.

use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub chunk: BytecodeChunk,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Number(f64),
    String(String),
    Boolean(bool),
    Function(Arc<FunctionDef>),
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

    // Objects & Arrays
    CreateObject,
    CreateArray(u16),
    GetProp(u16),
    SetProp(u16),
    GetElem,
    SetElem,

    // Functions, Calls & Construction
    CreateFunction(u16),
    CreateAsyncFunction(u16),
    CreateGenerator(u16),
    Call(u8), // argc
    CallWithThis(u8), // argc with explicit this_val on stack
    Construct(u8), // new Ctor(args) with argc
    Return,
    Await,
    Yield,

    // Modules
    Export(u16),
    ImportModule(u16),
    DynamicImport,

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,

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

    // Bitwise
    BitAnd,
    BitOr,
    BitXor,

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
    SetGlobal(u16),

    // Stack Management
    Pop,
    Dup,
    Halt,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BytecodeChunk {
    pub instructions: Vec<OpCode>,
    pub constants: Vec<Constant>,
    pub lines: Vec<usize>,
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
        for (i, existing) in self.constants.iter().enumerate() {
            if existing == &val {
                return i as u16;
            }
        }
        let idx = self.constants.len() as u16;
        self.constants.push(val);
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
