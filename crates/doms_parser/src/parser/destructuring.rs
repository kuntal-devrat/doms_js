use super::{ParseError, Parser};
use crate::ast::*;
use crate::lexer::TokenKind;

fn build_get_iterator_expr(iterable: Expression) -> Expression {
    Expression::Call {
        callee: Box::new(Expression::Index {
            object: Box::new(iterable),
            index: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("Symbol".to_string())),
                property: "iterator".to_string(),
            }),
        }),
        args: vec![],
    }
}

fn build_iterator_step_expr(iter_name: &str) -> Expression {
    Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier(iter_name.to_string())),
            property: "next".to_string(),
        }),
        args: vec![],
    }
}

fn build_iterator_close_expr(iter_name: &str) -> Expression {
    let ret_decl = Statement::VarDecl {
        kind: VarKind::Let,
        declarations: vec![VarDeclarator {
            name: "ret".to_string(),
            init: Some(Expression::Member {
                object: Box::new(Expression::Identifier("iter".to_string())),
                property: "return".to_string(),
            }),
        }],
    };
    let if_nullish = Statement::If {
        condition: Expression::Binary {
            op: BinaryOp::LogicalOr,
            left: Box::new(Expression::Binary {
                op: BinaryOp::StrictEq,
                left: Box::new(Expression::Identifier("ret".to_string())),
                right: Box::new(Expression::Literal(Literal::Undefined)),
            }),
            right: Box::new(Expression::Binary {
                op: BinaryOp::StrictEq,
                left: Box::new(Expression::Identifier("ret".to_string())),
                right: Box::new(Expression::Literal(Literal::Null)),
            }),
        },
        then_branch: Box::new(Statement::Return(None)),
        else_branch: None,
    };
    let if_not_func = Statement::If {
        condition: Expression::Binary {
            op: BinaryOp::StrictNotEq,
            left: Box::new(Expression::Unary {
                op: UnaryOp::Typeof,
                expr: Box::new(Expression::Identifier("ret".to_string())),
            }),
            right: Box::new(Expression::Literal(Literal::String {
                value: "function".to_string(),
                has_escape: false,
                has_legacy_escape: false,
            })),
        },
        then_branch: Box::new(Statement::Throw(Expression::New {
            callee: Box::new(Expression::Identifier("TypeError".to_string())),
            args: vec![Expression::Literal(Literal::String {
                value: "Iterator return is not callable".to_string(),
                has_escape: false,
                has_legacy_escape: false,
            })],
        })),
        else_branch: None,
    };
    let res_decl = Statement::VarDecl {
        kind: VarKind::Let,
        declarations: vec![VarDeclarator {
            name: "res".to_string(),
            init: Some(Expression::Call {
                callee: Box::new(Expression::Member {
                    object: Box::new(Expression::Identifier("ret".to_string())),
                    property: "call".to_string(),
                }),
                args: vec![Expression::Identifier("iter".to_string())],
            }),
        }],
    };
    let if_not_obj = Statement::If {
        condition: Expression::Binary {
            op: BinaryOp::LogicalOr,
            left: Box::new(Expression::Binary {
                op: BinaryOp::LogicalAnd,
                left: Box::new(Expression::Binary {
                    op: BinaryOp::StrictNotEq,
                    left: Box::new(Expression::Unary {
                        op: UnaryOp::Typeof,
                        expr: Box::new(Expression::Identifier("res".to_string())),
                    }),
                    right: Box::new(Expression::Literal(Literal::String {
                        value: "object".to_string(),
                        has_escape: false,
                        has_legacy_escape: false,
                    })),
                }),
                right: Box::new(Expression::Binary {
                    op: BinaryOp::StrictNotEq,
                    left: Box::new(Expression::Unary {
                        op: UnaryOp::Typeof,
                        expr: Box::new(Expression::Identifier("res".to_string())),
                    }),
                    right: Box::new(Expression::Literal(Literal::String {
                        value: "function".to_string(),
                        has_escape: false,
                        has_legacy_escape: false,
                    })),
                }),
            }),
            right: Box::new(Expression::Binary {
                op: BinaryOp::StrictEq,
                left: Box::new(Expression::Identifier("res".to_string())),
                right: Box::new(Expression::Literal(Literal::Null)),
            }),
        },
        then_branch: Box::new(Statement::Throw(Expression::New {
            callee: Box::new(Expression::Identifier("TypeError".to_string())),
            args: vec![Expression::Literal(Literal::String {
                value: "Iterator return did not return an object".to_string(),
                has_escape: false,
                has_legacy_escape: false,
            })],
        })),
        else_branch: None,
    };

    Expression::Call {
        callee: Box::new(Expression::ArrowExpr {
            params: vec!["iter".to_string()],
            body: vec![ret_decl, if_nullish, if_not_func, res_decl, if_not_obj],
        }),
        args: vec![Expression::Identifier(iter_name.to_string())],
    }
}

