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

        if let Some(ref n) = class_def.name {
            if self.symbols.scope_depth > 0 {
                let slot = if let Some(local) = self.symbols.get_local_in_current_scope_mut(n) {
                    local.is_declared = true;
                    local.slot
                } else {
                    self.symbols.define_local(n.clone())
                };
                self.chunk.emit(OpCode::Dup, 0);
                self.chunk.emit(OpCode::SetLocal(slot), 0);
                self.chunk.emit(OpCode::Pop, 0);
            } else {
                let name_idx = self.chunk.add_constant(Constant::String(n.clone()));
                self.chunk.emit(OpCode::Dup, 0);
                self.chunk.emit(OpCode::SetGlobalDirect(name_idx), 0);
                self.chunk.emit(OpCode::Pop, 0);
            }
        }

        for element in &class_def.elements {
            match &element.kind {
                ClassElementKind::StaticBlock(stmts) => {
                    self.chunk.emit(OpCode::Dup, 0); // duplicate Ctor
                    let def = Self::compile_function(None, vec![], stmts, self.is_strict)?;
                    let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                    self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                    self.chunk.emit(OpCode::CallWithThis(0), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
                ClassElementKind::Field(init_opt) => {
                    if element.is_static {
                        self.chunk.emit(OpCode::Dup, 0); // duplicate Ctor
                        match &element.key {
                            PropertyKey::Literal(s) if s.starts_with('#') => {
                                if let Some(init_expr) = init_opt {
                                    self.chunk.emit(OpCode::Dup, 0); // Ctor for CallWithThis
                                    let def = Self::compile_function(None, vec![], &[Statement::Return(Some(init_expr.clone()))], self.is_strict)?;
                                    let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                                    self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                                    self.chunk.emit(OpCode::CallWithThis(0), 0);
                                } else {
                                    self.chunk.emit(OpCode::LoadUndefined, 0);
                                }
                                let const_idx = self.chunk.add_constant(Constant::String(s.clone()));
                                self.chunk.emit(OpCode::DefinePrivateField(const_idx), 0);
                                self.chunk.emit(OpCode::Pop, 0);
                            }
                            PropertyKey::Literal(s) => {
                                if let Some(init_expr) = init_opt {
                                    self.chunk.emit(OpCode::Dup, 0); // Ctor for CallWithThis
                                    let def = Self::compile_function(None, vec![], &[Statement::Return(Some(init_expr.clone()))], self.is_strict)?;
                                    let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                                    self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                                    self.chunk.emit(OpCode::CallWithThis(0), 0);
                                } else {
                                    self.chunk.emit(OpCode::LoadUndefined, 0);
                                }
                                let const_idx = self.chunk.add_constant(Constant::String(s.clone()));
                                self.chunk.emit(OpCode::SetProp(const_idx), 0);
                                self.chunk.emit(OpCode::Pop, 0);
                            }
                            PropertyKey::Computed(k_expr) => {
                                if let Some(init_expr) = init_opt {
                                    self.chunk.emit(OpCode::Dup, 0); // duplicate Ctor for CallWithThis
                                    let def = Self::compile_function(None, vec![], &[Statement::Return(Some(init_expr.clone()))], self.is_strict)?;
                                    let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                                    self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                                    self.chunk.emit(OpCode::CallWithThis(0), 0);
                                    let val_tmp = self.symbols.define_local("@static_val".to_string());
                                    self.chunk.emit(OpCode::SetLocal(val_tmp), 0);
                                    self.chunk.emit(OpCode::Pop, 0);
                                    self.compile_expression(k_expr)?;
                                    self.chunk.emit(OpCode::GetLocal(val_tmp), 0);
                                } else {
                                    self.compile_expression(k_expr)?;
                                    self.chunk.emit(OpCode::LoadUndefined, 0);
                                }
                                self.chunk.emit(OpCode::SetElem, 0);
                                self.chunk.emit(OpCode::Pop, 0);
                            }
                        }
                    }
                }
                ClassElementKind::Method(_)
                | ClassElementKind::Getter(_)
                | ClassElementKind::Setter(_) => {
                    if element.is_static && matches!(&element.key, PropertyKey::Literal(s) if s.starts_with('#')) {
                        let s = match &element.key { PropertyKey::Literal(s) => s.clone(), _ => unreachable!() };
                        let m_expr = match &element.kind {
                            ClassElementKind::Method(m) => m,
                            ClassElementKind::Getter(g) => g,
                            ClassElementKind::Setter(set) => set,
                            _ => unreachable!(),
                        };
                        self.chunk.emit(OpCode::Dup, 0); // duplicate Ctor
                        self.compile_expression(m_expr)?;
                        let const_idx = self.chunk.add_constant(Constant::String(s));
                        if matches!(&element.kind, ClassElementKind::Getter(_)) {
                            self.chunk.emit(OpCode::DefinePrivateGetter(const_idx), 0);
                        } else if matches!(&element.kind, ClassElementKind::Setter(_)) {
                            self.chunk.emit(OpCode::DefinePrivateSetter(const_idx), 0);
                        } else {
                            self.chunk.emit(OpCode::DefinePrivateField(const_idx), 0);
                        }
                        self.chunk.emit(OpCode::Pop, 0);
                        continue;
                    }
                    if !element.is_static && matches!(&element.key, PropertyKey::Literal(s) if s.starts_with('#')) {
                        continue;
                    }
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
                        _ => unreachable!(),
                    };
                    let flags = if element.is_static { kind_flag | 1 } else { kind_flag };
                    self.chunk.emit(OpCode::DefineClassElement(flags), 0);
                }
            }
        }
        Ok(())
    }
}
