//! # DOMS Compiler
//!
//! Bytecode compiler for the DOMS JavaScript engine.

pub mod bytecode;
pub mod compiler;
pub mod scope;

pub use bytecode::{BytecodeChunk, Constant, OpCode};
pub use compiler::{CompileError, Compiler};
pub use scope::{Local, SymbolTable};

use doms_parser::ast::Program;

/// Compile an AST [`Program`] into a [`BytecodeChunk`].
pub fn compile(program: &Program) -> Result<BytecodeChunk, CompileError> {
    Compiler::new().compile(program)
}