fn build_iterator_rest_expr(
    iter_name: &str,
    prev_step_done: Option<&str>,
    step_active_var: Option<&str>,
) -> Expression {
    let mut body = Vec::new();
    body.push(Statement::VarDecl {
        kind: VarKind::Let,
        declarations: vec![
            VarDeclarator {
                name: "a".to_string(),
                init: Some(Expression::Array(vec![])),
            },
            VarDeclarator {
                name: "s".to_string(),
                init: None,
            },
        ],
    });

    let mut loop_body = Vec::new();
    if let Some(active) = step_active_var {
        loop_body.push(Statement::Expression(Expression::Assignment {
            target: active.to_string(),
            value: Box::new(Expression::Literal(Literal::Boolean(true))),
        }));
    }
    loop_body.push(Statement::Expression(Expression::Assignment {
        target: "s".to_string(),
        value: Box::new(Expression::Call {
            callee: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("iter".to_string())),
                property: "next".to_string(),
            }),
            args: vec![],
        }),
    }));
    if let Some(active) = step_active_var {
        loop_body.push(Statement::Expression(Expression::Assignment {
            target: active.to_string(),
            value: Box::new(Expression::Literal(Literal::Boolean(false))),
        }));
    }
    loop_body.push(Statement::If {
        condition: Expression::Member {
            object: Box::new(Expression::Identifier("s".to_string())),
            property: "done".to_string(),
        },
        then_branch: Box::new(Statement::Break(None)),
        else_branch: None,
    });
    loop_body.push(Statement::Expression(Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier("a".to_string())),
            property: "push".to_string(),
        }),
        args: vec![Expression::Member {
            object: Box::new(Expression::Identifier("s".to_string())),
            property: "value".to_string(),
        }],
    }));

    body.push(Statement::While {
        condition: Expression::Literal(Literal::Boolean(true)),
        body: Box::new(Statement::Block(loop_body)),
    });
    body.push(Statement::Return(Some(Expression::Identifier("a".to_string()))));

    let rest_call = Expression::Call {
        callee: Box::new(Expression::ArrowExpr {
            params: vec!["iter".to_string()],
            body,
        }),
        args: vec![Expression::Identifier(iter_name.to_string())],
    };

    if let Some(prev) = prev_step_done {
        Expression::Conditional {
            condition: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier(prev.to_string())),
                property: "done".to_string(),
            }),
            then_expr: Box::new(Expression::Array(vec![])),
            else_expr: Box::new(rest_call),
        }
    } else {
        rest_call
    }
}

