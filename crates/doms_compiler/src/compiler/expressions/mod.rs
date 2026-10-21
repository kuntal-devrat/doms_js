//! Expression compilation and routing.

use super::Compiler;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;

mod assignments;
mod calls_new;
mod functions_async;
mod literals_objects;
mod operators;
mod optional_chain;

pub(crate) fn unwrap_grouping<'a>(mut expr: &'a Expression) -> &'a Expression {
    while let Expression::Grouping(inner) = expr {
        expr = inner.as_ref();
    }
    expr
}

impl Compiler {
    pub(crate) fn compile_expression(&mut self, expr: &Expression) -> Result<(), CompileError> {
        match expr {
            Expression::Literal(lit) => {
                self.compile_literal(lit)?;
            }
            Expression::Identifier(name) => {
                self.compile_identifier(name)?;
            }
            Expression::This => {
                self.chunk.emit(OpCode::This, 0);
            }
            Expression::Assignment { target, value } => {
                self.compile_assignment(target, value)?;
            }
            Expression::MemberAssignment { object, property, value } => {
                self.compile_member_assignment(object, property, value)?;
            }
            Expression::IndexAssignment { object, index, value } => {
                self.compile_index_assignment(object, index, value)?;
            }
            Expression::Update { op, is_prefix, expr } => {
                self.compile_update(op, *is_prefix, expr)?;
            }
            Expression::Object(props) => {
                self.compile_object(props)?;
            }
            Expression::Array(elements) => {
                self.compile_array(elements)?;
            }
            Expression::Index { object, index } => {
                self.compile_expression(object)?;
                self.compile_expression(index)?;
                self.chunk.emit(OpCode::GetElem, 0);
            }
            Expression::Binary { op, left, right } => {
                self.compile_binary(op, left, right)?;
            }
            Expression::Unary { op, expr } => {
                self.compile_unary(op, expr)?;
            }
            Expression::Conditional { condition, then_expr, else_expr } => {
                self.compile_conditional(condition, then_expr, else_expr)?;
            }
            Expression::Call { callee, args } => {
                self.compile_call(callee, args)?;
            }
            Expression::Member { object, property } => {
                self.compile_expression(object)?;
                let prop_idx =
                    self.chunk.add_constant(crate::bytecode::Constant::String(property.clone()));
                self.chunk.emit(OpCode::GetProp(prop_idx), 0);
            }
            Expression::FunctionExpr { name, params, body } => {
                self.compile_function_expr(name, params, body)?;
            }
            Expression::ArrowExpr { params, body } => {
                self.compile_arrow_expr(params, body)?;
            }
            Expression::MethodExpr { name, params, body } => {
                self.compile_method_expr(name, params, body)?;
            }
            Expression::AsyncFunctionExpr { name, params, body } => {
                self.compile_async_function_expr(name, params, body)?;
            }
            Expression::GeneratorExpr { name, params, body } => {
                self.compile_generator_expr(name, params, body)?;
            }
            Expression::AsyncGeneratorExpr { name, params, body } => {
                self.compile_async_generator_expr(name, params, body)?;
            }
            Expression::NewTarget => {
                self.chunk.emit(OpCode::GetNewTarget, 0);
            }
            Expression::Super => {
                self.chunk.emit(OpCode::LoadSuper, 0);
            }
            Expression::New { callee, args } => {
                self.compile_new(callee, args)?;
            }
            Expression::Await(expr) => {
                self.compile_expression(expr)?;
                self.chunk.emit(OpCode::Await, 0);
            }
            Expression::Yield(opt_expr) => {
                if let Some(expr) = opt_expr {
                    self.compile_expression(expr)?;
                } else {
                    self.chunk.emit(OpCode::LoadUndefined, 0);
                }
                self.chunk.emit(OpCode::Yield, 0);
            }
            Expression::YieldStar(expr) => {
                self.compile_expression(expr)?;
                self.chunk.emit(OpCode::GetIterator, 0);
                let iter_slot = self
                    .symbols
                    .define_local(format!("@iter_ys_{}", self.chunk.instructions.len()));
                self.chunk.emit(OpCode::SetLocal(iter_slot), 0);
                self.chunk.emit(OpCode::Pop, 0);

                let loop_start = self.chunk.instructions.len() as i32;
                self.chunk.emit(OpCode::GetLocal(iter_slot), 0);
                let jump_exit_idx = self.chunk.emit(OpCode::IteratorNext(0), 0);
                self.chunk.emit(OpCode::Yield, 0);
                self.chunk.emit(OpCode::Pop, 0);
                self.chunk.emit(OpCode::Jump(loop_start), 0);

                let exit_target = self.chunk.instructions.len() as i32;
                self.chunk.patch_jump(jump_exit_idx, exit_target);
                self.chunk.emit(OpCode::LoadUndefined, 0);
            }
            Expression::DynamicImport(expr) => {
                self.compile_expression(expr)?;
                self.chunk.emit(OpCode::DynamicImport, 0);
            }
            Expression::Sequence(exprs) => {
                self.compile_sequence(exprs)?;
            }
            Expression::ClassExpr(class_def) => {
                self.compile_class_definition(class_def)?;
            }
            Expression::Grouping(inner) => {
                self.compile_expression(inner)?;
            }
            Expression::Spread(inner) => {
                self.compile_expression(inner)?;
            }
            Expression::PrivateIdentifier(name) => {
                let idx = self.chunk.add_constant(Constant::String(name.clone()));
                self.chunk.emit(OpCode::LoadConst(idx), 0);
            }
            Expression::ObjectRest { target, excluded_keys } => {
                self.compile_expression(target)?;
                for key in excluded_keys {
                    self.compile_expression(key)?;
                }
                self.chunk.emit(OpCode::ObjectRest(excluded_keys.len() as u16), 0);
            }
            Expression::DestructuringAssignment { pattern, value } => {
                self.compile_destructuring_assignment(pattern, value)?;
            }
            Expression::DefineField { target, property, value } => {
                self.compile_expression(target)?;
                self.compile_expression(value)?;
                let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                self.chunk.emit(OpCode::DefinePrivateField(prop_idx), 0);
            }
            Expression::DefinePrivateAccessor { target, property, is_getter, accessor } => {
                self.compile_expression(target)?;
                self.compile_expression(accessor)?;
                let prop_idx = self.chunk.add_constant(Constant::String(property.clone()));
                if *is_getter {
                    self.chunk.emit(OpCode::DefinePrivateGetter(prop_idx), 0);
                } else {
                    self.chunk.emit(OpCode::DefinePrivateSetter(prop_idx), 0);
                }
            }
            Expression::OptionalChain(oc) => {
                self.compile_optional_chain(oc)?;
            }
            Expression::TaggedTemplate { tag, quasis, raw_quasis, exprs } => {
                self.compile_tagged_template(tag, quasis, raw_quasis, exprs)?;
            }
        }
        Ok(())
    }
}
