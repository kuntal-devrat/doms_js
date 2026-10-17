use super::{ParseError, Parser};
use crate::ast::*;
use crate::lexer::TokenKind;

impl<'a> Parser<'a> {
    pub(crate) fn parse_array_destructuring_into(
        &mut self,
        tmp_name: &str,
        declarations: &mut Vec<VarDeclarator>,
    ) -> Result<(), ParseError> {
        let mut curr_idx = 0usize;
        while !self.check(&TokenKind::RBracket) && self.current.kind != TokenKind::Eof {
            if self.check(&TokenKind::Comma) {
                self.advance()?;
                curr_idx += 1;
                continue;
            }
            if self.match_token(&TokenKind::DotDotDot)? {
                let elem_init = Expression::Conditional {
                    condition: Box::new(Expression::Binary {
                        op: BinaryOp::StrictEq,
                        left: Box::new(Expression::Unary {
                            op: UnaryOp::Typeof,
                            expr: Box::new(Expression::Member {
                                object: Box::new(Expression::Identifier(tmp_name.to_string())),
                                property: "slice".to_string(),
                            }),
                        }),
                        right: Box::new(Expression::Literal(Literal::String {
                            value: "function".to_string(),
                            has_escape: false,
                            has_legacy_escape: false,
                        })),
                    }),
                    then_expr: Box::new(Expression::Call {
                        callee: Box::new(Expression::Member {
                            object: Box::new(Expression::Identifier(tmp_name.to_string())),
                            property: "slice".to_string(),
                        }),
                        args: vec![Expression::Literal(Literal::Number(curr_idx as f64))],
                    }),
                    else_expr: Box::new(Expression::Call {
                        callee: Box::new(Expression::Member {
                            object: Box::new(Expression::Call {
                                callee: Box::new(Expression::Member {
                                    object: Box::new(Expression::Identifier("Array".to_string())),
                                    property: "from".to_string(),
                                }),
                                args: vec![Expression::Identifier(tmp_name.to_string())],
                            }),
                            property: "slice".to_string(),
                        }),
                        args: vec![Expression::Literal(Literal::Number(curr_idx as f64))],
                    }),
                };
                if self.check(&TokenKind::LBracket) {
                    self.advance()?;
                    let nested_tmp = format!("@destruct_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    let mut nested_decls = Vec::new();
                    self.parse_array_destructuring_into(&nested_tmp, &mut nested_decls)?;
                    declarations.push(VarDeclarator { name: nested_tmp, init: Some(elem_init) });
                    declarations.extend(nested_decls);
                } else if self.check(&TokenKind::LBrace) {
                    self.advance()?;
                    let nested_tmp = format!("@destruct_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    let mut nested_decls = Vec::new();
                    self.parse_object_destructuring_into(&nested_tmp, &mut nested_decls)?;
                    declarations.push(VarDeclarator { name: nested_tmp, init: Some(elem_init) });
                    declarations.extend(nested_decls);
                } else {
                    let id = self.expect_identifier()?;
                    declarations.push(VarDeclarator { name: id, init: Some(elem_init) });
                }
                break;
            }

            let elem_init = Expression::Index {
                object: Box::new(Expression::Identifier(tmp_name.to_string())),
                index: Box::new(Expression::Literal(Literal::Number(curr_idx as f64))),
            };

            if self.check(&TokenKind::LBracket) {
                self.advance()?;
                let nested_tmp = format!("@destruct_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let mut nested_decls = Vec::new();
                self.parse_array_destructuring_into(&nested_tmp, &mut nested_decls)?;
                let def = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                let final_init = if let Some(def_expr) = def {
                    let val_var = format!("@val_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    declarations.push(VarDeclarator { name: val_var.clone(), init: Some(elem_init) });
                    Expression::Conditional {
                        condition: Box::new(Expression::Binary {
                            op: BinaryOp::StrictEq,
                            left: Box::new(Expression::Identifier(val_var.clone())),
                            right: Box::new(Expression::Literal(Literal::Undefined)),
                        }),
                        then_expr: Box::new(def_expr),
                        else_expr: Box::new(Expression::Identifier(val_var)),
                    }
                } else {
                    elem_init
                };
                declarations.push(VarDeclarator { name: nested_tmp, init: Some(final_init) });
                declarations.extend(nested_decls);
            } else if self.check(&TokenKind::LBrace) {
                self.advance()?;
                let nested_tmp = format!("@destruct_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let mut nested_decls = Vec::new();
                self.parse_object_destructuring_into(&nested_tmp, &mut nested_decls)?;
                let def = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                let final_init = if let Some(def_expr) = def {
                    let val_var = format!("@val_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    declarations.push(VarDeclarator { name: val_var.clone(), init: Some(elem_init) });
                    Expression::Conditional {
                        condition: Box::new(Expression::Binary {
                            op: BinaryOp::StrictEq,
                            left: Box::new(Expression::Identifier(val_var.clone())),
                            right: Box::new(Expression::Literal(Literal::Undefined)),
                        }),
                        then_expr: Box::new(def_expr),
                        else_expr: Box::new(Expression::Identifier(val_var)),
                    }
                } else {
                    elem_init
                };
                declarations.push(VarDeclarator { name: nested_tmp, init: Some(final_init) });
                declarations.extend(nested_decls);
            } else {
                let id = self.expect_identifier()?;
                let def = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                let final_init = if let Some(def_expr) = def {
                    let val_var = format!("@val_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    declarations.push(VarDeclarator { name: val_var.clone(), init: Some(elem_init) });
                    Expression::Conditional {
                        condition: Box::new(Expression::Binary {
                            op: BinaryOp::StrictEq,
                            left: Box::new(Expression::Identifier(val_var.clone())),
                            right: Box::new(Expression::Literal(Literal::Undefined)),
                        }),
                        then_expr: Box::new(def_expr),
                        else_expr: Box::new(Expression::Identifier(val_var)),
                    }
                } else {
                    elem_init
                };
                declarations.push(VarDeclarator { name: id, init: Some(final_init) });
            }

            curr_idx += 1;
            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RBracket)?;
        Ok(())
    }

    pub(crate) fn parse_object_destructuring_into(
        &mut self,
        tmp_name: &str,
        declarations: &mut Vec<VarDeclarator>,
    ) -> Result<(), ParseError> {
        let mut excluded_keys: Vec<Expression> = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            if self.check(&TokenKind::Comma) {
                self.advance()?;
                continue;
            }
            if self.match_token(&TokenKind::DotDotDot)? {
                let rest_init = Expression::ObjectRest {
                    target: Box::new(Expression::Identifier(tmp_name.to_string())),
                    excluded_keys: excluded_keys.clone(),
                };
                if self.check(&TokenKind::LBracket) {
                    self.advance()?;
                    let nested_tmp = format!("@destruct_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    let mut nested_decls = Vec::new();
                    self.parse_array_destructuring_into(&nested_tmp, &mut nested_decls)?;
                    declarations.push(VarDeclarator { name: nested_tmp, init: Some(rest_init) });
                    declarations.extend(nested_decls);
                } else if self.check(&TokenKind::LBrace) {
                    self.advance()?;
                    let nested_tmp = format!("@destruct_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    let mut nested_decls = Vec::new();
                    self.parse_object_destructuring_into(&nested_tmp, &mut nested_decls)?;
                    declarations.push(VarDeclarator { name: nested_tmp, init: Some(rest_init) });
                    declarations.extend(nested_decls);
                } else {
                    let id = self.expect_identifier()?;
                    declarations.push(VarDeclarator {
                        name: id,
                        init: Some(rest_init),
                    });
                }
                if self.check(&TokenKind::Comma) {
                    return Err(ParseError::UnexpectedToken(TokenKind::Comma, self.current.line, self.current.col));
                }
                break;
            }

            let (prop_key, is_ident) = self.parse_property_name_meta()?;
            let (key_expr, key_str) = match prop_key {
                PropertyKey::Computed(expr) => (Some(expr), None),
                PropertyKey::Literal(s) => (None, Some(s)),
            };

            let base_prop = if let Some(k) = key_expr {
                let key_var = format!("@key_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let key_to_string_expr = Expression::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(k),
                    right: Box::new(Expression::Literal(Literal::String {
                        value: "".to_string(),
                        has_escape: false,
                        has_legacy_escape: false,
                    })),
                };
                declarations.push(VarDeclarator {
                    name: key_var.clone(),
                    init: Some(key_to_string_expr),
                });
                excluded_keys.push(Expression::Identifier(key_var.clone()));
                Expression::Index {
                    object: Box::new(Expression::Identifier(tmp_name.to_string())),
                    index: Box::new(Expression::Identifier(key_var)),
                }
            } else {
                let key = key_str.clone().unwrap();
                excluded_keys.push(Expression::Literal(Literal::String {
                    value: key.clone(),
                    has_escape: false,
                    has_legacy_escape: false,
                }));
                Expression::Member {
                    object: Box::new(Expression::Identifier(tmp_name.to_string())),
                    property: key,
                }
            };

            let has_colon = self.match_token(&TokenKind::Colon)?;
            if has_colon && self.check(&TokenKind::LBracket) {
                self.advance()?;
                let nested_tmp = format!("@destruct_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let mut nested_decls = Vec::new();
                self.parse_array_destructuring_into(&nested_tmp, &mut nested_decls)?;
                let def = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                let prop_init = if let Some(def_expr) = def {
                    let val_var = format!("@val_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    declarations.push(VarDeclarator { name: val_var.clone(), init: Some(base_prop) });
                    Expression::Conditional {
                        condition: Box::new(Expression::Binary {
                            op: BinaryOp::StrictEq,
                            left: Box::new(Expression::Identifier(val_var.clone())),
                            right: Box::new(Expression::Literal(Literal::Undefined)),
                        }),
                        then_expr: Box::new(def_expr),
                        else_expr: Box::new(Expression::Identifier(val_var)),
                    }
                } else {
                    base_prop
                };
                declarations.push(VarDeclarator { name: nested_tmp, init: Some(prop_init) });
                declarations.extend(nested_decls);
            } else if has_colon && self.check(&TokenKind::LBrace) {
                self.advance()?;
                let nested_tmp = format!("@destruct_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let mut nested_decls = Vec::new();
                self.parse_object_destructuring_into(&nested_tmp, &mut nested_decls)?;
                let def = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                let prop_init = if let Some(def_expr) = def {
                    let val_var = format!("@val_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    declarations.push(VarDeclarator { name: val_var.clone(), init: Some(base_prop) });
                    Expression::Conditional {
                        condition: Box::new(Expression::Binary {
                            op: BinaryOp::StrictEq,
                            left: Box::new(Expression::Identifier(val_var.clone())),
                            right: Box::new(Expression::Literal(Literal::Undefined)),
                        }),
                        then_expr: Box::new(def_expr),
                        else_expr: Box::new(Expression::Identifier(val_var)),
                    }
                } else {
                    base_prop
                };
                declarations.push(VarDeclarator { name: nested_tmp, init: Some(prop_init) });
                declarations.extend(nested_decls);
            } else {
                let target = if has_colon {
                    self.expect_identifier()?
                } else {
                    if !is_ident {
                        return Err(ParseError::UnexpectedToken(self.current.kind.clone(), self.current.line, self.current.col));
                    }
                    key_str.unwrap()
                };
                let def = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                let prop_init = if let Some(def_expr) = def {
                    let val_var = format!("@val_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    declarations.push(VarDeclarator { name: val_var.clone(), init: Some(base_prop) });
                    Expression::Conditional {
                        condition: Box::new(Expression::Binary {
                            op: BinaryOp::StrictEq,
                            left: Box::new(Expression::Identifier(val_var.clone())),
                            right: Box::new(Expression::Literal(Literal::Undefined)),
                        }),
                        then_expr: Box::new(def_expr),
                        else_expr: Box::new(Expression::Identifier(val_var)),
                    }
                } else {
                    base_prop
                };
                declarations.push(VarDeclarator { name: target, init: Some(prop_init) });
            }

            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(())
    }

    pub fn desugar_assignment_destructuring(
        &mut self,
        lhs: &Expression,
        rhs: Expression,
        out: &mut Vec<Statement>,
    ) {
        match lhs {
            Expression::Grouping(inner) => {
                self.desugar_assignment_destructuring(inner, rhs, out);
            }
            Expression::Identifier(name) => {
                out.push(Statement::Expression(Expression::Assignment {
                    target: name.clone(),
                    value: Box::new(rhs),
                }));
            }
            Expression::Member { object, property } => {
                out.push(Statement::Expression(Expression::MemberAssignment {
                    object: object.clone(),
                    property: property.clone(),
                    value: Box::new(rhs),
                }));
            }
            Expression::Index { object, index } => {
                out.push(Statement::Expression(Expression::IndexAssignment {
                    object: object.clone(),
                    index: index.clone(),
                    value: Box::new(rhs),
                }));
            }
            Expression::Array(elements) => {
                let tmp_var = format!("@dstr_arr_{}", self.destruct_counter);
                self.destruct_counter += 1;
                out.push(Statement::VarDecl {
                    kind: VarKind::Let,
                    declarations: vec![VarDeclarator {
                        name: tmp_var.clone(),
                        init: Some(rhs),
                    }],
                });
                for (idx, elem) in elements.iter().enumerate() {
                    match elem {
                        Expression::Literal(Literal::Hole) => {}
                        Expression::Spread(inner) => {
                            let rest_rhs = Expression::Conditional {
                                condition: Box::new(Expression::Binary {
                                    op: BinaryOp::StrictEq,
                                    left: Box::new(Expression::Unary {
                                        op: UnaryOp::Typeof,
                                        expr: Box::new(Expression::Member {
                                            object: Box::new(Expression::Identifier(tmp_var.clone())),
                                            property: "slice".to_string(),
                                        }),
                                    }),
                                    right: Box::new(Expression::Literal(Literal::String {
                                        value: "function".to_string(),
                                        has_escape: false,
                                        has_legacy_escape: false,
                                    })),
                                }),
                                then_expr: Box::new(Expression::Call {
                                    callee: Box::new(Expression::Member {
                                        object: Box::new(Expression::Identifier(tmp_var.clone())),
                                        property: "slice".to_string(),
                                    }),
                                    args: vec![Expression::Literal(Literal::Number(idx as f64))],
                                }),
                                else_expr: Box::new(Expression::Call {
                                    callee: Box::new(Expression::Member {
                                        object: Box::new(Expression::Call {
                                            callee: Box::new(Expression::Member {
                                                object: Box::new(Expression::Identifier("Array".to_string())),
                                                property: "from".to_string(),
                                            }),
                                            args: vec![Expression::Identifier(tmp_var.clone())],
                                        }),
                                        property: "slice".to_string(),
                                    }),
                                    args: vec![Expression::Literal(Literal::Number(idx as f64))],
                                }),
                            };
                            self.desugar_assignment_destructuring(inner, rest_rhs, out);
                        }
                        Expression::Assignment { target, value: def } => {
                            let elem_rhs = Expression::Index {
                                object: Box::new(Expression::Identifier(tmp_var.clone())),
                                index: Box::new(Expression::Literal(Literal::Number(idx as f64))),
                            };
                            let val_var = format!("@val_{}", self.destruct_counter);
                            self.destruct_counter += 1;
                            out.push(Statement::VarDecl {
                                kind: VarKind::Let,
                                declarations: vec![VarDeclarator {
                                    name: val_var.clone(),
                                    init: Some(elem_rhs),
                                }],
                            });
                            let cond_expr = Expression::Conditional {
                                condition: Box::new(Expression::Binary {
                                    op: BinaryOp::StrictEq,
                                    left: Box::new(Expression::Identifier(val_var.clone())),
                                    right: Box::new(Expression::Literal(Literal::Undefined)),
                                }),
                                then_expr: def.clone(),
                                else_expr: Box::new(Expression::Identifier(val_var)),
                            };
                            self.desugar_assignment_destructuring(
                                &Expression::Identifier(target.clone()),
                                cond_expr,
                                out,
                            );
                        }
                        Expression::MemberAssignment { object, property, value: def } => {
                            let elem_rhs = Expression::Index {
                                object: Box::new(Expression::Identifier(tmp_var.clone())),
                                index: Box::new(Expression::Literal(Literal::Number(idx as f64))),
                            };
                            let val_var = format!("@val_{}", self.destruct_counter);
                            self.destruct_counter += 1;
                            out.push(Statement::VarDecl {
                                kind: VarKind::Let,
                                declarations: vec![VarDeclarator {
                                    name: val_var.clone(),
                                    init: Some(elem_rhs),
                                }],
                            });
                            let cond_expr = Expression::Conditional {
                                condition: Box::new(Expression::Binary {
                                    op: BinaryOp::StrictEq,
                                    left: Box::new(Expression::Identifier(val_var.clone())),
                                    right: Box::new(Expression::Literal(Literal::Undefined)),
                                }),
                                then_expr: def.clone(),
                                else_expr: Box::new(Expression::Identifier(val_var)),
                            };
                            self.desugar_assignment_destructuring(
                                &Expression::Member { object: object.clone(), property: property.clone() },
                                cond_expr,
                                out,
                            );
                        }
                        Expression::IndexAssignment { object, index, value: def } => {
                            let elem_rhs = Expression::Index {
                                object: Box::new(Expression::Identifier(tmp_var.clone())),
                                index: Box::new(Expression::Literal(Literal::Number(idx as f64))),
                            };
                            let val_var = format!("@val_{}", self.destruct_counter);
                            self.destruct_counter += 1;
                            out.push(Statement::VarDecl {
                                kind: VarKind::Let,
                                declarations: vec![VarDeclarator {
                                    name: val_var.clone(),
                                    init: Some(elem_rhs),
                                }],
                            });
                            let cond_expr = Expression::Conditional {
                                condition: Box::new(Expression::Binary {
                                    op: BinaryOp::StrictEq,
                                    left: Box::new(Expression::Identifier(val_var.clone())),
                                    right: Box::new(Expression::Literal(Literal::Undefined)),
                                }),
                                then_expr: def.clone(),
                                else_expr: Box::new(Expression::Identifier(val_var)),
                            };
                            self.desugar_assignment_destructuring(
                                &Expression::Index { object: object.clone(), index: index.clone() },
                                cond_expr,
                                out,
                            );
                        }
                        _ => {
                            let elem_rhs = Expression::Index {
                                object: Box::new(Expression::Identifier(tmp_var.clone())),
                                index: Box::new(Expression::Literal(Literal::Number(idx as f64))),
                            };
                            self.desugar_assignment_destructuring(elem, elem_rhs, out);
                        }
                    }
                }
            }
            Expression::Object(properties) => {
                let tmp_var = format!("@dstr_obj_{}", self.destruct_counter);
                self.destruct_counter += 1;
                out.push(Statement::VarDecl {
                    kind: VarKind::Let,
                    declarations: vec![VarDeclarator {
                        name: tmp_var.clone(),
                        init: Some(rhs),
                    }],
                });
                let mut excluded_keys: Vec<Expression> = Vec::new();
                for prop in properties {
                    match prop {
                        ObjectProperty::Spread(inner) => {
                            let rest_rhs = Expression::ObjectRest {
                                target: Box::new(Expression::Identifier(tmp_var.clone())),
                                excluded_keys: excluded_keys.clone(),
                            };
                            self.desugar_assignment_destructuring(inner, rest_rhs, out);
                        }
                        ObjectProperty::KeyValue(key, value) => {
                            let (prop_rhs, key_expr_for_excluded) = match key {
                                PropertyKey::Literal(k) => (
                                    Expression::Member {
                                        object: Box::new(Expression::Identifier(tmp_var.clone())),
                                        property: k.clone(),
                                    },
                                    Expression::Literal(Literal::String {
                                        value: k.clone(),
                                        has_escape: false,
                                        has_legacy_escape: false,
                                    }),
                                ),
                                PropertyKey::Computed(k_expr) => {
                                    let key_var = format!("@dstr_k_{}", self.destruct_counter);
                                    self.destruct_counter += 1;
                                    out.push(Statement::VarDecl {
                                        kind: VarKind::Let,
                                        declarations: vec![VarDeclarator {
                                            name: key_var.clone(),
                                            init: Some(k_expr.clone()),
                                        }],
                                    });
                                    (
                                        Expression::Index {
                                            object: Box::new(Expression::Identifier(tmp_var.clone())),
                                            index: Box::new(Expression::Identifier(key_var.clone())),
                                        },
                                        Expression::Identifier(key_var),
                                    )
                                }
                            };
                            excluded_keys.push(key_expr_for_excluded);
                            match value {
                                Expression::Assignment { target, value: def } => {
                                    let val_var = format!("@val_{}", self.destruct_counter);
                                    self.destruct_counter += 1;
                                    out.push(Statement::VarDecl {
                                        kind: VarKind::Let,
                                        declarations: vec![VarDeclarator {
                                            name: val_var.clone(),
                                            init: Some(prop_rhs),
                                        }],
                                    });
                                    let cond_expr = Expression::Conditional {
                                        condition: Box::new(Expression::Binary {
                                            op: BinaryOp::StrictEq,
                                            left: Box::new(Expression::Identifier(val_var.clone())),
                                            right: Box::new(Expression::Literal(Literal::Undefined)),
                                        }),
                                        then_expr: def.clone(),
                                        else_expr: Box::new(Expression::Identifier(val_var)),
                                    };
                                    self.desugar_assignment_destructuring(
                                        &Expression::Identifier(target.clone()),
                                        cond_expr,
                                        out,
                                    );
                                }
                                Expression::MemberAssignment { object, property, value: def } => {
                                    let val_var = format!("@val_{}", self.destruct_counter);
                                    self.destruct_counter += 1;
                                    out.push(Statement::VarDecl {
                                        kind: VarKind::Let,
                                        declarations: vec![VarDeclarator {
                                            name: val_var.clone(),
                                            init: Some(prop_rhs),
                                        }],
                                    });
                                    let cond_expr = Expression::Conditional {
                                        condition: Box::new(Expression::Binary {
                                            op: BinaryOp::StrictEq,
                                            left: Box::new(Expression::Identifier(val_var.clone())),
                                            right: Box::new(Expression::Literal(Literal::Undefined)),
                                        }),
                                        then_expr: def.clone(),
                                        else_expr: Box::new(Expression::Identifier(val_var)),
                                    };
                                    self.desugar_assignment_destructuring(
                                        &Expression::Member { object: object.clone(), property: property.clone() },
                                        cond_expr,
                                        out,
                                    );
                                }
                                Expression::IndexAssignment { object, index, value: def } => {
                                    let val_var = format!("@val_{}", self.destruct_counter);
                                    self.destruct_counter += 1;
                                    out.push(Statement::VarDecl {
                                        kind: VarKind::Let,
                                        declarations: vec![VarDeclarator {
                                            name: val_var.clone(),
                                            init: Some(prop_rhs),
                                        }],
                                    });
                                    let cond_expr = Expression::Conditional {
                                        condition: Box::new(Expression::Binary {
                                            op: BinaryOp::StrictEq,
                                            left: Box::new(Expression::Identifier(val_var.clone())),
                                            right: Box::new(Expression::Literal(Literal::Undefined)),
                                        }),
                                        then_expr: def.clone(),
                                        else_expr: Box::new(Expression::Identifier(val_var)),
                                    };
                                    self.desugar_assignment_destructuring(
                                        &Expression::Index { object: object.clone(), index: index.clone() },
                                        cond_expr,
                                        out,
                                    );
                                }
                                _ => {
                                    self.desugar_assignment_destructuring(value, prop_rhs, out);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}
