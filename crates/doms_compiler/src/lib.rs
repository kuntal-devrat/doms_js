//! # DOMS Compiler
//!
//! Bytecode compiler for the DOMS JavaScript engine.

pub mod bytecode;
pub mod compiler;
pub mod scope;
pub mod source_map;

pub use bytecode::{BytecodeChunk, Constant, FeedbackSlot, FeedbackVector, Label, OpCode, TypeMask};
pub use compiler::{CompileError, Compiler};
pub use scope::{Local, SymbolTable, UpvalueInfo};
pub use source_map::{decode_vlq, Mapping, OriginalPosition, SourceMap};

use doms_parser::ast::Program;

/// Compile an AST [`Program`] into a [`BytecodeChunk`].
pub fn compile(program: &Program) -> Result<BytecodeChunk, CompileError> {
    Compiler::new().compile(program)
}

/// Compile an AST [`Program`] into a [`BytecodeChunk`] in strict mode.
pub fn compile_strict(program: &Program) -> Result<BytecodeChunk, CompileError> {
    let mut compiler = Compiler::new();
    compiler.is_strict = true;
    compiler.compile(program)
}

/// Compile an AST [`Program`] for eval execution.
pub fn compile_eval(program: &Program, is_strict: bool) -> Result<BytecodeChunk, CompileError> {
    let mut compiler = Compiler::new();
    compiler.is_strict = is_strict;
    compiler.is_eval = true;
    compiler.compile(program)
}
