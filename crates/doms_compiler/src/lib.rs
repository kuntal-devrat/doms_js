//! # DOMS Compiler
//!
//! Bytecode compiler for the DOMS JavaScript engine.

#![allow(clippy::all)]

pub mod bytecode;
pub mod compiler;
pub mod scope;
pub mod source_map;

pub use bytecode::{
    BytecodeChunk, Constant, FeedbackSlot, FeedbackVector, Label, OpCode, TypeMask,
};
pub use compiler::{CompileError, Compiler};
pub use scope::{Local, SymbolTable, UpvalueInfo};
pub use source_map::{Mapping, OriginalPosition, SourceMap, decode_vlq};

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
#[cfg(test)]
mod tests {
    use super::*;
    use doms_parser::parse;

    #[test]
    fn test_compile_strict_success() {
        let program = parse("let x = 10;").unwrap();
        let chunk = compile_strict(&program).unwrap();
        assert!(chunk.instructions.len() > 0);
    }

    #[test]
    fn test_compile_strict_error() {
        let program = parse("with (obj) { }").unwrap();
        let err = compile_strict(&program).unwrap_err();
        match err {
            CompileError::General(msg) => {
                assert!(msg.contains("Strict mode code may not include a with statement"));
            }
            _ => panic!("Expected General error for strict mode with statement"),
        }
    }

    #[test]
    fn test_compile_success() {
        let program = parse("let y = 20;").unwrap();
        let chunk = compile(&program).unwrap();
        assert!(chunk.instructions.len() > 0);
    }

    #[test]
    fn test_compile_eval_success() {
        let program = parse("let z = 30;").unwrap();

        let chunk_strict = compile_eval(&program, true).unwrap();
        assert!(chunk_strict.instructions.len() > 0);

        let chunk_non_strict = compile_eval(&program, false).unwrap();
        assert!(chunk_non_strict.instructions.len() > 0);
    }

    #[test]
    fn test_compile_basic_program() {
        let program = parse("let x = 1;").unwrap();
        let chunk = compile(&program).unwrap();
        assert!(!chunk.instructions.is_empty());
        assert!(!chunk.constants.is_empty());
    }
}
