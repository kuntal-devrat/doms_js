//! AST to bytecode compiler.

pub mod classes;
pub mod expressions;
pub mod functions;
pub mod modules;
pub mod program;
pub mod scope_analysis;
pub mod statements;

use crate::bytecode::{BytecodeChunk, OpCode};
use crate::scope::SymbolTable;
use doms_parser::ast::Statement;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("Identifier '{0}' has already been declared")]
    DuplicateDeclaration(String),
    #[error("Variable '{0}' already declared in this scope")]
    DuplicateVariable(String),
    #[error("Too many constants in chunk")]
    TooManyConstants,
    #[error("Too many locals")]
    TooManyLocals,
    #[error("{0}")]
    General(String),
}

#[derive(Debug, Clone)]
pub struct TargetScope {
    pub labels: Vec<String>,
    pub jumps: Vec<usize>,
    pub scope_depth: usize,
    pub iter_slot: Option<u16>,
    pub is_async_iter: bool,
    pub break_depth: usize,
    pub catch_depth: usize,
}

#[derive(Debug, Clone)]
pub struct FinallyEntry {
    pub stmt: Statement,
    pub scope_depth: usize,
    pub break_depth: usize,
    pub catch_depth: usize,
}

pub struct Compiler {
    pub chunk: BytecodeChunk,
    pub symbols: SymbolTable,
    pub break_targets: Vec<TargetScope>,
    pub continue_targets: Vec<TargetScope>,
    pub current_labels: Vec<String>,
    pub finally_stack: Vec<FinallyEntry>,
    pub is_in_function: bool,
    pub internal_statements: bool,
    pub with_depth: usize,
    pub block_scope_depth: usize,
    pub catch_depth: usize,
    pub is_strict: bool,
    pub is_eval: bool,
    pub touched_bindings: std::collections::HashSet<String>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            chunk: BytecodeChunk::new(),
            symbols: SymbolTable::new(),
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
            current_labels: Vec::new(),
            finally_stack: Vec::new(),
            is_in_function: false,
            internal_statements: false,
            with_depth: 0,
            block_scope_depth: 0,
            catch_depth: 0,
            is_strict: false,
            is_eval: false,
            touched_bindings: std::collections::HashSet::new(),
        }
    }

    pub(crate) fn emit_push_scope(&mut self) {
        self.chunk.emit(OpCode::PushScope, 0);
        self.block_scope_depth += 1;
    }

    #[allow(dead_code)]
    pub(crate) fn emit_push_catch_scope(&mut self, name_idx: u16) {
        self.chunk.emit(OpCode::PushCatchScope(name_idx), 0);
        self.block_scope_depth += 1;
    }

    pub(crate) fn emit_pop_scope(&mut self) {
        self.chunk.emit(OpCode::PopScope, 0);
        self.block_scope_depth = self.block_scope_depth.saturating_sub(1);
    }

    pub fn is_anonymous_function_definition(expr: &doms_parser::ast::Expression) -> bool {
        use doms_parser::ast::Expression;
        match expr {
            Expression::ArrowExpr { .. } => true,
            Expression::FunctionExpr { name, .. } => name.is_none(),
            Expression::AsyncFunctionExpr { name, .. } => name.is_none(),
            Expression::GeneratorExpr { name, .. } => name.is_none(),
            Expression::AsyncGeneratorExpr { name, .. } => name.is_none(),
            Expression::ClassExpr(class_def) => class_def.name.is_none(),
            Expression::Grouping(inner) => Self::is_anonymous_function_definition(inner),
            _ => false,
        }
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doms_parser::parse;

    #[test]
    fn test_compile_arithmetic() {
        let prog = parse("let x = 1 + 2 * 3;").unwrap();
        let chunk = Compiler::new().compile(&prog).unwrap();
        assert!(chunk.instructions.contains(&OpCode::Add));
        assert!(chunk.instructions.contains(&OpCode::Mul));
    }

    #[test]
    fn test_compile_object_and_array() {
        let prog = parse("let obj = { a: 1 }; let arr = [10, 20];").unwrap();
        let chunk = Compiler::new().compile(&prog).unwrap();
        assert!(chunk.instructions.contains(&OpCode::CreateObject));
        assert!(chunk.instructions.contains(&OpCode::CreateArray(2)));
    }

    #[test]
    fn test_try_finally_continue() {
        let prog = parse("var c1=0, fin=0; while(c1<2){ try{ c1+=1; break; } catch(e){} finally{ fin=1; continue; } fin=-1; c1+=2; }").unwrap();
        let chunk = Compiler::new().compile(&prog).unwrap();
        assert!(!chunk.instructions.is_empty());
    }
}
