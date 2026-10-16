use super::{ParseError, Parser};
use crate::ast::*;
use crate::lexer::TokenKind;

impl<'a> Parser<'a> {
    pub(crate) fn parse_array_destructuring_into(
        &mut self,
        tmp_name: &str,
        declarations: &mut Vec<VarDeclarator>,
    ) -> Result<(), ParseError> {
        let mut names = Vec::new();
        while !self.check(&TokenKind::RBracket) && self.current.kind != TokenKind::Eof {
            if self.check(&TokenKind::Comma) {
                self.advance()?;
                continue;
            }
            if self.match_token(&TokenKind::DotDotDot)? {
                let id = self.expect_identifier()?;
                names.push((id, true, None));
                break;
            }
            let id = self.expect_identifier()?;
            let def = if self.match_token(&TokenKind::Eq)? {
                Some(self.parse_expression(1)?)
            } else {
                None
            };
            names.push((id, false, def));
            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RBracket)?;
        for (idx, (n, is_rest, def)) in names.into_iter().enumerate() {
            let elem_init = if is_rest {
                Expression::Call {
                    callee: Box::new(Expression::Member {
                        object: Box::new(Expression::Identifier(tmp_name.to_string())),
                        property: "slice".to_string(),
                    }),
                    args: vec![Expression::Literal(Literal::Number(idx as f64))],
                }
            } else {
                Expression::Index {
                    object: Box::new(Expression::Identifier(tmp_name.to_string())),
                    index: Box::new(Expression::Literal(Literal::Number(idx as f64))),
                }
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
            declarations.push(VarDeclarator { name: n, init: Some(final_init) });
        }
        Ok(())
    }

    pub(crate) fn parse_object_destructuring_into(
        &mut self,
        tmp_name: &str,
        declarations: &mut Vec<VarDeclarator>,
    ) -> Result<(), ParseError> {
        let mut props = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            if self.check(&TokenKind::Comma) {
                self.advance()?;
                continue;
            }
            let (key_expr, key_str, target, default_val) = if self.match_token(&TokenKind::LBracket)? {
                let k = self.parse_expression(1)?;
                self.expect(TokenKind::RBracket)?;
                self.expect(TokenKind::Colon)?;
                let t = self.expect_identifier()?;
                let def = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                (Some(k), None, t, def)
            } else {
                let key = self.expect_identifier()?;
                let (t, def) = if self.match_token(&TokenKind::Colon)? {
                    let target_id = self.expect_identifier()?;
                    let def = if self.match_token(&TokenKind::Eq)? {
                        Some(self.parse_expression(1)?)
                    } else {
                        None
                    };
                    (target_id, def)
                } else {
                    let def = if self.match_token(&TokenKind::Eq)? {
                        Some(self.parse_expression(1)?)
                    } else {
                        None
                    };
                    (key.clone(), def)
                };
                (None, Some(key), t, def)
            };
            props.push((key_expr, key_str, target, default_val));
            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RBrace)?;
        for (key_expr, key_str, target, default_val) in props {
            let prop_init = if let Some(k) = key_expr {
                let key_var = format!("@key_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let key_to_string_expr = Expression::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(k),
                    right: Box::new(Expression::Literal(Literal::String { value: "".to_string(), has_escape: false, has_legacy_escape: false })),
                };
                declarations.push(VarDeclarator { name: key_var.clone(), init: Some(key_to_string_expr) });
                let touch_var = format!("@touch_{}", self.destruct_counter);
                self.destruct_counter += 1;
                declarations.push(VarDeclarator { name: touch_var, init: Some(Expression::Identifier(target.clone())) });
                let val_var = format!("@val_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let index_expr = Expression::Index {
                    object: Box::new(Expression::Identifier(tmp_name.to_string())),
                    index: Box::new(Expression::Identifier(key_var)),
                };
                declarations.push(VarDeclarator { name: val_var.clone(), init: Some(index_expr) });
                if let Some(def) = default_val {
                    Expression::Conditional {
                        condition: Box::new(Expression::Binary {
                            op: BinaryOp::StrictEq,
                            left: Box::new(Expression::Identifier(val_var.clone())),
                            right: Box::new(Expression::Literal(Literal::Undefined)),
                        }),
                        then_expr: Box::new(def),
                        else_expr: Box::new(Expression::Identifier(val_var)),
                    }
                } else {
                    Expression::Identifier(val_var)
                }
            } else {
                let key = key_str.unwrap();
                let base_prop = Expression::Member {
                    object: Box::new(Expression::Identifier(tmp_name.to_string())),
                    property: key,
                };
                if let Some(def) = default_val {
                    let val_var = format!("@val_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    declarations.push(VarDeclarator { name: val_var.clone(), init: Some(base_prop) });
                    Expression::Conditional {
                        condition: Box::new(Expression::Binary {
                            op: BinaryOp::StrictEq,
                            left: Box::new(Expression::Identifier(val_var.clone())),
                            right: Box::new(Expression::Literal(Literal::Undefined)),
                        }),
                        then_expr: Box::new(def),
                        else_expr: Box::new(Expression::Identifier(val_var)),
                    }
                } else {
                    base_prop
                }
            };
            declarations.push(VarDeclarator { name: target, init: Some(prop_init) });
        }
        Ok(())
    }
}
