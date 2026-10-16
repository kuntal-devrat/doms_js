use super::super::Compiler;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;

impl Compiler {
    pub(crate) fn compile_try_catch(
        &mut self,
        try_block: &Statement,
        catch_param: &Option<String>,
        catch_block: &Option<Box<Statement>>,
        finally_block: &Option<Box<Statement>>,
    ) -> Result<(), CompileError> {
        let catch_jump_idx = self.chunk.emit(OpCode::PushCatch(0), 0);
        self.compile_statement(try_block, false)?;
        self.chunk.emit(OpCode::PopCatch, 0);

        let jump_over_catch = self.chunk.emit(OpCode::Jump(0), 0);
        let catch_start = self.chunk.instructions.len() as i32;
        self.chunk.patch_jump(catch_jump_idx, catch_start);

        if let Some(catch_stmt) = catch_block {
            let has_catch_scope = !self.is_in_function && catch_param.is_some();
            self.symbols.enter_scope();
            if let Some(param) = catch_param {
                if !self.is_in_function {
                    let name_idx = self.chunk.add_constant(Constant::String(param.clone()));
                    self.emit_push_catch_scope(name_idx);
                    self.symbols.define_local(param.clone());
                } else {
                    let slot = self.symbols.define_local(param.clone());
                    self.chunk.emit(OpCode::SetLocal(slot), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
            } else {
                self.chunk.emit(OpCode::Pop, 0);
            }
            if !self.is_in_function {
                self.chunk.emit(OpCode::LoadUndefined, 0);
            }
            self.compile_statement(catch_stmt, false)?;
            if has_catch_scope {
                self.emit_pop_scope();
            }
            self.symbols.exit_scope();
        }

        let exit_target = self.chunk.instructions.len() as i32;
        self.chunk.patch_jump(jump_over_catch, exit_target);

        if let Some(finally_stmt) = finally_block {
            self.compile_statement(finally_stmt, false)?;
        }
        Ok(())
    }

    pub(crate) fn compile_throw(
        &mut self,
        expr: &Expression,
    ) -> Result<(), CompileError> {
        self.compile_expression(expr)?;
        self.chunk.emit(OpCode::Throw, 0);
        Ok(())
    }
}