impl<'a> Parser<'a> {
    pub(crate) fn parse_array_destructuring_into(
        &mut self,
        tmp_name: &str,
        declarations: &mut Vec<VarDeclarator>,
    ) -> Result<(), ParseError> {
        let iter_name = format!("@iter_{}", self.destruct_counter);
        self.destruct_counter += 1;
        declarations.push(VarDeclarator {
            name: iter_name.clone(),
            init: Some(build_get_iterator_expr(Expression::Identifier(tmp_name.to_string()))),
        });

        let mut prev_step: Option<String> = None;
        let mut has_rest = false;

        while !self.check(&TokenKind::RBracket) && self.current.kind != TokenKind::Eof {
            if self.check(&TokenKind::Comma) {
                self.advance()?;
                let step_name = format!("@step_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let step_init = if let Some(ref p) = prev_step {
                    Expression::Conditional {
                        condition: Box::new(Expression::Member {
                            object: Box::new(Expression::Identifier(p.clone())),
                            property: "done".to_string(),
                        }),
                        then_expr: Box::new(Expression::Object(vec![ObjectProperty::KeyValue(
                            PropertyKey::Literal("done".to_string()),
                            Expression::Literal(Literal::Boolean(true)),
                        )])),
                        else_expr: Box::new(build_iterator_step_expr(&iter_name)),
                    }
                } else {
                    build_iterator_step_expr(&iter_name)
                };
                declarations.push(VarDeclarator {
                    name: step_name.clone(),
                    init: Some(step_init),
                });
                prev_step = Some(step_name);
                continue;
            }

            if self.match_token(&TokenKind::DotDotDot)? {
                has_rest = true;
                let rest_init = build_iterator_rest_expr(&iter_name, prev_step.as_deref(), None);
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
                    declarations.push(VarDeclarator { name: id, init: Some(rest_init) });
                }
                break;
            }

            let step_name = format!("@step_{}", self.destruct_counter);
            self.destruct_counter += 1;
            let step_init = if let Some(ref p) = prev_step {
                Expression::Conditional {
                    condition: Box::new(Expression::Member {
                        object: Box::new(Expression::Identifier(p.clone())),
                        property: "done".to_string(),
                    }),
                    then_expr: Box::new(Expression::Object(vec![ObjectProperty::KeyValue(
                        PropertyKey::Literal("done".to_string()),
                        Expression::Literal(Literal::Boolean(true)),
                    )])),
                    else_expr: Box::new(build_iterator_step_expr(&iter_name)),
                }
            } else {
                build_iterator_step_expr(&iter_name)
            };
            declarations.push(VarDeclarator {
                name: step_name.clone(),
                init: Some(step_init),
            });
            prev_step = Some(step_name.clone());

            let elem_init = Expression::Conditional {
                condition: Box::new(Expression::Member {
                    object: Box::new(Expression::Identifier(step_name.clone())),
                    property: "done".to_string(),
                }),
                then_expr: Box::new(Expression::Literal(Literal::Undefined)),
                else_expr: Box::new(Expression::Member {
                    object: Box::new(Expression::Identifier(step_name)),
                    property: "value".to_string(),
                }),
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

            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RBracket)?;

        if !has_rest {
            let close_expr = if let Some(ref p) = prev_step {
                Expression::Conditional {
                    condition: Box::new(Expression::Unary {
                        op: UnaryOp::Not,
                        expr: Box::new(Expression::Member {
                            object: Box::new(Expression::Identifier(p.clone())),
                            property: "done".to_string(),
                        }),
                    }),
                    then_expr: Box::new(build_iterator_close_expr(&iter_name)),
                    else_expr: Box::new(Expression::Literal(Literal::Undefined)),
                }
            } else {
                build_iterator_close_expr(&iter_name)
            };
            let close_name = format!("@close_{}", self.destruct_counter);
            self.destruct_counter += 1;
            declarations.push(VarDeclarator {
                name: close_name,
                init: Some(close_expr),
            });
        }

        Ok(())
    }

