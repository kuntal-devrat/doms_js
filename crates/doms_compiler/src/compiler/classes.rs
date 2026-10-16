use super::Compiler;
use crate::bytecode::{Constant, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;
use std::sync::Arc;

impl Compiler {
    pub(crate) fn compile_class_definition(&mut self, class_def: &ClassDefinition) -> Result<(), CompileError> {
        self.compile_class_definition_with_name(class_def, None)
    }

    pub(crate) fn compile_class_definition_with_name(&mut self, class_def: &ClassDefinition, override_name: Option<String>) -> Result<(), CompileError> {
        let name = override_name.or_else(|| class_def.name.clone());
        let mut def = Self::compile_function(name, class_def.ctor_params.clone(), &class_def.ctor_body, self.is_strict)?;
        if class_def.super_class.is_some() {
            let len = def.chunk.instructions.len();
            if len >= 2 && def.chunk.instructions[len - 2] == OpCode::LoadUndefined && def.chunk.instructions[len - 1] == OpCode::Return {
                def.chunk.instructions[len - 2] = OpCode::This;
            }
        }
        let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
        self.chunk.emit(OpCode::CreateFunction(const_idx), 0);

        if let Some(ref sup) = class_def.super_class {
            self.compile_expression(sup)?;
            self.chunk.emit(OpCode::InheritClass, 0);
        }

        for element in &class_def.elements {
            self.chunk.emit(OpCode::Dup, 0);
            match &element.key {
                PropertyKey::Literal(s) => {
                    let const_idx = self.chunk.add_constant(Constant::String(s.clone()));
                    self.chunk.emit(OpCode::LoadConst(const_idx), 0);
                }
                PropertyKey::Computed(k_expr) => {
                    self.compile_expression(k_expr)?;
                }
            }
            let kind_flag = match &element.kind {
                ClassElementKind::Method(m_expr) => {
                    self.compile_expression(m_expr)?;
                    0u8
                }
                ClassElementKind::Getter(g_expr) => {
                    self.compile_expression(g_expr)?;
                    2u8
                }
                ClassElementKind::Setter(s_expr) => {
                    self.compile_expression(s_expr)?;
                    4u8
                }
            };
            let flags = if element.is_static { kind_flag | 1 } else { kind_flag };
            self.chunk.emit(OpCode::DefineClassElement(flags), 0);
        }
        Ok(())
    }
}
