//! Bytecode instruction set and chunk representation.

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Number(f64),
    String(String),
    Boolean(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    // Constant & Literal Loading
    LoadConst(u16),
    LoadNull,
    LoadUndefined,
    LoadTrue,
    LoadFalse,

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    // Unary
    Neg,
    Not,
    BitNot,
    Typeof,

    // Comparison & Equality
    Eq,
    NotEq,
    StrictEq,
    StrictNotEq,
    Lt,
    Lte,
    Gt,
    Gte,

    // Bitwise
    BitAnd,
    BitOr,
    BitXor,

    // Control Flow
    Jump(i32),
    JumpIfFalse(i32),
    JumpIfTrue(i32),

    // Variable Access
    GetLocal(u16),
    SetLocal(u16),
    GetGlobal(u16), // index into constants pool for identifier name
    SetGlobal(u16),

    // Functions & Calls
    Call(u8), // argc
    Return,

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
            _ => panic!("Attempted to patch non-jump instruction"),
        }
    }
}
