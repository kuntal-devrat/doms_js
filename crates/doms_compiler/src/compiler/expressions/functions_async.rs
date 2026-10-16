use super::super::Compiler;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;
use std::sync::Arc;

impl Compiler {
    pub(crate) fn compile_function_expr(
        &mut self,
        name: &Option<String>,
        params: &[String],
        body: &[Statement],
    ) -> Result<(), CompileError> {
        let def = Self::compile_function(name.clone(), params.to_vec(), body, self.is_strict)?;
        let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
        self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
        Ok(())
    }

    pub(crate) fn compile_arrow_expr(
        &mut self,
        params: &[String],
        body: &[Statement],
    ) -> Result<(), CompileError> {
        let def = Self::compile_function_full(None, params.to_vec(), body, self.is_strict, false, true, false)?;
        let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
        self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
        Ok(())
    }

    pub(crate) fn compile_method_expr(
        &mut self,
        name: &Option<String>,
        params: &[String],
        body: &[Statement],
    ) -> Result<(), CompileError> {
        let def = Self::compile_function_full(name.clone(), params.to_vec(), body, self.is_strict, false, false, true)?;
        let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
        self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
        Ok(())
    }

    pub(crate) fn compile_async_function_expr(
        &mut self,
        name: &Option<String>,
        params: &[String],
        body: &[Statement],
    ) -> Result<(), CompileError> {
        let def = Self::compile_function(name.clone(), params.to_vec(), body, self.is_strict)?;
        let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
        self.chunk.emit(OpCode::CreateAsyncFunction(const_idx), 0);
        Ok(())
    }

    pub(crate) fn compile_generator_expr(
        &mut self,
        name: &Option<String>,
        params: &[String],
        body: &[Statement],
    ) -> Result<(), CompileError> {
        let def = Self::compile_function_full(name.clone(), params.to_vec(), body, self.is_strict, true, false, false)?;
        let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
        self.chunk.emit(OpCode::CreateGenerator(const_idx), 0);
        Ok(())
    }

    pub(crate) fn compile_async_generator_expr(
        &mut self,
        name: &Option<String>,
        params: &[String],
        body: &[Statement],
    ) -> Result<(), CompileError> {
        let def = Self::compile_function_full(name.clone(), params.to_vec(), body, self.is_strict, true, false, false)?;
        let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
        self.chunk.emit(OpCode::CreateAsyncGenerator(const_idx), 0);
        Ok(())
    }

    pub(crate) fn compile_sequence(
        &mut self,
        exprs: &[Expression],
    ) -> Result<(), CompileError> {
        let len = exprs.len();
        for (i, e) in exprs.iter().enumerate() {
            self.compile_expression(e)?;
            if i + 1 < len {
                self.chunk.emit(OpCode::Pop, 0);
            }
        }
        Ok(())
    }
}
