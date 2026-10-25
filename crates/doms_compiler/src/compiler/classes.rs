use super::Compiler;
use crate::bytecode::{Constant, FunctionDef, OpCode};
use crate::compiler::CompileError;
use doms_parser::ast::*;
use std::sync::Arc;

impl Compiler {
    pub(crate) fn compile_class_definition(
        &mut self,
        class_def: &ClassDefinition,
    ) -> Result<(), CompileError> {
        self.compile_class_definition_with_name(class_def, None, false)
    }

    pub(crate) fn compile_class_decl(
        &mut self,
        class_def: &ClassDefinition,
    ) -> Result<(), CompileError> {
        self.compile_class_definition_with_name(class_def, None, true)
    }

    pub(crate) fn compile_class_definition_with_name(
        &mut self,
        class_def: &ClassDefinition,
        override_name: Option<String>,
        is_declaration: bool,
    ) -> Result<(), CompileError> {
        let prev_strict = self.is_strict;
        self.is_strict = true;
        let res = self.compile_class_definition_inner(class_def, override_name, is_declaration);
        self.is_strict = prev_strict;
        res
    }

    fn compile_class_definition_inner(
        &mut self,
        class_def: &ClassDefinition,
        override_name: Option<String>,
        is_declaration: bool,
    ) -> Result<(), CompileError> {
        let name = override_name.or_else(|| class_def.name.clone());

        if let Some(ref n) = class_def.name {
            self.shadowed_class_names.push(n.clone());
        }

        let class_scope_name_idx = if let Some(ref n) = class_def.name {
            let idx = self.chunk.add_constant(Constant::String(n.clone()));
            self.chunk.emit(OpCode::PushClassScope(idx), 0);
            Some(idx)
        } else {
            let idx = self.chunk.add_constant(Constant::String("".to_string()));
            self.chunk.emit(OpCode::PushClassScope(idx), 0);
            None
        };

        let super_slot = if let Some(ref sup) = class_def.super_class {
            self.compile_expression(sup)?;
            let slot =
                self.symbols.define_local(format!("@super_{}", self.chunk.instructions.len()));
            self.chunk.emit(OpCode::SetLocal(slot), 0);
            self.chunk.emit(OpCode::Pop, 0);
            Some(slot)
        } else {
            None
        };

        let mut class_private_names = self.private_names.clone();
        for el in &class_def.elements {
            if let PropertyKey::Literal(ref k) = el.key {
                if k.starts_with('#') {
                    let base_name = k.split('@').next().unwrap_or(k);
                    class_private_names.insert(base_name.to_string(), k.clone());
                }
            }
        }

        let is_derived = class_def.super_class.is_some();
        let mut def = if is_derived && !class_def.has_constructor {
            if class_def.ctor_body.is_empty() {
                let mut chunk = crate::bytecode::BytecodeChunk::new();
                chunk.emit(OpCode::SuperCallForward, 0);
                chunk.emit(OpCode::Return, 0);
                FunctionDef {
                    name,
                    params: Vec::new(),
                    length: 0,
                    self_slot: None,
                    local_names: Arc::new(Vec::new()),
                    lexical_local_names: Arc::new(Vec::new()),
                    chunk,
                    is_strict: true,
                    has_param_exprs: false,
                    is_generator: false,
                    is_arrow: false,
                    is_method: false,
                    is_class_constructor: true,
                    is_derived_constructor: true,
                }
            } else {
                Self::compile_derived_default_constructor(
                    name,
                    &class_def.ctor_body,
                    self.chunk.imported_bindings.clone(),
                    self.chunk.export_aliases.clone(),
                )?
            }
        } else {
            Self::compile_class_constructor(
                name,
                class_def.ctor_params.clone(),
                &class_def.ctor_body,
                is_derived,
                self.chunk.imported_bindings.clone(),
                self.chunk.export_aliases.clone(),
            )?
        };
        def.chunk.private_names = class_private_names.clone();
        let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
        self.chunk.emit(OpCode::CreateFunction(const_idx), 0);

        if let Some(slot) = super_slot {
            self.chunk.emit(OpCode::GetLocal(slot), 0);
            self.chunk.emit(OpCode::InheritClass, 0);
        }

        if let Some(idx) = class_scope_name_idx {
            self.chunk.emit(OpCode::InitClassBinding(idx), 0);
        }

        let prev_private_names = std::mem::replace(&mut self.private_names, class_private_names);

        let mut static_computed_slots: Vec<Option<u16>> = Vec::new();

        // Phase 1: Evaluate all element names and define methods (ECMA-262 Step 28)
        for element in &class_def.elements {
            match &element.kind {
                ClassElementKind::StaticBlock(_) => {}
                ClassElementKind::Field(_) => {
                    if let Some(ref var_name) = element.computed_key_var {
                        if let PropertyKey::Computed(ref k_expr) = element.key {
                            self.compile_expression(k_expr)?;
                            self.chunk.emit(OpCode::ToPropertyKey, 0);
                            let var_idx = self.chunk.add_constant(Constant::String(var_name.clone()));
                            self.chunk.emit(OpCode::StoreScopeBinding(var_idx), 0);
                        }
                    } else if element.is_static {
                        if let PropertyKey::Computed(ref k_expr) = element.key {
                            self.compile_expression(k_expr)?;
                            self.chunk.emit(OpCode::ToPropertyKey, 0);
                            let key_slot = self.symbols.define_local(format!("@static_key_{}", static_computed_slots.len()));
                            self.chunk.emit(OpCode::SetLocal(key_slot), 0);
                            self.chunk.emit(OpCode::Pop, 0);
                            static_computed_slots.push(Some(key_slot));
                        } else {
                            static_computed_slots.push(None);
                        }
                    }
                }
                ClassElementKind::Method(_)
                | ClassElementKind::Getter(_)
                | ClassElementKind::Setter(_) => {
                    if element.is_static
                        && matches!(&element.key, PropertyKey::Literal(s) if s.starts_with('#'))
                    {
                        let s = match &element.key {
                            PropertyKey::Literal(s) => s.clone(),
                            _ => unreachable!(),
                        };
                        let m_expr = match &element.kind {
                            ClassElementKind::Method(m) => m,
                            ClassElementKind::Getter(g) => g,
                            ClassElementKind::Setter(set) => set,
                            _ => unreachable!(),
                        };
                        self.chunk.emit(OpCode::Dup, 0); // duplicate Ctor for DefinePrivate*
                        self.chunk.emit(OpCode::Dup, 0); // duplicate Ctor for SetMethodHomeObject
                        self.compile_expression(m_expr)?;
                        self.chunk.emit(OpCode::SetMethodHomeObject, 0);
                        let const_idx = self.chunk.add_constant(Constant::String(s));
                        if matches!(&element.kind, ClassElementKind::Getter(_)) {
                            self.chunk.emit(OpCode::DefinePrivateGetter(const_idx), 0);
                        } else if matches!(&element.kind, ClassElementKind::Setter(_)) {
                            self.chunk.emit(OpCode::DefinePrivateSetter(const_idx), 0);
                        } else {
                            self.chunk.emit(OpCode::DefinePrivateMethod(const_idx), 0);
                        }
                        self.chunk.emit(OpCode::Pop, 0);
                        continue;
                    }
                    if !element.is_static
                        && matches!(&element.key, PropertyKey::Literal(s) if s.starts_with('#'))
                    {
                        if let Some(ref var_name) = element.computed_key_var {
                            let m_expr = match &element.kind {
                                ClassElementKind::Method(m) => m,
                                ClassElementKind::Getter(g) => g,
                                ClassElementKind::Setter(set) => set,
                                _ => unreachable!(),
                            };
                            self.chunk.emit(OpCode::Dup, 0); // duplicate Ctor
                            let proto_idx =
                                self.chunk.add_constant(Constant::String("prototype".to_string()));
                            self.chunk.emit(OpCode::GetProp(proto_idx), 0); // stack: [..., Ctor, Ctor.prototype]
                            self.compile_expression(m_expr)?; // stack: [..., Ctor, Ctor.prototype, method_fn]
                            self.chunk.emit(OpCode::SetMethodHomeObject, 0); // stack: [..., Ctor, method_fn]
                            let var_idx =
                                self.chunk.add_constant(Constant::String(var_name.clone()));
                            self.chunk.emit(OpCode::StoreScopeBinding(var_idx), 0); // stores method_fn in class scope
                        }
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

        // Phase 2: Execute static elements (static fields and static blocks) in order (ECMA-262 Step 34)
        let mut static_field_idx = 0;
        for element in &class_def.elements {
            match &element.kind {
                ClassElementKind::StaticBlock(stmts) => {
                    self.chunk.emit(OpCode::Dup, 0); // duplicate Ctor
                    let mut def = Self::compile_function(None, vec![], stmts, self.is_strict)?;
                    def.chunk.private_names = self.private_names.clone();
                    let const_idx = self.chunk.add_constant(Constant::Function(Arc::new(def)));
                    self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                    self.chunk.emit(OpCode::CallWithThis(0), 0);
                    self.chunk.emit(OpCode::Pop, 0);
                }
                ClassElementKind::Field(init_opt) if element.is_static => {
                    let key_slot_opt = static_computed_slots[static_field_idx];
                    static_field_idx += 1;
                    self.chunk.emit(OpCode::Dup, 0); // duplicate Ctor
                    match &element.key {
                        PropertyKey::Literal(s) if s.starts_with('#') => {
                            if let Some(init_expr) = init_opt {
                                self.chunk.emit(OpCode::Dup, 0); // Ctor for CallWithThis
                                let mut def = Self::compile_function(
                                    None,
                                    vec![],
                                    &[Statement::Return(Some(init_expr.clone()))],
                                    self.is_strict,
                                )?;
                                def.chunk.private_names = self.private_names.clone();
                                let const_idx =
                                    self.chunk.add_constant(Constant::Function(Arc::new(def)));
                                self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                                self.chunk.emit(OpCode::EnterFieldInit, 0);
                                self.chunk.emit(OpCode::CallWithThis(0), 0);
                                self.chunk.emit(OpCode::LeaveFieldInit, 0);
                            } else {
                                self.chunk.emit(OpCode::LoadUndefined, 0);
                            }
                            let const_idx =
                                self.chunk.add_constant(Constant::String(s.clone()));
                            self.chunk.emit(OpCode::DefinePrivateField(const_idx), 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        }
                        PropertyKey::Literal(s) => {
                            if let Some(init_expr) = init_opt {
                                self.chunk.emit(OpCode::Dup, 0); // Ctor for CallWithThis
                                let mut def = Self::compile_function(
                                    None,
                                    vec![],
                                    &[Statement::Return(Some(init_expr.clone()))],
                                    self.is_strict,
                                )?;
                                def.chunk.private_names = self.private_names.clone();
                                let const_idx =
                                    self.chunk.add_constant(Constant::Function(Arc::new(def)));
                                self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                                self.chunk.emit(OpCode::EnterFieldInit, 0);
                                self.chunk.emit(OpCode::CallWithThis(0), 0);
                                self.chunk.emit(OpCode::LeaveFieldInit, 0);
                            } else {
                                self.chunk.emit(OpCode::LoadUndefined, 0);
                            }
                            let const_idx =
                                self.chunk.add_constant(Constant::String(s.clone()));
                            self.chunk.emit(OpCode::DefineDataProp(const_idx), 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        }
                        PropertyKey::Computed(_) => {
                            let key_slot = key_slot_opt.unwrap();
                            if let Some(init_expr) = init_opt {
                                self.chunk.emit(OpCode::Dup, 0); // duplicate Ctor for CallWithThis
                                let mut def = Self::compile_function(
                                    None,
                                    vec![],
                                    &[Statement::Return(Some(init_expr.clone()))],
                                    self.is_strict,
                                )?;
                                def.chunk.private_names = self.private_names.clone();
                                let const_idx =
                                    self.chunk.add_constant(Constant::Function(Arc::new(def)));
                                self.chunk.emit(OpCode::CreateFunction(const_idx), 0);
                                self.chunk.emit(OpCode::EnterFieldInit, 0);
                                self.chunk.emit(OpCode::CallWithThis(0), 0);
                                self.chunk.emit(OpCode::LeaveFieldInit, 0);
                                let val_slot = self.symbols.define_local("@static_val".to_string());
                                self.chunk.emit(OpCode::SetLocal(val_slot), 0);
                                self.chunk.emit(OpCode::Pop, 0);

                                self.chunk.emit(OpCode::GetLocal(key_slot), 0);
                                self.chunk.emit(OpCode::GetLocal(val_slot), 0);
                            } else {
                                self.chunk.emit(OpCode::GetLocal(key_slot), 0);
                                self.chunk.emit(OpCode::LoadUndefined, 0);
                            }
                            self.chunk.emit(OpCode::DefineDataPropComputed, 0);
                            self.chunk.emit(OpCode::Pop, 0);
                        }
                    }
                }
                _ => {}
            }
        }

        self.chunk.emit(OpCode::PopScope, 0);
        if class_def.name.is_some() {
            self.shadowed_class_names.pop();
        }

        if is_declaration {
            if let Some(ref n) = class_def.name {
                let slot = if let Some(local) = self.symbols.get_local_in_current_scope_mut(n) {
                    local.is_declared = true;
                    local.slot
                } else {
                    self.symbols.define_local(n.clone())
                };
                self.chunk.emit(OpCode::Dup, 0);
                self.chunk.emit(OpCode::SetLocal(slot), 0);
                self.chunk.emit(OpCode::Pop, 0);
            }
        }

        self.private_names = prev_private_names;

        Ok(())
    }
}
