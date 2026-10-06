//! Bytecode interpreter virtual machine.

use crate::inline_cache::InlineCacheSlot;
use doms_compiler::bytecode::{BytecodeChunk, Constant, OpCode};
use doms_gc::heap::Heap;
use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VmError {
    #[error("Stack underflow")]
    StackUnderflow,
    #[error("Undefined global variable: {0}")]
    UndefinedGlobal(String),
    #[error("Type error: {0}")]
    TypeError(String),
    #[error("Invalid instruction at ip: {0}")]
    InvalidInstruction(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum VmValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
}

impl VmValue {
    pub fn is_truthy(&self) -> bool {
        match self {
            VmValue::Undefined | VmValue::Null => false,
            VmValue::Boolean(b) => *b,
            VmValue::Number(n) => *n != 0.0 && !n.is_nan(),
            VmValue::String(s) => !s.is_empty(),
        }
    }

    pub fn type_of(&self) -> &'static str {
        match self {
            VmValue::Undefined => "undefined",
            VmValue::Null => "object", // JS spec quirk: typeof null === "object"
            VmValue::Boolean(_) => "boolean",
            VmValue::Number(_) => "number",
            VmValue::String(_) => "string",
        }
    }

    pub fn to_number(&self) -> f64 {
        match self {
            VmValue::Undefined => f64::NAN,
            VmValue::Null => 0.0,
            VmValue::Boolean(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            VmValue::Number(n) => *n,
            VmValue::String(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
        }
    }

    pub fn to_string_value(&self) -> String {
        match self {
            VmValue::Undefined => "undefined".into(),
            VmValue::Null => "null".into(),
            VmValue::Boolean(b) => {
                if *b {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            VmValue::Number(n) => {
                if n.is_nan() {
                    "NaN".into()
                } else if n.is_infinite() {
                    if *n > 0.0 { "Infinity".into() } else { "-Infinity".into() }
                } else if n.fract() == 0.0 && n.abs() < 1e16 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            VmValue::String(s) => s.clone(),
        }
    }
}

impl fmt::Display for VmValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_value())
    }
}

pub struct Vm {
    pub stack: Vec<VmValue>,
    pub globals: HashMap<String, VmValue>,
    pub heap: Heap,
    pub inline_caches: Vec<InlineCacheSlot>,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(256),
            globals: HashMap::new(),
            heap: Heap::with_defaults(),
            inline_caches: Vec::new(),
        }
    }

    pub fn push(&mut self, val: VmValue) {
        self.stack.push(val);
    }

    pub fn pop(&mut self) -> Result<VmValue, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    pub fn peek(&self) -> Result<&VmValue, VmError> {
        self.stack.last().ok_or(VmError::StackUnderflow)
    }

    pub fn run(&mut self, chunk: &BytecodeChunk) -> Result<VmValue, VmError> {
        let mut ip = 0;
        let instructions = &chunk.instructions;
        let constants = &chunk.constants;

        while ip < instructions.len() {
            let op = instructions[ip];
            ip += 1;

            match op {
                OpCode::LoadConst(idx) => {
                    let c = &constants[idx as usize];
                    let val = match c {
                        Constant::Number(n) => VmValue::Number(*n),
                        Constant::String(s) => VmValue::String(s.clone()),
                        Constant::Boolean(b) => VmValue::Boolean(*b),
                    };
                    self.push(val);
                }
                OpCode::LoadNull => self.push(VmValue::Null),
                OpCode::LoadUndefined => self.push(VmValue::Undefined),
                OpCode::LoadTrue => self.push(VmValue::Boolean(true)),
                OpCode::LoadFalse => self.push(VmValue::Boolean(false)),

                OpCode::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (VmValue::String(sa), _) => {
                            self.push(VmValue::String(format!("{}{}", sa, b.to_string_value())));
                        }
                        (_, VmValue::String(sb)) => {
                            self.push(VmValue::String(format!("{}{}", a.to_string_value(), sb)));
                        }
                        _ => {
                            self.push(VmValue::Number(a.to_number() + b.to_number()));
                        }
                    }
                }
                OpCode::Sub => {
                    let b = self.pop()?.to_number();
                    let a = self.pop()?.to_number();
                    self.push(VmValue::Number(a - b));
                }
                OpCode::Mul => {
                    let b = self.pop()?.to_number();
                    let a = self.pop()?.to_number();
                    self.push(VmValue::Number(a * b));
                }
                OpCode::Div => {
                    let b = self.pop()?.to_number();
                    let a = self.pop()?.to_number();
                    self.push(VmValue::Number(a / b));
                }
                OpCode::Mod => {
                    let b = self.pop()?.to_number();
                    let a = self.pop()?.to_number();
                    self.push(VmValue::Number(a % b));
                }

                OpCode::Neg => {
                    let n = self.pop()?.to_number();
                    self.push(VmValue::Number(-n));
                }
                OpCode::Not => {
                    let v = self.pop()?;
                    self.push(VmValue::Boolean(!v.is_truthy()));
                }
                OpCode::BitNot => {
                    let n = self.pop()?.to_number() as i32;
                    self.push(VmValue::Number((!n) as f64));
                }
                OpCode::Typeof => {
                    let v = self.pop()?;
                    self.push(VmValue::String(v.type_of().to_string()));
                }

                OpCode::BitAnd => {
                    let b = self.pop()?.to_number() as i32;
                    let a = self.pop()?.to_number() as i32;
                    self.push(VmValue::Number((a & b) as f64));
                }
                OpCode::BitOr => {
                    let b = self.pop()?.to_number() as i32;
                    let a = self.pop()?.to_number() as i32;
                    self.push(VmValue::Number((a | b) as f64));
                }
                OpCode::BitXor => {
                    let b = self.pop()?.to_number() as i32;
                    let a = self.pop()?.to_number() as i32;
                    self.push(VmValue::Number((a ^ b) as f64));
                }

                OpCode::Eq | OpCode::StrictEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(VmValue::Boolean(a == b));
                }
                OpCode::NotEq | OpCode::StrictNotEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(VmValue::Boolean(a != b));
                }
                OpCode::Lt => {
                    let b = self.pop()?.to_number();
                    let a = self.pop()?.to_number();
                    self.push(VmValue::Boolean(a < b));
                }
                OpCode::Lte => {
                    let b = self.pop()?.to_number();
                    let a = self.pop()?.to_number();
                    self.push(VmValue::Boolean(a <= b));
                }
                OpCode::Gt => {
                    let b = self.pop()?.to_number();
                    let a = self.pop()?.to_number();
                    self.push(VmValue::Boolean(a > b));
                }
                OpCode::Gte => {
                    let b = self.pop()?.to_number();
                    let a = self.pop()?.to_number();
                    self.push(VmValue::Boolean(a >= b));
                }

                OpCode::Jump(target) => {
                    ip = target as usize;
                }
                OpCode::JumpIfFalse(target) => {
                    let cond = self.peek()?;
                    if !cond.is_truthy() {
                        ip = target as usize;
                    }
                }
                OpCode::JumpIfTrue(target) => {
                    let cond = self.peek()?;
                    if cond.is_truthy() {
                        ip = target as usize;
                    }
                }

                OpCode::GetLocal(slot) => {
                    let val =
                        self.stack.get(slot as usize).cloned().ok_or(VmError::StackUnderflow)?;
                    self.push(val);
                }
                OpCode::SetLocal(slot) => {
                    let val = self.peek()?.clone();
                    if (slot as usize) < self.stack.len() {
                        self.stack[slot as usize] = val;
                    } else {
                        self.stack.push(val);
                    }
                }
                OpCode::GetGlobal(name_idx) => {
                    let name = match &constants[name_idx as usize] {
                        Constant::String(s) => s,
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    let val = self.globals.get(name).cloned().unwrap_or(VmValue::Undefined);
                    self.push(val);
                }
                OpCode::SetGlobal(name_idx) => {
                    let name = match &constants[name_idx as usize] {
                        Constant::String(s) => s.clone(),
                        _ => return Err(VmError::TypeError("Global name must be string".into())),
                    };
                    let val = self.peek()?.clone();
                    self.globals.insert(name, val);
                }

                OpCode::Call(_argc) => {
                    // Call frame handling in Phase 1
                }
                OpCode::Return => {
                    return self.pop();
                }

                OpCode::Pop => {
                    self.pop()?;
                }
                OpCode::Dup => {
                    let val = self.peek()?.clone();
                    self.push(val);
                }
                OpCode::Halt => {
                    return Ok(self.pop().unwrap_or(VmValue::Undefined));
                }
            }
        }

        Ok(self.pop().unwrap_or(VmValue::Undefined))
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_simple_bytecode() {
        let mut chunk = BytecodeChunk::new();
        let c1 = chunk.add_constant(Constant::Number(10.0));
        let c2 = chunk.add_constant(Constant::Number(32.0));
        chunk.emit(OpCode::LoadConst(c1), 1);
        chunk.emit(OpCode::LoadConst(c2), 1);
        chunk.emit(OpCode::Add, 1);
        chunk.emit(OpCode::Halt, 1);

        let mut vm = Vm::new();
        let res = vm.run(&chunk).unwrap();
        assert_eq!(res, VmValue::Number(42.0));
    }
}