    pub(crate) fn parse_object_destructuring_into(
        &mut self,
        tmp_name: &str,
        declarations: &mut Vec<VarDeclarator>,
    ) -> Result<(), ParseError> {
        let chk_name = format!("@destruct_chk_{}", self.destruct_counter);
        self.destruct_counter += 1;
        declarations.push(VarDeclarator {
            name: chk_name,
            init: Some(Expression::Identifier(tmp_name.to_string())),
        });
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
                let iter_name = format!("@dstr_iter_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let done_var = format!("@done_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let has_err_var = format!("@has_err_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let step_active_var = format!("@step_active_{}", self.destruct_counter);
                self.destruct_counter += 1;

                out.push(Statement::VarDecl {
                    kind: VarKind::Let,
                    declarations: vec![
                        VarDeclarator {
                            name: iter_name.clone(),
                            init: Some(build_get_iterator_expr(rhs)),
                        },
                        VarDeclarator {
                            name: done_var.clone(),
                            init: Some(Expression::Literal(Literal::Boolean(false))),
                        },
                        VarDeclarator {
                            name: has_err_var.clone(),
                            init: Some(Expression::Literal(Literal::Boolean(false))),
                        },
                        VarDeclarator {
                            name: step_active_var.clone(),
                            init: Some(Expression::Literal(Literal::Boolean(false))),
                        },
                    ],
                });

                let mut try_stmts = Vec::new();
                let mut prev_step: Option<String> = None;
                let mut has_rest = false;

                for elem in elements {
                    match elem {
                        Expression::Literal(Literal::Hole) => {
                            let step_name = format!("@step_{}", self.destruct_counter);
                            self.destruct_counter += 1;
                            let step_init = if let Some(ref p) = prev_step {
                                Expression::Conditional {
                                    condition: Box::new(Expression::Member {
                                        object: Box::new(Expression::Identifier(p.clone())),
                                        property: "done".to_string(),
                                    }),
                                    then_expr: Box::new(Expression::Object(vec![ObjectProperty::KeyValue(
                                        PropertyKey::Literal("done".to_string()),
                                        Expression::Literal(Literal::Boolean(true)),
                                    )])),
                                    else_expr: Box::new(build_iterator_step_expr(&iter_name)),
                                }
                            } else {
                                build_iterator_step_expr(&iter_name)
                            };
                            try_stmts.push(Statement::Expression(Expression::Assignment {
                                target: step_active_var.clone(),
                                value: Box::new(Expression::Literal(Literal::Boolean(true))),
                            }));
                            try_stmts.push(Statement::VarDecl {
                                kind: VarKind::Let,
                                declarations: vec![VarDeclarator {
                                    name: step_name.clone(),
                                    init: Some(step_init),
                                }],
                            });
                            try_stmts.push(Statement::Expression(Expression::Assignment {
                                target: step_active_var.clone(),
                                value: Box::new(Expression::Literal(Literal::Boolean(false))),
                            }));
                            prev_step = Some(step_name);
                        }
                        Expression::Spread(inner) => {
                            has_rest = true;
                            let target_inner = match &**inner {
                                Expression::Member { object, property } => {
                                    let base_var = format!("@base_{}", self.destruct_counter);
                                    self.destruct_counter += 1;
                                    try_stmts.push(Statement::VarDecl {
                                        kind: VarKind::Let,
                                        declarations: vec![VarDeclarator {
                                            name: base_var.clone(),
                                            init: Some(Expression::Literal(Literal::Undefined)),
                                        }],
                                    });
                                    try_stmts.push(Statement::Expression(Expression::Assignment {
                                        target: base_var.clone(),
                                        value: object.clone(),
                                    }));
                                    Expression::Member {
                                        object: Box::new(Expression::Identifier(base_var)),
                                        property: property.clone(),
                                    }
                                }
                                Expression::Index { object, index } => {
                                    let base_var = format!("@base_{}", self.destruct_counter);
                                    self.destruct_counter += 1;
                                    let key_var = format!("@key_{}", self.destruct_counter);
                                    self.destruct_counter += 1;
                                    try_stmts.push(Statement::VarDecl {
                                        kind: VarKind::Let,
                                        declarations: vec![
                                            VarDeclarator { name: base_var.clone(), init: Some(Expression::Literal(Literal::Undefined)) },
                                            VarDeclarator { name: key_var.clone(), init: Some(Expression::Literal(Literal::Undefined)) },
                                        ],
                                    });
                                    try_stmts.push(Statement::Expression(Expression::Assignment {
                                        target: base_var.clone(),
                                        value: object.clone(),
                                    }));
                                    try_stmts.push(Statement::Expression(Expression::Assignment {
                                        target: key_var.clone(),
                                        value: index.clone(),
                                    }));
                                    Expression::Index {
                                        object: Box::new(Expression::Identifier(base_var)),
                                        index: Box::new(Expression::Identifier(key_var)),
                                    }
                                }
                                other => other.clone(),
                            };
                            let rest_rhs = build_iterator_rest_expr(&iter_name, prev_step.as_deref(), Some(&step_active_var));
                            let rest_val_var = format!("@rest_val_{}", self.destruct_counter);
                            self.destruct_counter += 1;
                            try_stmts.push(Statement::VarDecl {
                                kind: VarKind::Let,
                                declarations: vec![VarDeclarator {
                                    name: rest_val_var.clone(),
                                    init: Some(rest_rhs),
                                }],
                            });
                            try_stmts.push(Statement::Expression(Expression::Assignment {
                                target: done_var.clone(),
                                value: Box::new(Expression::Literal(Literal::Boolean(true))),
                            }));
                            self.desugar_assignment_destructuring(&target_inner, Expression::Identifier(rest_val_var), &mut try_stmts);
                            break;
                        }
                        _ => {
                            let (target_expr, maybe_def) = match elem {
                                Expression::Assignment { target, value: def } => {
                                    (Expression::Identifier(target.clone()), Some(def.clone()))
                                }
                                Expression::MemberAssignment { object, property, value: def } => {
                                    (Expression::Member { object: object.clone(), property: property.clone() }, Some(def.clone()))
                                }
                                Expression::IndexAssignment { object, index, value: def } => {
                                    (Expression::Index { object: object.clone(), index: index.clone() }, Some(def.clone()))
                                }
                                Expression::DestructuringAssignment { pattern, value: def } => {
                                    ((**pattern).clone(), Some(def.clone()))
                                }
                                other => (other.clone(), None),
                            };

                            let target_expr = match target_expr {
                                Expression::Member { object, property } => {
                                    let base_var = format!("@base_{}", self.destruct_counter);
                                    self.destruct_counter += 1;
                                    try_stmts.push(Statement::VarDecl {
                                        kind: VarKind::Let,
                                        declarations: vec![VarDeclarator {
                                            name: base_var.clone(),
                                            init: Some(Expression::Literal(Literal::Undefined)),
                                        }],
                                    });
                                    try_stmts.push(Statement::Expression(Expression::Assignment {
                                        target: base_var.clone(),
                                        value: object,
                                    }));
                                    Expression::Member {
                                        object: Box::new(Expression::Identifier(base_var)),
                                        property,
                                    }
                                }
                                Expression::Index { object, index } => {
                                    let base_var = format!("@base_{}", self.destruct_counter);
                                    self.destruct_counter += 1;
                                    let key_var = format!("@key_{}", self.destruct_counter);
                                    self.destruct_counter += 1;
                                    try_stmts.push(Statement::VarDecl {
                                        kind: VarKind::Let,
                                        declarations: vec![
                                            VarDeclarator { name: base_var.clone(), init: Some(Expression::Literal(Literal::Undefined)) },
                                            VarDeclarator { name: key_var.clone(), init: Some(Expression::Literal(Literal::Undefined)) },
                                        ],
                                    });
                                    try_stmts.push(Statement::Expression(Expression::Assignment {
                                        target: base_var.clone(),
                                        value: object,
                                    }));
                                    try_stmts.push(Statement::Expression(Expression::Assignment {
                                        target: key_var.clone(),
                                        value: index,
                                    }));
                                    Expression::Index {
                                        object: Box::new(Expression::Identifier(base_var)),
                                        index: Box::new(Expression::Identifier(key_var)),
                                    }
                                }
                                other => other,
                            };

                            let step_name = format!("@step_{}", self.destruct_counter);
                            self.destruct_counter += 1;
                            let step_init = if let Some(ref p) = prev_step {
                                Expression::Conditional {
                                    condition: Box::new(Expression::Member {
                                        object: Box::new(Expression::Identifier(p.clone())),
                                        property: "done".to_string(),
                                    }),
                                    then_expr: Box::new(Expression::Object(vec![ObjectProperty::KeyValue(
                                        PropertyKey::Literal("done".to_string()),
                                        Expression::Literal(Literal::Boolean(true)),
                                    )])),
                                    else_expr: Box::new(build_iterator_step_expr(&iter_name)),
                                }
                            } else {
                                build_iterator_step_expr(&iter_name)
                            };
                            try_stmts.push(Statement::Expression(Expression::Assignment {
                                target: step_active_var.clone(),
                                value: Box::new(Expression::Literal(Literal::Boolean(true))),
                            }));
                            try_stmts.push(Statement::VarDecl {
                                kind: VarKind::Let,
                                declarations: vec![VarDeclarator {
                                    name: step_name.clone(),
                                    init: Some(step_init),
                                }],
                            });
                            try_stmts.push(Statement::Expression(Expression::Assignment {
                                target: step_active_var.clone(),
                                value: Box::new(Expression::Literal(Literal::Boolean(false))),
                            }));
                            prev_step = Some(step_name.clone());

                            let elem_rhs = Expression::Conditional {
                                condition: Box::new(Expression::Member {
                                    object: Box::new(Expression::Identifier(step_name.clone())),
                                    property: "done".to_string(),
                                }),
                                then_expr: Box::new(Expression::Literal(Literal::Undefined)),
                                else_expr: Box::new(Expression::Member {
                                    object: Box::new(Expression::Identifier(step_name)),
                                    property: "value".to_string(),
                                }),
                            };

                            let final_elem_rhs = if let Some(def) = maybe_def {
                                let val_var = format!("@val_{}", self.destruct_counter);
                                self.destruct_counter += 1;
                                let def_var = if let Expression::Identifier(ref name) = target_expr {
                                    format!("@name:{}:{}", name, self.destruct_counter)
                                } else {
                                    format!("@def_{}", self.destruct_counter)
                                };
                                self.destruct_counter += 1;
                                try_stmts.push(Statement::VarDecl {
                                    kind: VarKind::Let,
                                    declarations: vec![
                                        VarDeclarator {
                                            name: val_var.clone(),
                                            init: Some(elem_rhs),
                                        },
                                        VarDeclarator {
                                            name: def_var.clone(),
                                            init: Some(Expression::Literal(Literal::Undefined)),
                                        },
                                    ],
                                });
                                try_stmts.push(Statement::If {
                                    condition: Expression::Binary {
                                        op: BinaryOp::StrictEq,
                                        left: Box::new(Expression::Identifier(val_var.clone())),
                                        right: Box::new(Expression::Literal(Literal::Undefined)),
                                    },
                                    then_branch: Box::new(Statement::Expression(Expression::Assignment {
                                        target: def_var.clone(),
                                        value: def,
                                    })),
                                    else_branch: None,
                                });
                                Expression::Conditional {
                                    condition: Box::new(Expression::Binary {
                                        op: BinaryOp::StrictEq,
                                        left: Box::new(Expression::Identifier(val_var.clone())),
                                        right: Box::new(Expression::Literal(Literal::Undefined)),
                                    }),
                                    then_expr: Box::new(Expression::Identifier(def_var)),
                                    else_expr: Box::new(Expression::Identifier(val_var)),
                                }
                            } else {
                                elem_rhs
                            };

                            self.desugar_assignment_destructuring(&target_expr, final_elem_rhs, &mut try_stmts);
                        }
                    }
                }

                if !has_rest {
                    if let Some(ref p) = prev_step {
                        try_stmts.push(Statement::Expression(Expression::Assignment {
                            target: done_var.clone(),
                            value: Box::new(Expression::Member {
                                object: Box::new(Expression::Identifier(p.clone())),
                                property: "done".to_string(),
                            }),
                        }));
                    }
                }

                let close_call_normal = Statement::Expression(build_iterator_close_expr(&iter_name));
                let close_call_error = Statement::TryCatch {
                    try_block: Box::new(Statement::Block(vec![Statement::Expression(build_iterator_close_expr(&iter_name))])),
                    catch_param: Some("_".to_string()),
                    catch_block: Some(Box::new(Statement::Block(vec![]))),
                    finally_block: None,
                };
                let finally_if = Statement::If {
                    condition: Expression::Binary {
                        op: BinaryOp::LogicalAnd,
                        left: Box::new(Expression::Unary {
                            op: UnaryOp::Not,
                            expr: Box::new(Expression::Identifier(done_var)),
                        }),
                        right: Box::new(Expression::Unary {
                            op: UnaryOp::Not,
                            expr: Box::new(Expression::Identifier(step_active_var)),
                        }),
                    },
                    then_branch: Box::new(Statement::If {
                        condition: Expression::Identifier(has_err_var.clone()),
                        then_branch: Box::new(close_call_error),
                        else_branch: Some(Box::new(close_call_normal)),
                    }),
                    else_branch: None,
                };
                let catch_stmts = vec![
                    Statement::Expression(Expression::Assignment {
                        target: has_err_var,
                        value: Box::new(Expression::Literal(Literal::Boolean(true))),
                    }),
                    Statement::Throw(Expression::Identifier("@err_dstr".to_string())),
                ];

                out.push(Statement::TryCatch {
                    try_block: Box::new(Statement::Block(try_stmts)),
                    catch_param: Some("@err_dstr".to_string()),
                    catch_block: Some(Box::new(Statement::Block(catch_stmts))),
                    finally_block: Some(Box::new(Statement::Block(vec![finally_if]))),
                });
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
                                Expression::DestructuringAssignment { pattern, value: def } => {
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
                                        pattern,
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
