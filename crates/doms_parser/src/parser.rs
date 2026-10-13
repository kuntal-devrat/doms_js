//! Pratt parser for JavaScript syntax.

use crate::ast::*;
use crate::lexer::{LexError, Lexer, Token, TokenKind};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("Lexer error: {0}")]
    Lex(#[from] LexError),
    #[error("Unexpected token {0:?} at line {1}, col {2}")]
    UnexpectedToken(TokenKind, usize, usize),
    #[error("Unexpected EOF")]
    UnexpectedEof,
    #[error("Invalid assignment target at line {0}, col {1}")]
    InvalidAssignmentTarget(usize, usize),
    #[error("Syntax error: {0}")]
    General(String),
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
    peeked: Token,
    destruct_counter: usize,
    pub is_module: bool,
    pub is_strict: bool,
    pub is_top_level: bool,
    pub in_generator: bool,
    pub in_function: bool,
    pub in_async: bool,
    pub in_non_arrow_function: bool,
    pub in_method: bool,
    pub in_derived_constructor: bool,
    pub allow_in: bool,
    pub in_param: bool,
    pub private_names_stack: Vec<std::collections::HashSet<String>>,
}

#[inline]
pub fn is_strict_reserved_word(s: &str) -> bool {
    matches!(
        s,
        "implements"
            | "interface"
            | "let"
            | "package"
            | "private"
            | "protected"
            | "public"
            | "static"
            | "yield"
    )
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token()?;
        let peeked = lexer.next_token()?;
        let is_strict = lexer.is_strict
            || matches!(&current.kind, TokenKind::StringLit { value, has_escape, .. } if value == "use strict" && !has_escape);
        lexer.is_strict = is_strict;
        Ok(Self {
            lexer,
            current,
            peeked,
            destruct_counter: 0,
            is_module: false,
            is_strict,
            is_top_level: true,
            in_generator: false,
            in_function: false,
            in_async: false,
            in_non_arrow_function: false,
            in_method: false,
            in_derived_constructor: false,
            allow_in: true,
            in_param: false,
            private_names_stack: Vec::new(),
        })
    }

    pub fn new_module(source: &'a str) -> Result<Self, ParseError> {
        let mut p = Self::new_strict(source)?;
        p.is_module = true;
        Ok(p)
    }

    pub fn new_strict(source: &'a str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new_strict(source);
        let current = lexer.next_token()?;
        let peeked = lexer.next_token()?;
        Ok(Self {
            lexer,
            current,
            peeked,
            destruct_counter: 0,
            is_module: false,
            is_strict: true,
            is_top_level: true,
            in_generator: false,
            in_function: false,
            in_async: false,
            in_non_arrow_function: false,
            in_method: false,
            in_derived_constructor: false,
            allow_in: true,
            in_param: false,
            private_names_stack: Vec::new(),
        })
    }

    pub fn new_eval(
        source: &'a str,
        is_strict: bool,
        in_method: bool,
        in_function: bool,
    ) -> Result<Self, ParseError> {
        let mut p = if is_strict {
            Self::new_strict(source)?
        } else {
            Self::new(source)?
        };
        p.in_method = in_method;
        p.in_function = in_function;
        Ok(p)
    }

    fn advance(&mut self) -> Result<Token, ParseError> {
        let prev = std::mem::replace(
            &mut self.current,
            std::mem::replace(&mut self.peeked, self.lexer.next_token()?),
        );
        Ok(prev)
    }

    fn check(&self, kind: &TokenKind) -> bool {
        &self.current.kind == kind
    }

    fn check_ident(&self, name: &str) -> bool {
        match &self.current.kind {
            TokenKind::Ident(s) => s == name,
            _ => false,
        }
    }

    fn match_token(&mut self, kind: &TokenKind) -> Result<bool, ParseError> {
        if self.check(kind) {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn consume_semicolon(&mut self) -> Result<(), ParseError> {
        if self.match_token(&TokenKind::Semicolon)? {
            return Ok(());
        }
        if self.current.has_preceding_line_terminator
            || self.current.kind == TokenKind::RBrace
            || self.current.kind == TokenKind::Eof
        {
            return Ok(());
        }
        Err(ParseError::UnexpectedToken(
            self.current.kind.clone(),
            self.current.line,
            self.current.col,
        ))
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        if self.current.kind == kind {
            self.advance()
        } else {
            Err(ParseError::UnexpectedToken(
                self.current.kind.clone(),
                self.current.line,
                self.current.col,
            ))
        }
    }

    fn expect_identifier(&mut self) -> Result<String, ParseError> {
        let tok = self.advance()?;
        let name = match tok.kind {
            TokenKind::Ident(ref id) => {
                if self.is_strict && (is_strict_reserved_word(id) || id == "eval" || id == "arguments") {
                    return Err(ParseError::UnexpectedToken(TokenKind::Ident(id.clone()), tok.line, tok.col));
                }
                id.clone()
            }
            TokenKind::Undefined => "undefined".to_string(),
            TokenKind::As => "as".to_string(),
            TokenKind::From => "from".to_string(),
            TokenKind::Async => "async".to_string(),
            TokenKind::Await => {
                if self.is_module || self.is_strict {
                    return Err(ParseError::UnexpectedToken(TokenKind::Await, tok.line, tok.col));
                }
                "await".to_string()
            }
            TokenKind::Yield => {
                if self.is_strict {
                    return Err(ParseError::UnexpectedToken(TokenKind::Yield, tok.line, tok.col));
                }
                "yield".to_string()
            }
            TokenKind::Implements
            | TokenKind::Interface
            | TokenKind::Package
            | TokenKind::Private
            | TokenKind::Protected
            | TokenKind::Public
            | TokenKind::Static => {
                if self.is_strict {
                    return Err(ParseError::UnexpectedToken(tok.kind, tok.line, tok.col));
                }
                tok.kind.as_prop_name().unwrap_or_default()
            }
            TokenKind::Let => {
                if self.is_strict {
                    return Err(ParseError::UnexpectedToken(tok.kind, tok.line, tok.col));
                }
                "let".to_string()
            }
            other => return Err(ParseError::UnexpectedToken(other, tok.line, tok.col)),
        };
        Ok(name)
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut body = Vec::new();
        let mut in_directive_prologue = true;
        while self.current.kind != TokenKind::Eof {
            self.is_top_level = true;
            let stmt = self.parse_statement()?;
            if in_directive_prologue {
                if let Statement::Expression(Expression::Literal(Literal::String { ref value, has_escape, has_legacy_escape })) = stmt {
                    if has_legacy_escape && self.is_strict {
                        return Err(ParseError::UnexpectedToken(TokenKind::StringLit { value: value.clone(), has_escape, has_legacy_escape, has_unpaired_surrogate: false }, self.current.line, self.current.col));
                    }
                    if value == "use strict" && !has_escape {
                        self.is_strict = true;
                        self.lexer.is_strict = true;
                        if body.iter().any(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { has_legacy_escape: true, .. })))) {
                            return Err(ParseError::UnexpectedToken(TokenKind::StringLit { value: value.clone(), has_escape, has_legacy_escape, has_unpaired_surrogate: false }, self.current.line, self.current.col));
                        }
                    }
                } else {
                    in_directive_prologue = false;
                }
            }
            body.push(stmt);
        }
        if self.is_module {
            self.validate_module_early_errors(&body)?;
        }
        Ok(Program { body })
    }

    pub fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        let is_top = self.is_top_level;
        self.is_top_level = false;
        match self.current.kind {
            TokenKind::Class => self.parse_class_decl(),
            TokenKind::Let => self.parse_var_decl(VarKind::Let),
            TokenKind::Const => self.parse_var_decl(VarKind::Const),
            TokenKind::Var => self.parse_var_decl(VarKind::Var),
            TokenKind::Async => {
                self.advance()?;
                if self.check(&TokenKind::Function) {
                    self.parse_async_function_decl()
                } else {
                    let expr = self.parse_expression(0)?;
                    self.consume_semicolon()?;
                    Ok(Statement::Expression(expr))
                }
            }
            TokenKind::Function => {
                self.advance()?; // function
                if self.match_token(&TokenKind::Star)? {
                    self.parse_generator_decl()
                } else {
                    self.parse_function_decl_body()
                }
            }
            TokenKind::Import => {
                if self.peeked.kind == TokenKind::LParen {
                    let expr = self.parse_expression(0)?;
                    self.consume_semicolon()?;
                    Ok(Statement::Expression(expr))
                } else {
                    self.parse_import_statement(is_top)
                }
            }
            TokenKind::Export => self.parse_export_statement(is_top),
            TokenKind::If => self.parse_if_statement(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::Do => self.parse_do_while_statement(),
            TokenKind::For => self.parse_for_statement(),
            TokenKind::Switch => self.parse_switch_statement(),
            TokenKind::Break => {
                self.advance()?;
                let label = if let TokenKind::Ident(name) = &self.current.kind {
                    if !self.current.has_preceding_line_terminator {
                        let l = name.clone();
                        self.advance()?;
                        Some(l)
                    } else {
                        None
                    }
                } else {
                    None
                };
                self.consume_semicolon()?;
                Ok(Statement::Break(label))
            }
            TokenKind::Continue => {
                self.advance()?;
                let label = if let TokenKind::Ident(name) = &self.current.kind {
                    if !self.current.has_preceding_line_terminator {
                        let l = name.clone();
                        self.advance()?;
                        Some(l)
                    } else {
                        None
                    }
                } else {
                    None
                };
                self.consume_semicolon()?;
                Ok(Statement::Continue(label))
            }
            TokenKind::Try => self.parse_try_statement(),
            TokenKind::Throw => {
                self.advance()?;
                if self.current.has_preceding_line_terminator {
                    return Err(ParseError::UnexpectedToken(
                        self.current.kind.clone(),
                        self.current.line,
                        self.current.col,
                    ));
                }
                let expr = self.parse_expression(0)?;
                self.consume_semicolon()?;
                Ok(Statement::Throw(expr))
            }
            TokenKind::Return => self.parse_return_statement(),
            TokenKind::LBrace => self.parse_block_statement(),
            TokenKind::Semicolon => {
                self.advance()?;
                Ok(Statement::Empty)
            }
            TokenKind::Debugger => {
                self.advance()?;
                self.consume_semicolon()?;
                Ok(Statement::Debugger)
            }
            TokenKind::With => self.parse_with_statement(),
            ref tok if self.peeked.kind == TokenKind::Colon && match tok {
                TokenKind::Ident(_) => true,
                TokenKind::Implements
                | TokenKind::Interface
                | TokenKind::Package
                | TokenKind::Private
                | TokenKind::Protected
                | TokenKind::Public
                | TokenKind::Static
                | TokenKind::Yield
                | TokenKind::Await => true,
                _ => false,
            } => {
                let lbl = match &self.current.kind {
                    TokenKind::Ident(s) => s.clone(),
                    other => other.as_prop_name().unwrap_or_default(),
                };
                if self.is_strict && is_strict_reserved_word(&lbl) {
                    return Err(ParseError::UnexpectedToken(self.current.kind.clone(), self.current.line, self.current.col));
                }
                if (self.is_strict || self.is_module) && lbl == "await" {
                    return Err(ParseError::UnexpectedToken(TokenKind::Await, self.current.line, self.current.col));
                }
                self.advance()?; // ident/keyword
                self.advance()?; // colon
                let stmt = self.parse_statement()?;
                self.check_sub_statement(&stmt, true)?;
                Ok(Statement::Labeled { label: lbl, body: Box::new(stmt) })
            }
            _ => {
                let expr = self.parse_expression(0)?;
                self.consume_semicolon()?;
                Ok(Statement::Expression(expr))
            }
        }
    }

    fn parse_with_statement(&mut self) -> Result<Statement, ParseError> {
        let tok = self.advance()?; // with
        if self.is_strict {
            return Err(ParseError::UnexpectedToken(TokenKind::With, tok.line, tok.col));
        }
        self.expect(TokenKind::LParen)?;
        let object = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;
        let body = Box::new(self.parse_statement()?);
        self.check_sub_statement(&body, false)?;
        Ok(Statement::With { object, body })
    }

    fn parse_do_while_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // do
        let body = Box::new(self.parse_statement()?);
        self.check_sub_statement(&body, false)?;
        self.expect(TokenKind::While)?;
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;
        let _ = self.match_token(&TokenKind::Semicolon)?;
        Ok(Statement::DoWhile { body, condition })
    }

    fn parse_switch_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // switch
        self.expect(TokenKind::LParen)?;
        let discriminant = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::LBrace)?;

        let mut cases = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            if self.match_token(&TokenKind::Case)? {
                let test = self.parse_expression(0)?;
                self.expect(TokenKind::Colon)?;
                let mut consequent = Vec::new();
                while !self.check(&TokenKind::Case)
                    && !self.check(&TokenKind::Default)
                    && !self.check(&TokenKind::RBrace)
                    && self.current.kind != TokenKind::Eof
                {
                    consequent.push(self.parse_statement()?);
                }
                cases.push(crate::ast::SwitchCase {
                    test: Some(test),
                    consequent,
                });
            } else if self.match_token(&TokenKind::Default)? {
                self.expect(TokenKind::Colon)?;
                let mut consequent = Vec::new();
                while !self.check(&TokenKind::Case)
                    && !self.check(&TokenKind::Default)
                    && !self.check(&TokenKind::RBrace)
                    && self.current.kind != TokenKind::Eof
                {
                    consequent.push(self.parse_statement()?);
                }
                cases.push(crate::ast::SwitchCase {
                    test: None,
                    consequent,
                });
            } else {
                let tok = self.advance()?;
                return Err(ParseError::UnexpectedToken(tok.kind, tok.line, tok.col));
            }
        }

        self.expect(TokenKind::RBrace)?;
        Ok(Statement::Switch { discriminant, cases })
    }

    fn parse_array_destructuring_into(
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

    fn parse_object_destructuring_into(
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

    fn parse_var_decl(&mut self, kind: VarKind) -> Result<Statement, ParseError> {
        self.advance()?; // let / const / var
        let mut declarations = Vec::new();

        loop {
            if self.match_token(&TokenKind::LBracket)? {
                let tmp_name = format!("@destruct_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let mut destruct_decls = Vec::new();
                self.parse_array_destructuring_into(&tmp_name, &mut destruct_decls)?;
                let init = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                declarations.push(VarDeclarator { name: tmp_name, init });
                declarations.extend(destruct_decls);
            } else if self.match_token(&TokenKind::LBrace)? {
                let tmp_name = format!("@destruct_{}", self.destruct_counter);
                self.destruct_counter += 1;
                let mut destruct_decls = Vec::new();
                self.parse_object_destructuring_into(&tmp_name, &mut destruct_decls)?;
                let init = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                declarations.push(VarDeclarator { name: tmp_name, init });
                declarations.extend(destruct_decls);
            } else {
                let name = self.expect_identifier()?;
                let init = if self.match_token(&TokenKind::Eq)? {
                    Some(self.parse_expression(1)?)
                } else {
                    None
                };
                declarations.push(VarDeclarator { name, init });
            }

            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }

        self.consume_semicolon()?;
        Ok(Statement::VarDecl { kind, declarations })
    }

    fn skip_balanced(&mut self, open: TokenKind, close: TokenKind) -> Result<(), ParseError> {
        self.expect(open.clone())?;
        let mut depth = 1;
        while depth > 0 && !self.check(&TokenKind::Eof) {
            if self.check(&open) {
                depth += 1;
            } else if self.check(&close) {
                depth -= 1;
                if depth == 0 {
                    self.advance()?;
                    break;
                }
            } else if self.check(&TokenKind::LBracket) && open != TokenKind::LBracket {
                self.skip_balanced(TokenKind::LBracket, TokenKind::RBracket)?;
                continue;
            } else if self.check(&TokenKind::LBrace) && open != TokenKind::LBrace {
                self.skip_balanced(TokenKind::LBrace, TokenKind::RBrace)?;
                continue;
            } else if self.check(&TokenKind::LParen) {
                self.skip_balanced(TokenKind::LParen, TokenKind::RParen)?;
                continue;
            }
            self.advance()?;
        }
        Ok(())
    }

    fn parse_formal_parameters(&mut self) -> Result<(Vec<String>, Vec<Statement>), ParseError> {
        let prev_in_param = self.in_param;
        self.in_param = true;
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        let mut defaults = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                if self.match_token(&TokenKind::DotDotDot)? {
                    if self.check(&TokenKind::LBracket) {
                        self.skip_balanced(TokenKind::LBracket, TokenKind::RBracket)?;
                        params.push(format!("...__rest_{}", self.destruct_counter));
                        self.destruct_counter += 1;
                    } else if self.check(&TokenKind::LBrace) {
                        self.skip_balanced(TokenKind::LBrace, TokenKind::RBrace)?;
                        params.push(format!("...__rest_{}", self.destruct_counter));
                        self.destruct_counter += 1;
                    } else {
                        let id = self.expect_identifier()?;
                        params.push(format!("...{}", id));
                    }
                    break;
                }
                let mut destruct_decls = Vec::new();
                let (id, is_destruct) = if self.match_token(&TokenKind::LBracket)? {
                    let name = format!("__param_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    self.parse_array_destructuring_into(&name, &mut destruct_decls)?;
                    (name, true)
                } else if self.match_token(&TokenKind::LBrace)? {
                    let name = format!("__param_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    self.parse_object_destructuring_into(&name, &mut destruct_decls)?;
                    (name, true)
                } else {
                    (self.expect_identifier()?, false)
                };
                if self.match_token(&TokenKind::Eq)? {
                    let def_expr = self.parse_expression(1)?;
                    defaults.push(Statement::If {
                        condition: Expression::Binary {
                            op: BinaryOp::StrictEq,
                            left: Box::new(Expression::Identifier(id.clone())),
                            right: Box::new(Expression::Literal(Literal::Undefined)),
                        },
                        then_branch: Box::new(Statement::Expression(Expression::Assignment {
                            target: id.clone(),
                            value: Box::new(def_expr),
                        })),
                        else_branch: None,
                    });
                }
                if is_destruct {
                    defaults.push(Statement::If {
                        condition: Expression::Binary {
                            op: BinaryOp::Eq,
                            left: Box::new(Expression::Identifier(id.clone())),
                            right: Box::new(Expression::Literal(Literal::Null)),
                        },
                        then_branch: Box::new(Statement::Throw(Expression::New {
                            callee: Box::new(Expression::Identifier("TypeError".to_string())),
                            args: vec![Expression::Literal(Literal::String {
                                value: "Cannot destructure 'undefined' or 'null'".to_string(),
                                has_escape: false,
                                has_legacy_escape: false,
                            })],
                        })),
                        else_branch: None,
                    });
                    if !destruct_decls.is_empty() {
                        defaults.push(Statement::VarDecl {
                            kind: VarKind::Var,
                            declarations: destruct_decls,
                        });
                    }
                }
                params.push(id);
                if !self.match_token(&TokenKind::Comma)? {
                    break;
                }
                if self.check(&TokenKind::RParen) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen)?;
        self.in_param = prev_in_param;
        Ok((params, defaults))
    }

    fn apply_parameter_defaults(body: &mut Vec<Statement>, defaults: Vec<Statement>) {
        if defaults.is_empty() {
            return;
        }
        let mut insert_idx = 0;
        for stmt in body.iter() {
            if let Statement::Expression(Expression::Literal(Literal::String { value, has_escape, .. })) = stmt {
                if value == "use strict" && !has_escape {
                    insert_idx += 1;
                    continue;
                }
            }
            break;
        }
        body.splice(insert_idx..insert_idx, defaults);
    }

    fn parse_function_signature_and_body_opt_name(
        &mut self,
        allow_anon: bool,
    ) -> Result<(String, Vec<String>, Vec<Statement>), ParseError> {
        let name = if allow_anon && !matches!(&self.current.kind, TokenKind::Ident(_)) {
            "".to_string()
        } else {
            self.expect_identifier()?
        };
        let (params, defaults) = self.parse_formal_parameters()?;
        let mut body = self.parse_function_body(&params, if name.is_empty() { None } else { Some(&name) })?;
        Self::apply_parameter_defaults(&mut body, defaults);
        Ok((name, params, body))
    }

    fn parse_function_signature_and_body(
        &mut self,
    ) -> Result<(String, Vec<String>, Vec<Statement>), ParseError> {
        self.parse_function_signature_and_body_opt_name(false)
    }

    fn validate_strict_params_and_name(
        params: &[String],
        name: Option<&str>,
        line: usize,
        col: usize,
    ) -> Result<(), ParseError> {
        if let Some(n) = name {
            if is_strict_reserved_word(n) || n == "eval" || n == "arguments" {
                return Err(ParseError::UnexpectedToken(TokenKind::Ident(n.to_string()), line, col));
            }
        }
        let mut seen = std::collections::HashSet::new();
        for p in params {
            let clean = p.trim_start_matches("...");
            if is_strict_reserved_word(clean) || clean == "eval" || clean == "arguments" {
                return Err(ParseError::UnexpectedToken(TokenKind::Ident(clean.to_string()), line, col));
            }
            if !seen.insert(clean) {
                return Err(ParseError::UnexpectedToken(TokenKind::Ident(clean.to_string()), line, col));
            }
        }
        Ok(())
    }

    fn parse_function_body(
        &mut self,
        params: &[String],
        name: Option<&str>,
    ) -> Result<Vec<Statement>, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let prev_strict = self.is_strict;
        let prev_in_func = self.in_function;
        let prev_in_non_arrow = self.in_non_arrow_function;
        self.in_function = true;
        self.in_non_arrow_function = true;
        if prev_strict {
            Self::validate_strict_params_and_name(params, name, self.current.line, self.current.col)?;
        }
        let mut body = Vec::new();
        let mut in_directive_prologue = true;
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            let stmt = self.parse_statement()?;
            if in_directive_prologue {
                if let Statement::Expression(Expression::Literal(Literal::String { ref value, has_escape, has_legacy_escape })) = stmt {
                    if has_legacy_escape && self.is_strict {
                        return Err(ParseError::UnexpectedToken(TokenKind::StringLit { value: value.clone(), has_escape, has_legacy_escape, has_unpaired_surrogate: false }, self.current.line, self.current.col));
                    }
                    if value == "use strict" && !has_escape {
                        if !self.is_strict {
                            self.is_strict = true;
                            self.lexer.is_strict = true;
                            if body.iter().any(|s| matches!(s, Statement::Expression(Expression::Literal(Literal::String { has_legacy_escape: true, .. })))) {
                                return Err(ParseError::UnexpectedToken(TokenKind::StringLit { value: value.clone(), has_escape, has_legacy_escape, has_unpaired_surrogate: false }, self.current.line, self.current.col));
                            }
                            Self::validate_strict_params_and_name(params, name, self.current.line, self.current.col)?;
                        }
                    }
                } else {
                    in_directive_prologue = false;
                }
            }
            body.push(stmt);
        }
        self.expect(TokenKind::RBrace)?;
        self.is_strict = prev_strict;
        self.lexer.is_strict = prev_strict;
        self.in_function = prev_in_func;
        self.in_non_arrow_function = prev_in_non_arrow;
        Ok(body)
    }

    fn parse_function_decl_body(&mut self) -> Result<Statement, ParseError> {
        let (name, params, body) = self.parse_function_signature_and_body()?;
        Ok(Statement::FunctionDecl { name, params, body })
    }

    fn parse_async_function_decl(&mut self) -> Result<Statement, ParseError> {
        self.expect(TokenKind::Function)?;
        let is_generator = self.match_token(&TokenKind::Star)?;
        let prev_gen = self.in_generator;
        if is_generator {
            self.in_generator = true;
        }
        let res = self.parse_function_signature_and_body();
        self.in_generator = prev_gen;
        let (name, params, body) = res?;
        if is_generator {
            Ok(Statement::AsyncGeneratorDecl { name, params, body })
        } else {
            Ok(Statement::AsyncFunctionDecl { name, params, body })
        }
    }

    fn parse_generator_decl(&mut self) -> Result<Statement, ParseError> {
        let prev_gen = self.in_generator;
        self.in_generator = true;
        let res = self.parse_function_signature_and_body();
        self.in_generator = prev_gen;
        let (name, params, body) = res?;
        Ok(Statement::GeneratorDecl { name, params, body })
    }

    fn parse_property_name_meta(&mut self) -> Result<(PropertyKey, bool), ParseError> {
        if self.match_token(&TokenKind::LBracket)? {
            let expr = self.parse_expression(0)?;
            self.expect(TokenKind::RBracket)?;
            Ok((PropertyKey::Computed(expr), false))
        } else {
            let key_tok = self.advance()?;
            let is_ident = matches!(&key_tok.kind, TokenKind::Ident(_));
            let key = match &key_tok.kind {
                TokenKind::StringLit { value, .. } => value.clone(),
                TokenKind::Number(n) => n.to_string(),
                TokenKind::BigInt(b) => b.to_string(),
                TokenKind::PrivateIdent(id) => format!("#{}", id),
                other => match other.as_prop_name() {
                    Some(id) => id,
                    None => return Err(ParseError::UnexpectedToken(other.clone(), key_tok.line, key_tok.col)),
                },
            };
            Ok((PropertyKey::Literal(key), is_ident))
        }
    }

    fn parse_property_name(&mut self) -> Result<PropertyKey, ParseError> {
        self.parse_property_name_meta().map(|(k, _)| k)
    }

    fn scan_declared_private_names(&self) -> HashSet<String> {
        let mut names = HashSet::new();
        let mut lex = self.lexer.clone();
        let mut prev_kind = Some(self.current.kind.clone());
        let mut brace_depth = 1usize;

        match &self.current.kind {
            TokenKind::LBrace => brace_depth += 1,
            TokenKind::RBrace => {
                brace_depth = brace_depth.saturating_sub(1);
            }
            TokenKind::PrivateIdent(s) => {
                if brace_depth == 1 {
                    names.insert(format!("#{}", s));
                }
            }
            _ => {}
        }
        if brace_depth == 0 {
            return names;
        }

        let mut cur_tok = self.peeked.clone();
        loop {
            match &cur_tok.kind {
                TokenKind::LBrace => {
                    brace_depth += 1;
                }
                TokenKind::RBrace => {
                    brace_depth = brace_depth.saturating_sub(1);
                    if brace_depth == 0 {
                        break;
                    }
                }
                TokenKind::Eof => {
                    break;
                }
                TokenKind::PrivateIdent(s) => {
                    if brace_depth == 1 && prev_kind != Some(TokenKind::Dot) {
                        names.insert(format!("#{}", s));
                    }
                }
                _ => {}
            }
            prev_kind = Some(cur_tok.kind);
            match lex.next_token() {
                Ok(t) => cur_tok = t,
                Err(_) => break,
            }
        }
        names
    }

    fn parse_class_elements(&mut self, is_derived: bool) -> Result<(Vec<String>, Vec<Statement>, bool, Vec<ClassElement>), ParseError> {
        let mut ctor_params = Vec::new();
        let mut ctor_body = Vec::new();
        let mut has_ctor = false;
        let mut elements = Vec::new();

        self.expect(TokenKind::LBrace)?;
        let private_names = self.scan_declared_private_names();
        self.private_names_stack.push(private_names);
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            if self.match_token(&TokenKind::Semicolon)? {
                continue;
            }
            let is_static = if self.check(&TokenKind::Static) || self.check_ident("static") {
                if self.peeked.kind == TokenKind::LParen || self.peeked.kind == TokenKind::Colon || self.peeked.kind == TokenKind::Eq {
                    false
                } else {
                    self.advance()?;
                    true
                }
            } else {
                false
            };

            let is_async = if self.check(&TokenKind::Async) || self.check_ident("async") {
                if self.peeked.kind == TokenKind::LParen || self.peeked.kind == TokenKind::Colon || self.peeked.kind == TokenKind::Eq || self.peeked.kind == TokenKind::Semicolon {
                    false
                } else {
                    self.advance()?;
                    true
                }
            } else {
                false
            };

            let is_generator = self.match_token(&TokenKind::Star)?;

            if !is_generator && (self.check_ident("get") || self.check_ident("set"))
                && self.peeked.kind != TokenKind::LParen
                && self.peeked.kind != TokenKind::Semicolon
                && self.peeked.kind != TokenKind::RBrace
                && self.peeked.kind != TokenKind::Colon
                && self.peeked.kind != TokenKind::Eq
            {
                let is_getter = self.check_ident("get");
                self.advance()?; // 'get' or 'set'
                let key = self.parse_property_name()?;
                if is_getter {
                    self.expect(TokenKind::LParen)?;
                    self.expect(TokenKind::RParen)?;
                    let prev_ctor = self.in_derived_constructor;
                    let prev_method = self.in_method;
                    self.in_method = true;
                    self.in_derived_constructor = false;
                    let body = self.parse_function_body(&[], None)?;
                    self.in_method = prev_method;
                    self.in_derived_constructor = prev_ctor;
                    elements.push(ClassElement {
                        is_static,
                        key,
                        kind: ClassElementKind::Getter(Expression::FunctionExpr {
                            name: None,
                            params: vec![],
                            body,
                        }),
                    });
                } else {
                    self.expect(TokenKind::LParen)?;
                    let param = self.expect_identifier()?;
                    self.expect(TokenKind::RParen)?;
                    let prev_ctor = self.in_derived_constructor;
                    let prev_method = self.in_method;
                    self.in_method = true;
                    self.in_derived_constructor = false;
                    let body = self.parse_function_body(&[param.clone()], None)?;
                    self.in_method = prev_method;
                    self.in_derived_constructor = prev_ctor;
                    elements.push(ClassElement {
                        is_static,
                        key,
                        kind: ClassElementKind::Setter(Expression::FunctionExpr {
                            name: None,
                            params: vec![param],
                            body,
                        }),
                    });
                }
                continue;
            }

            let key = self.parse_property_name()?;
            if !is_static && !is_generator && matches!(&key, PropertyKey::Literal(k) if k == "constructor") {
                if self.check(&TokenKind::LParen) {
                    let (params, defaults) = self.parse_formal_parameters()?;
                    let prev_ctor = self.in_derived_constructor;
                    let prev_method = self.in_method;
                    self.in_derived_constructor = is_derived;
                    self.in_method = false;
                    let mut body = self.parse_function_body(&params, None)?;
                    self.in_derived_constructor = prev_ctor;
                    self.in_method = prev_method;
                    Self::apply_parameter_defaults(&mut body, defaults);
                    ctor_params = params;
                    ctor_body = body;
                    has_ctor = true;
                    continue;
                }
            }

            if self.check(&TokenKind::LParen) {
                let (params, defaults) = self.parse_formal_parameters()?;
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                self.in_method = true;
                self.in_derived_constructor = false;
                let mut body = self.parse_function_body(&params, None)?;
                self.in_derived_constructor = prev_ctor;
                self.in_method = prev_method;
                Self::apply_parameter_defaults(&mut body, defaults);
                let method_expr = if is_async && is_generator {
                    Expression::AsyncGeneratorExpr {
                        name: None,
                        params,
                        body,
                    }
                } else if is_async {
                    Expression::AsyncFunctionExpr {
                        name: None,
                        params,
                        body,
                    }
                } else if is_generator {
                    Expression::GeneratorExpr {
                        name: None,
                        params,
                        body,
                    }
                } else {
                    Expression::MethodExpr {
                        name: None,
                        params,
                        body,
                    }
                };
                elements.push(ClassElement {
                    is_static,
                    key,
                    kind: ClassElementKind::Method(method_expr),
                });
            } else if self.match_token(&TokenKind::Eq)? {
                let init_expr = self.parse_expression(0)?;
                let _ = self.match_token(&TokenKind::Semicolon)?;
                if !is_static {
                    let init_stmt = match key {
                        PropertyKey::Literal(k) => Statement::Expression(Expression::MemberAssignment {
                            object: Box::new(Expression::This),
                            property: k,
                            value: Box::new(init_expr),
                        }),
                        PropertyKey::Computed(expr) => Statement::Expression(Expression::IndexAssignment {
                            object: Box::new(Expression::This),
                            index: Box::new(expr),
                            value: Box::new(init_expr),
                        }),
                    };
                    ctor_body.push(init_stmt);
                }
            } else {
                let _ = self.match_token(&TokenKind::Semicolon)?;
            }
        }
        self.expect(TokenKind::RBrace)?;
        self.private_names_stack.pop();
        let _ = self.match_token(&TokenKind::Semicolon)?;

        Ok((ctor_params, ctor_body, has_ctor, elements))
    }

    fn parse_class_decl(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // 'class'
        if self.is_module && self.check(&TokenKind::Await) {
            return Err(ParseError::UnexpectedToken(TokenKind::Await, self.current.line, self.current.col));
        }
        let name = if let TokenKind::Ident(id) = &self.current.kind {
            let n = id.clone();
            self.advance()?;
            Some(n)
        } else if !self.is_module && !self.is_strict && self.check(&TokenKind::Await) {
            self.advance()?;
            Some("await".to_string())
        } else {
            None
        };
        let super_class = if self.match_token(&TokenKind::Extends)? {
            Some(Box::new(self.parse_expression(0)?))
        } else {
            None
        };
        let (mut ctor_params, mut ctor_body, has_ctor, elements) = self.parse_class_elements(super_class.is_some())?;
        if !has_ctor {
            if let Some(ref sup) = super_class {
                ctor_params = vec!["...__args".to_string()];
                ctor_body = vec![
                    Statement::Expression(Expression::Call {
                        callee: Box::new(Expression::Member {
                            object: sup.clone(),
                            property: "apply".to_string(),
                        }),
                        args: vec![Expression::This, Expression::Identifier("__args".to_string())],
                    }),
                ];
            }
        }
        if let Some(ref sup) = super_class {
            let this_var = format!("__this_{}", self.destruct_counter);
            self.destruct_counter += 1;
            ctor_body.insert(0, Statement::VarDecl {
                kind: VarKind::Var,
                declarations: vec![
                    VarDeclarator {
                        name: this_var.clone(),
                        init: Some(Expression::This),
                    },
                    VarDeclarator {
                        name: "super".to_string(),
                        init: Some(Expression::FunctionExpr {
                            name: None,
                            params: vec!["...__sargs".to_string()],
                            body: vec![Statement::Return(Some(Expression::Call {
                                callee: Box::new(Expression::Member {
                                    object: sup.clone(),
                                    property: "apply".to_string(),
                                }),
                                args: vec![Expression::Identifier(this_var), Expression::Identifier("__sargs".to_string())],
                            }))],
                        }),
                    },
                ],
            });
        }
        Ok(Statement::ClassDecl(ClassDefinition {
            name,
            super_class,
            ctor_params,
            ctor_body,
            elements,
        }))
    }

    fn parse_class_expr(&mut self) -> Result<Expression, ParseError> {
        if self.is_module && self.check(&TokenKind::Await) {
            return Err(ParseError::UnexpectedToken(TokenKind::Await, self.current.line, self.current.col));
        }
        let name = if let TokenKind::Ident(id) = &self.current.kind {
            let n = id.clone();
            self.advance()?;
            Some(n)
        } else {
            None
        };
        let super_class = if self.match_token(&TokenKind::Extends)? {
            Some(Box::new(self.parse_expression(0)?))
        } else {
            None
        };
        let (mut ctor_params, mut ctor_body, has_ctor, elements) = self.parse_class_elements(super_class.is_some())?;
        if !has_ctor {
            if let Some(ref sup) = super_class {
                ctor_params = vec!["...__args".to_string()];
                ctor_body = vec![
                    Statement::Expression(Expression::Call {
                        callee: Box::new(Expression::Member {
                            object: sup.clone(),
                            property: "apply".to_string(),
                        }),
                        args: vec![Expression::This, Expression::Identifier("__args".to_string())],
                    }),
                ];
            }
        }
        if let Some(ref sup) = super_class {
            let this_var = format!("__this_{}", self.destruct_counter);
            self.destruct_counter += 1;
            ctor_body.insert(0, Statement::VarDecl {
                kind: VarKind::Var,
                declarations: vec![
                    VarDeclarator {
                        name: this_var.clone(),
                        init: Some(Expression::This),
                    },
                    VarDeclarator {
                        name: "super".to_string(),
                        init: Some(Expression::FunctionExpr {
                            name: None,
                            params: vec!["...__sargs".to_string()],
                            body: vec![Statement::Return(Some(Expression::Call {
                                callee: Box::new(Expression::Member {
                                    object: sup.clone(),
                                    property: "apply".to_string(),
                                }),
                                args: vec![Expression::Identifier(this_var), Expression::Identifier("__sargs".to_string())],
                            }))],
                        }),
                    },
                ],
            });
        }
        Ok(Expression::ClassExpr(Box::new(ClassDefinition {
            name,
            super_class,
            ctor_params,
            ctor_body,
            elements,
        })))
    }

    fn expect_export_name(&mut self) -> Result<String, ParseError> {
        let tok = self.advance()?;
        match tok.kind {
            TokenKind::Ident(s) => Ok(s),
            TokenKind::StringLit { value, has_unpaired_surrogate, .. } => {
                if has_unpaired_surrogate {
                    return Err(ParseError::General("Export name contains unpaired surrogate".to_string()));
                }
                Ok(value)
            }
            TokenKind::Default => Ok("default".to_string()),
            TokenKind::As => Ok("as".to_string()),
            TokenKind::From => Ok("from".to_string()),
            TokenKind::Let => Ok("let".to_string()),
            TokenKind::Const => Ok("const".to_string()),
            TokenKind::Var => Ok("var".to_string()),
            TokenKind::Function => Ok("function".to_string()),
            TokenKind::Class => Ok("class".to_string()),
            TokenKind::If => Ok("if".to_string()),
            TokenKind::Else => Ok("else".to_string()),
            TokenKind::While => Ok("while".to_string()),
            TokenKind::For => Ok("for".to_string()),
            TokenKind::Do => Ok("do".to_string()),
            TokenKind::Break => Ok("break".to_string()),
            TokenKind::Continue => Ok("continue".to_string()),
            TokenKind::Return => Ok("return".to_string()),
            TokenKind::Switch => Ok("switch".to_string()),
            TokenKind::Case => Ok("case".to_string()),
            TokenKind::Throw => Ok("throw".to_string()),
            TokenKind::Try => Ok("try".to_string()),
            TokenKind::Catch => Ok("catch".to_string()),
            TokenKind::Finally => Ok("finally".to_string()),
            TokenKind::New => Ok("new".to_string()),
            TokenKind::This => Ok("this".to_string()),
            TokenKind::Super => Ok("super".to_string()),
            TokenKind::Null => Ok("null".to_string()),
            TokenKind::True => Ok("true".to_string()),
            TokenKind::False => Ok("false".to_string()),
            TokenKind::In => Ok("in".to_string()),
            TokenKind::Instanceof => Ok("instanceof".to_string()),
            TokenKind::Typeof => Ok("typeof".to_string()),
            TokenKind::Void => Ok("void".to_string()),
            TokenKind::Delete => Ok("delete".to_string()),
            TokenKind::With => Ok("with".to_string()),
            TokenKind::Yield => Ok("yield".to_string()),
            TokenKind::Await => Ok("await".to_string()),
            TokenKind::Import => Ok("import".to_string()),
            TokenKind::Export => Ok("export".to_string()),
            TokenKind::Debugger => Ok("debugger".to_string()),
            other => Err(ParseError::UnexpectedToken(other, tok.line, tok.col)),
        }
    }

    fn parse_import_statement(&mut self, is_top: bool) -> Result<Statement, ParseError> {
        let import_tok = self.advance()?; // import
        if !self.is_module || !is_top {
            return Err(ParseError::UnexpectedToken(TokenKind::Import, import_tok.line, import_tok.col));
        }

        // Bare import: import "module-name";
        if let TokenKind::StringLit { value, has_unpaired_surrogate, .. } = &self.current.kind {
            if *has_unpaired_surrogate {
                return Err(ParseError::General("Import name contains unpaired surrogate".to_string()));
            }
            let source = value.clone();
            self.advance()?;
            self.consume_semicolon()?;
            return Ok(Statement::Import { specifiers: Vec::new(), source, is_default: false, is_all: false });
        }

        let mut specifiers = Vec::new();
        let mut is_default = false;
        let mut is_all = false;

        if let TokenKind::Ident(default_name) = &self.current.kind {
            let name = default_name.clone();
            self.advance()?;
            specifiers.push(("default".to_string(), Some(name)));
            is_default = true;
            if self.match_token(&TokenKind::Comma)? {
                // optional comma after default
            }
        }

        if self.match_token(&TokenKind::Star)? {
            self.expect(TokenKind::As)?;
            let ns = self.expect_identifier()?;
            specifiers.push(("*".to_string(), Some(ns)));
            is_all = true;
        } else if self.match_token(&TokenKind::LBrace)? {
            while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
                let is_string_name = matches!(&self.current.kind, TokenKind::StringLit { .. });
                let name = self.expect_export_name()?;
                let alias = if self.match_token(&TokenKind::As)? {
                    Some(self.expect_identifier()?)
                } else {
                    if is_string_name {
                        return Err(ParseError::General("String literal import must have an 'as' clause".to_string()));
                    }
                    if is_strict_reserved_word(&name) {
                        return Err(ParseError::UnexpectedToken(TokenKind::Ident(name), self.current.line, self.current.col));
                    }
                    None
                };
                specifiers.push((name, alias));
                if !self.match_token(&TokenKind::Comma)? {
                    break;
                }
            }
            self.expect(TokenKind::RBrace)?;
        }

        self.expect(TokenKind::From)?;
        let src_tok = self.advance()?;
        let source = match src_tok.kind {
            TokenKind::StringLit { value, .. } => value,
            other => return Err(ParseError::UnexpectedToken(other, src_tok.line, src_tok.col)),
        };
        self.consume_semicolon()?;

        let mut bound_names = std::collections::HashSet::new();
        for (name, alias) in &specifiers {
            let bound = alias.as_ref().unwrap_or(name);
            if bound == "eval" || bound == "arguments" {
                return Err(ParseError::UnexpectedToken(TokenKind::Ident(bound.clone()), self.current.line, self.current.col));
            }
            if !bound_names.insert(bound.clone()) {
                return Err(ParseError::General(format!("Duplicate import binding '{}'", bound)));
            }
        }

        Ok(Statement::Import { specifiers, source, is_default, is_all })
    }

    fn parse_export_statement(&mut self, is_top: bool) -> Result<Statement, ParseError> {
        let export_tok = self.advance()?; // export
        if !self.is_module || !is_top {
            return Err(ParseError::UnexpectedToken(TokenKind::Export, export_tok.line, export_tok.col));
        }

        if self.match_token(&TokenKind::Star)? {
            let all_as = if self.match_token(&TokenKind::As)? {
                Some(self.expect_export_name()?)
            } else {
                None
            };
            self.expect(TokenKind::From)?;
            let src_tok = self.advance()?;
            let source = match src_tok.kind {
                TokenKind::StringLit { value, .. } => value,
                other => return Err(ParseError::UnexpectedToken(other, src_tok.line, src_tok.col)),
            };
            self.consume_semicolon()?;
            return Ok(Statement::Export {
                decl: None,
                is_default: false,
                specifiers: Vec::new(),
                source: Some(source),
                is_all: true,
                all_as,
            });
        }

        if self.match_token(&TokenKind::LBrace)? {
            let mut specifiers = Vec::new();
            let mut has_string_local = false;
            while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
                if matches!(&self.current.kind, TokenKind::StringLit { .. }) {
                    has_string_local = true;
                }
                let name = self.expect_export_name()?;
                let alias = if self.match_token(&TokenKind::As)? {
                    Some(self.expect_export_name()?)
                } else {
                    None
                };
                specifiers.push((name, alias));
                if !self.match_token(&TokenKind::Comma)? {
                    break;
                }
            }
            self.expect(TokenKind::RBrace)?;
            let source = if self.match_token(&TokenKind::From)? {
                let src_tok = self.advance()?;
                match src_tok.kind {
                    TokenKind::StringLit { value, .. } => Some(value),
                    other => return Err(ParseError::UnexpectedToken(other, src_tok.line, src_tok.col)),
                }
            } else {
                None
            };
            if source.is_none() && has_string_local {
                return Err(ParseError::General("Export specifier cannot be a string literal without a from clause".to_string()));
            }
            self.consume_semicolon()?;
            return Ok(Statement::Export {
                decl: None,
                is_default: false,
                specifiers,
                source,
                is_all: false,
                all_as: None,
            });
        }

        if self.match_token(&TokenKind::Default)? {
            if self.check(&TokenKind::Class) {
                let decl = self.parse_class_decl()?;
                return Ok(Statement::Export {
                    decl: Some(Box::new(decl)),
                    is_default: true,
                    specifiers: Vec::new(),
                    source: None,
                    is_all: false,
                    all_as: None,
                });
            } else if self.check(&TokenKind::Function) {
                self.advance()?; // function
                let is_generator = self.match_token(&TokenKind::Star)?;
                let prev_gen = self.in_generator;
                if is_generator {
                    self.in_generator = true;
                }
                let (name, params, body) = self.parse_function_signature_and_body_opt_name(true)?;
                self.in_generator = prev_gen;
                let decl = if is_generator {
                    Statement::GeneratorDecl { name, params, body }
                } else {
                    Statement::FunctionDecl { name, params, body }
                };
                return Ok(Statement::Export {
                    decl: Some(Box::new(decl)),
                    is_default: true,
                    specifiers: Vec::new(),
                    source: None,
                    is_all: false,
                    all_as: None,
                });
            } else if (self.check(&TokenKind::Async) || self.check_ident("async"))
                && !self.peeked.has_preceding_line_terminator
                && self.peeked.kind == TokenKind::Function
            {
                self.advance()?; // async
                self.expect(TokenKind::Function)?;
                let is_generator = self.match_token(&TokenKind::Star)?;
                let prev_gen = self.in_generator;
                let prev_async = self.in_async;
                self.in_async = true;
                if is_generator {
                    self.in_generator = true;
                }
                let (name, params, body) = self.parse_function_signature_and_body_opt_name(true)?;
                self.in_generator = prev_gen;
                self.in_async = prev_async;
                let decl = if is_generator {
                    Statement::AsyncGeneratorDecl { name, params, body }
                } else {
                    Statement::AsyncFunctionDecl { name, params, body }
                };
                return Ok(Statement::Export {
                    decl: Some(Box::new(decl)),
                    is_default: true,
                    specifiers: Vec::new(),
                    source: None,
                    is_all: false,
                    all_as: None,
                });
            } else {
                let expr = self.parse_expression(1)?;
                self.consume_semicolon()?;
                return Ok(Statement::Export {
                    decl: Some(Box::new(Statement::Expression(expr))),
                    is_default: true,
                    specifiers: Vec::new(),
                    source: None,
                    is_all: false,
                    all_as: None,
                });
            }
        }

        let stmt = self.parse_statement()?;
        Ok(Statement::Export {
            decl: Some(Box::new(stmt)),
            is_default: false,
            specifiers: Vec::new(),
            source: None,
            is_all: false,
            all_as: None,
        })
    }

    fn parse_if_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // if
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;

        let then_branch = Box::new(self.parse_statement()?);
        self.check_sub_statement(&then_branch, true)?;
        let else_branch = if self.match_token(&TokenKind::Else)? {
            let eb = Box::new(self.parse_statement()?);
            self.check_sub_statement(&eb, true)?;
            Some(eb)
        } else {
            None
        };

        Ok(Statement::If { condition, then_branch, else_branch })
    }

    fn parse_while_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // while
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;

        let body = Box::new(self.parse_statement()?);
        self.check_sub_statement(&body, false)?;
        Ok(Statement::While { condition, body })
    }

    fn parse_for_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // for
        let is_await = if self.match_token(&TokenKind::Await)? {
            if !self.in_async && !self.is_module {
                return Err(ParseError::UnexpectedToken(TokenKind::Await, self.current.line, self.current.col));
            }
            true
        } else {
            false
        };
        self.expect(TokenKind::LParen)?;

        if self.check(&TokenKind::Let) || self.check(&TokenKind::Const) || self.check(&TokenKind::Var) {
            let kind_tok = self.advance()?; // let/const/var
            let var_name = self.expect_identifier()?;
            let var_kind = match kind_tok.kind {
                TokenKind::Let => Some(VarKind::Let),
                TokenKind::Const => Some(VarKind::Const),
                _ => Some(VarKind::Var),
            };
            if self.match_token(&TokenKind::In)? {
                if is_await {
                    return Err(ParseError::UnexpectedToken(TokenKind::In, self.current.line, self.current.col));
                }
                let right = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;
                return Ok(Statement::ForIn { var_kind, left: var_name, right, body });
            }
            if self.check_ident("of") {
                self.advance()?; // of
                let right = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;
                return Ok(Statement::ForOf { var_kind, left: var_name, right, body, is_await });
            }
            if is_await {
                return Err(ParseError::UnexpectedToken(self.current.kind.clone(), self.current.line, self.current.col));
            }
            let kind = match kind_tok.kind {
                TokenKind::Let => VarKind::Let,
                TokenKind::Const => VarKind::Const,
                _ => VarKind::Var,
            };
            let init_val = if self.match_token(&TokenKind::Eq)? {
                let old_in = self.allow_in;
                self.allow_in = false;
                let val = self.parse_expression(1);
                self.allow_in = old_in;
                Some(val?)
            } else {
                None
            };
            let mut declarations = vec![VarDeclarator { name: var_name, init: init_val }];
            while self.match_token(&TokenKind::Comma)? {
                let n = self.expect_identifier()?;
                let iv = if self.match_token(&TokenKind::Eq)? {
                    let old_in = self.allow_in;
                    self.allow_in = false;
                    let val = self.parse_expression(1);
                    self.allow_in = old_in;
                    Some(val?)
                } else {
                    None
                };
                declarations.push(VarDeclarator { name: n, init: iv });
            }
            self.expect(TokenKind::Semicolon)?;
            let init = Some(Box::new(Statement::VarDecl { kind, declarations }));
            let condition = if self.check(&TokenKind::Semicolon) { None } else { Some(self.parse_expression(0)?) };
            self.expect(TokenKind::Semicolon)?;
            let update = if self.check(&TokenKind::RParen) { None } else { Some(self.parse_expression(0)?) };
            self.expect(TokenKind::RParen)?;
            let body = Box::new(self.parse_statement()?);
            self.check_sub_statement(&body, false)?;
            return Ok(Statement::For { init, condition, update, body });
        }

        let init = if self.match_token(&TokenKind::Semicolon)? {
            if is_await {
                return Err(ParseError::UnexpectedToken(TokenKind::Semicolon, self.current.line, self.current.col));
            }
            None
        } else {
            let old_in = self.allow_in;
            self.allow_in = false;
            let expr_res = self.parse_expression(0);
            self.allow_in = old_in;
            let expr = expr_res?;
            if self.match_token(&TokenKind::In)? {
                if is_await {
                    return Err(ParseError::UnexpectedToken(TokenKind::In, self.current.line, self.current.col));
                }
                let right = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;
                let left = match expr {
                    Expression::Identifier(n) => n,
                    _ => "__lhs".to_string(),
                };
                return Ok(Statement::ForIn { var_kind: None, left, right, body });
            }
            if self.check_ident("of") {
                self.advance()?; // of
                let right = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                let body = Box::new(self.parse_statement()?);
                self.check_sub_statement(&body, false)?;
                let left = match expr {
                    Expression::Identifier(n) => n,
                    _ => "__lhs".to_string(),
                };
                return Ok(Statement::ForOf { var_kind: None, left, right, body, is_await });
            }
            if is_await {
                return Err(ParseError::UnexpectedToken(self.current.kind.clone(), self.current.line, self.current.col));
            }
            self.expect(TokenKind::Semicolon)?;
            Some(Box::new(Statement::Expression(expr)))
        };

        let condition = if self.check(&TokenKind::Semicolon) { None } else { Some(self.parse_expression(0)?) };
        self.expect(TokenKind::Semicolon)?;
        let update = if self.check(&TokenKind::RParen) { None } else { Some(self.parse_expression(0)?) };
        self.expect(TokenKind::RParen)?;
        let body = Box::new(self.parse_statement()?);
        self.check_sub_statement(&body, false)?;
        Ok(Statement::For { init, condition, update, body })
    }

    fn parse_try_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // try
        let try_block = Box::new(self.parse_block_statement()?);
        let mut catch_param = None;
        let mut catch_block = None;
        if self.match_token(&TokenKind::Catch)? {
            let mut pattern_decls = Vec::new();
            if self.match_token(&TokenKind::LParen)? {
                if self.check(&TokenKind::LBrace) {
                    let tmp = format!("__catch_err_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    catch_param = Some(tmp.clone());
                    self.parse_object_destructuring_into(&tmp, &mut pattern_decls)?;
                } else if self.check(&TokenKind::LBracket) {
                    let tmp = format!("__catch_err_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    catch_param = Some(tmp.clone());
                    self.parse_array_destructuring_into(&tmp, &mut pattern_decls)?;
                } else {
                    let p_tok = self.advance()?;
                    match p_tok.kind {
                        TokenKind::Ident(id) => catch_param = Some(id),
                        other => return Err(ParseError::UnexpectedToken(other, p_tok.line, p_tok.col)),
                    }
                }
                self.expect(TokenKind::RParen)?;
            }
            let mut block = self.parse_block_statement()?;
            if !pattern_decls.is_empty() {
                if let Statement::Block(ref mut stmts) = block {
                    stmts.insert(0, Statement::VarDecl { kind: VarKind::Let, declarations: pattern_decls });
                }
            }
            catch_block = Some(Box::new(block));
        }
        let mut finally_block = None;
        if self.match_token(&TokenKind::Finally)? {
            finally_block = Some(Box::new(self.parse_block_statement()?));
        }
        Ok(Statement::TryCatch { try_block, catch_param, catch_block, finally_block })
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
        let ret_tok = self.advance()?; // return
        if !self.in_function {
            return Err(ParseError::UnexpectedToken(TokenKind::Return, ret_tok.line, ret_tok.col));
        }
        if self.check(&TokenKind::Semicolon)
            || self.check(&TokenKind::RBrace)
            || self.check(&TokenKind::Eof)
            || self.current.has_preceding_line_terminator
        {
            self.match_token(&TokenKind::Semicolon)?;
            Ok(Statement::Return(None))
        } else {
            let expr = self.parse_expression(0)?;
            self.consume_semicolon()?;
            Ok(Statement::Return(Some(expr)))
        }
    }

    fn check_sub_statement(&self, stmt: &Statement, allow_sloppy_func: bool) -> Result<(), ParseError> {
        match stmt {
            Statement::VarDecl { kind: VarKind::Let | VarKind::Const, .. }
            | Statement::ClassDecl(_)
            | Statement::AsyncFunctionDecl { .. }
            | Statement::GeneratorDecl { .. }
            | Statement::AsyncGeneratorDecl { .. } => {
                Err(ParseError::General("Declaration cannot appear in a single-statement context".into()))
            }
            Statement::FunctionDecl { .. } => {
                if allow_sloppy_func && !self.is_strict {
                    Ok(())
                } else {
                    Err(ParseError::General("Function declaration cannot appear in a single-statement context".into()))
                }
            }
            Statement::Labeled { body, .. } => {
                self.check_sub_statement(body, allow_sloppy_func)
            }
            _ => Ok(()),
        }
    }

    fn validate_block_declarations(&self, stmts: &[Statement]) -> Result<(), ParseError> {
        let mut lex_names: Vec<(String, bool)> = Vec::new();
        for stmt in stmts {
            match stmt {
                Statement::VarDecl { kind, declarations } if *kind != VarKind::Var => {
                    for decl in declarations {
                        if !decl.name.starts_with("__") {
                            Self::add_lexical_name(&mut lex_names, decl.name.clone(), false, self.is_strict)?;
                        }
                    }
                }
                Statement::ClassDecl(class_def) => {
                    if let Some(ref name) = class_def.name {
                        Self::add_lexical_name(&mut lex_names, name.clone(), false, self.is_strict)?;
                    }
                }
                Statement::FunctionDecl { name, .. } => {
                    Self::add_lexical_name(&mut lex_names, name.clone(), true, self.is_strict)?;
                }
                Statement::AsyncFunctionDecl { name, .. }
                | Statement::GeneratorDecl { name, .. }
                | Statement::AsyncGeneratorDecl { name, .. } => {
                    Self::add_lexical_name(&mut lex_names, name.clone(), false, self.is_strict)?;
                }
                _ => {}
            }
        }

        if !lex_names.is_empty() {
            let mut var_names = Vec::new();
            for stmt in stmts {
                Self::collect_var_declared_names(stmt, &mut var_names);
            }
            for (lex, _) in &lex_names {
                if var_names.contains(lex) {
                    return Err(ParseError::General(format!(
                        "Identifier '{}' has already been declared",
                        lex
                    )));
                }
            }
        }

        Ok(())
    }

    fn add_lexical_name(
        names: &mut Vec<(String, bool)>,
        name: String,
        is_plain_func: bool,
        is_strict: bool,
    ) -> Result<(), ParseError> {
        if let Some((_, prev_is_plain_func)) = names.iter().find(|(n, _)| *n == name) {
            if !is_strict && *prev_is_plain_func && is_plain_func {
                // Annex B allows duplicate ordinary FunctionDeclarations in sloppy mode
            } else {
                return Err(ParseError::General(format!(
                    "Identifier '{}' has already been declared",
                    name
                )));
            }
        }
        names.push((name, is_plain_func));
        Ok(())
    }

    fn collect_var_declared_names(stmt: &Statement, vars: &mut Vec<String>) {
        match stmt {
            Statement::VarDecl { kind: VarKind::Var, declarations } => {
                for decl in declarations {
                    if !vars.contains(&decl.name) {
                        vars.push(decl.name.clone());
                    }
                }
            }
            Statement::Block(inner) => {
                for s in inner {
                    Self::collect_var_declared_names(s, vars);
                }
            }
            Statement::If { then_branch, else_branch, .. } => {
                Self::collect_var_declared_names(then_branch, vars);
                if let Some(eb) = else_branch {
                    Self::collect_var_declared_names(eb, vars);
                }
            }
            Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
                Self::collect_var_declared_names(body, vars);
            }
            Statement::For { init, body, .. } => {
                if let Some(init_stmt) = init {
                    Self::collect_var_declared_names(init_stmt, vars);
                }
                Self::collect_var_declared_names(body, vars);
            }
            Statement::ForIn { var_kind, left, body, .. } | Statement::ForOf { var_kind, left, body, .. } => {
                if *var_kind == Some(VarKind::Var) && !vars.contains(left) {
                    vars.push(left.clone());
                }
                Self::collect_var_declared_names(body, vars);
            }
            Statement::With { body, .. } | Statement::Labeled { body, .. } => {
                Self::collect_var_declared_names(body, vars);
            }
            Statement::TryCatch { try_block, catch_block, finally_block, .. } => {
                Self::collect_var_declared_names(try_block, vars);
                if let Some(cb) = catch_block {
                    Self::collect_var_declared_names(cb, vars);
                }
                if let Some(fb) = finally_block {
                    Self::collect_var_declared_names(fb, vars);
                }
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    for s in &case.consequent {
                        Self::collect_var_declared_names(s, vars);
                    }
                }
            }
            _ => {}
        }
    }

    fn parse_block_statement(&mut self) -> Result<Statement, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            stmts.push(self.parse_statement()?);
        }
        self.expect(TokenKind::RBrace)?;
        self.validate_block_declarations(&stmts)?;
        Ok(Statement::Block(stmts))
    }

    fn validate_module_early_errors(&self, body: &[Statement]) -> Result<(), ParseError> {
        let mut lex_names: Vec<String> = Vec::new();
        let mut var_names: Vec<String> = Vec::new();
        let mut exported_names: Vec<String> = Vec::new();
        let mut unresolvable_exports: Vec<String> = Vec::new();

        let add_lex = |names: &mut Vec<String>, name: String| -> Result<(), ParseError> {
            if names.contains(&name) {
                return Err(ParseError::General(format!("Identifier '{}' has already been declared", name)));
            }
            names.push(name);
            Ok(())
        };

        let add_export = |names: &mut Vec<String>, name: String| -> Result<(), ParseError> {
            if names.contains(&name) {
                return Err(ParseError::General(format!("Duplicate exported name '{}'", name)));
            }
            names.push(name);
            Ok(())
        };

        for stmt in body {
            match stmt {
                Statement::VarDecl { kind, declarations } => {
                    if *kind == VarKind::Var {
                        for d in declarations {
                            if !var_names.contains(&d.name) {
                                var_names.push(d.name.clone());
                            }
                        }
                    } else {
                        for d in declarations {
                            add_lex(&mut lex_names, d.name.clone())?;
                        }
                    }
                }
                Statement::FunctionDecl { name, .. }
                | Statement::AsyncFunctionDecl { name, .. }
                | Statement::GeneratorDecl { name, .. }
                | Statement::AsyncGeneratorDecl { name, .. } => {
                    if !name.is_empty() {
                        add_lex(&mut lex_names, name.clone())?;
                    }
                }
                Statement::ClassDecl(class_def) => {
                    if let Some(ref name) = class_def.name {
                        add_lex(&mut lex_names, name.clone())?;
                    }
                }
                Statement::Import { specifiers, .. } => {
                    for (name, alias) in specifiers {
                        let bound = alias.as_ref().unwrap_or(name);
                        add_lex(&mut lex_names, bound.clone())?;
                    }
                }
                Statement::Export { decl, is_default, specifiers, source, all_as, .. } => {
                    if *is_default {
                        add_export(&mut exported_names, "default".to_string())?;
                    }
                    if let Some(alias) = all_as {
                        add_export(&mut exported_names, alias.clone())?;
                    }
                    if source.is_some() {
                        for (name, alias) in specifiers {
                            let exp = alias.as_ref().unwrap_or(name);
                            add_export(&mut exported_names, exp.clone())?;
                        }
                    } else if !specifiers.is_empty() {
                        for (name, alias) in specifiers {
                            let exp = alias.as_ref().unwrap_or(name);
                            add_export(&mut exported_names, exp.clone())?;
                            unresolvable_exports.push(name.clone());
                        }
                    }
                    if let Some(inner) = decl {
                        match &**inner {
                            Statement::VarDecl { kind, declarations } => {
                                if *kind == VarKind::Var {
                                    for d in declarations {
                                        if !var_names.contains(&d.name) {
                                            var_names.push(d.name.clone());
                                        }
                                        add_export(&mut exported_names, d.name.clone())?;
                                    }
                                } else {
                                    for d in declarations {
                                        add_lex(&mut lex_names, d.name.clone())?;
                                        add_export(&mut exported_names, d.name.clone())?;
                                    }
                                }
                            }
                            Statement::FunctionDecl { name, .. }
                            | Statement::AsyncFunctionDecl { name, .. }
                            | Statement::GeneratorDecl { name, .. }
                            | Statement::AsyncGeneratorDecl { name, .. } => {
                                if !name.is_empty() {
                                    add_lex(&mut lex_names, name.clone())?;
                                    if !*is_default {
                                        add_export(&mut exported_names, name.clone())?;
                                    }
                                }
                            }
                            Statement::ClassDecl(class_def) => {
                                if let Some(ref name) = class_def.name {
                                    add_lex(&mut lex_names, name.clone())?;
                                    if !*is_default {
                                        add_export(&mut exported_names, name.clone())?;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {
                    Self::collect_var_declared_names(stmt, &mut var_names);
                }
            }
        }

        // Check 1: Disjoint LexicallyDeclaredNames and VarDeclaredNames
        for lex in &lex_names {
            if var_names.contains(lex) {
                return Err(ParseError::General(format!("Identifier '{}' has already been declared", lex)));
            }
        }

        // Check 2: Unresolvable exports
        for exp_bind in unresolvable_exports {
            if !lex_names.contains(&exp_bind) && !var_names.contains(&exp_bind) {
                return Err(ParseError::General(format!("Exported identifier '{}' is not defined", exp_bind)));
            }
        }

        // Check 3: Check labels and undefined break/continue targets
        Self::validate_labels_and_jumps(body)?;

        Ok(())
    }

    fn validate_labels_and_jumps(stmts: &[Statement]) -> Result<(), ParseError> {
        let mut active_labels = Vec::new();
        for stmt in stmts {
            Self::check_labels_and_jumps_stmt(stmt, &mut active_labels, false, false)?;
        }
        Ok(())
    }

    fn check_labels_and_jumps_stmt(
        stmt: &Statement,
        active_labels: &mut Vec<String>,
        in_loop: bool,
        in_switch: bool,
    ) -> Result<(), ParseError> {
        match stmt {
            Statement::Labeled { label, body } => {
                if active_labels.contains(label) {
                    return Err(ParseError::General(format!("Duplicate label '{}'", label)));
                }
                active_labels.push(label.clone());
                let res = Self::check_labels_and_jumps_stmt(body, active_labels, in_loop, in_switch);
                active_labels.pop();
                res?;
            }
            Statement::Break(Some(label)) => {
                if !active_labels.contains(label) {
                    return Err(ParseError::General(format!("Undefined break label '{}'", label)));
                }
            }
            Statement::Break(None) => {
                if !in_loop && !in_switch {
                    return Err(ParseError::General("Illegal break statement".into()));
                }
            }
            Statement::Continue(Some(label)) => {
                if !active_labels.contains(label) {
                    return Err(ParseError::General(format!("Undefined continue label '{}'", label)));
                }
                if !in_loop {
                    return Err(ParseError::General("Illegal continue statement".into()));
                }
            }
            Statement::Continue(None) => {
                if !in_loop {
                    return Err(ParseError::General("Illegal continue statement".into()));
                }
            }
            Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
                Self::check_labels_and_jumps_stmt(body, active_labels, true, in_switch)?;
            }
            Statement::For { body, .. }
            | Statement::ForIn { body, .. }
            | Statement::ForOf { body, .. } => {
                Self::check_labels_and_jumps_stmt(body, active_labels, true, in_switch)?;
            }
            Statement::Block(inner) => {
                for s in inner {
                    Self::check_labels_and_jumps_stmt(s, active_labels, in_loop, in_switch)?;
                }
            }
            Statement::If { then_branch, else_branch, .. } => {
                Self::check_labels_and_jumps_stmt(then_branch, active_labels, in_loop, in_switch)?;
                if let Some(eb) = else_branch {
                    Self::check_labels_and_jumps_stmt(eb, active_labels, in_loop, in_switch)?;
                }
            }
            Statement::Switch { cases, .. } => {
                for c in cases {
                    for s in &c.consequent {
                        Self::check_labels_and_jumps_stmt(s, active_labels, in_loop, true)?;
                    }
                }
            }
            Statement::TryCatch { try_block, catch_block, finally_block, .. } => {
                Self::check_labels_and_jumps_stmt(try_block, active_labels, in_loop, in_switch)?;
                if let Some(cb) = catch_block {
                    Self::check_labels_and_jumps_stmt(cb, active_labels, in_loop, in_switch)?;
                }
                if let Some(fb) = finally_block {
                    Self::check_labels_and_jumps_stmt(fb, active_labels, in_loop, in_switch)?;
                }
            }
            Statement::With { body, .. } => {
                Self::check_labels_and_jumps_stmt(body, active_labels, in_loop, in_switch)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn parse_expression(&mut self, min_bp: u8) -> Result<Expression, ParseError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            // Postfix / Infix operations
            if self.check(&TokenKind::LParen) {
                if matches!(lhs, Expression::Super) && !self.in_derived_constructor {
                    return Err(ParseError::UnexpectedToken(TokenKind::Super, self.current.line, self.current.col));
                }
                // Function call
                self.advance()?;
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expression(1)?);
                        if !self.match_token(&TokenKind::Comma)? {
                            break;
                        }
                        if self.check(&TokenKind::RParen) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RParen)?;
                lhs = Expression::Call { callee: Box::new(lhs), args };
                continue;
            }

            if self.check(&TokenKind::Dot) {
                if matches!(lhs, Expression::Super) && !self.in_method && !self.in_derived_constructor {
                    return Err(ParseError::UnexpectedToken(TokenKind::Super, self.current.line, self.current.col));
                }
                self.advance()?;
                let prop_tok = self.advance()?;
                let prop = match prop_tok.kind.as_prop_name() {
                    Some(id) => id,
                    None => {
                        return Err(ParseError::UnexpectedToken(
                            prop_tok.kind,
                            prop_tok.line,
                            prop_tok.col,
                        ));
                    }
                };
                if prop.starts_with('#') && !self.private_names_stack.iter().any(|set| set.contains(&prop)) {
                    return Err(ParseError::UnexpectedToken(
                        prop_tok.kind,
                        prop_tok.line,
                        prop_tok.col,
                    ));
                }
                lhs = Expression::Member { object: Box::new(lhs), property: prop };
                continue;
            }

            if self.check(&TokenKind::LBracket) {
                if matches!(lhs, Expression::Super) && !self.in_method && !self.in_derived_constructor {
                    return Err(ParseError::UnexpectedToken(TokenKind::Super, self.current.line, self.current.col));
                }
                // Index access: lhs[expr]
                self.advance()?;
                let index = self.parse_expression(0)?;
                self.expect(TokenKind::RBracket)?;
                lhs = Expression::Index { object: Box::new(lhs), index: Box::new(index) };
                continue;
            }

            if matches!(lhs, Expression::Super) {
                return Err(ParseError::UnexpectedToken(TokenKind::Super, self.current.line, self.current.col));
            }

            if self.check(&TokenKind::Question) {
                if min_bp > 1 {
                    break;
                }
                self.advance()?; // ?
                let then_expr = self.parse_expression(1)?;
                self.expect(TokenKind::Colon)?;
                let else_expr = self.parse_expression(1)?;
                lhs = Expression::Conditional {
                    condition: Box::new(lhs),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                };
                continue;
            }

            if self.check(&TokenKind::PlusPlus) && !self.current.has_preceding_line_terminator {
                self.advance()?;
                if !matches!(&lhs, Expression::Identifier(_) | Expression::Member { .. } | Expression::Index { .. }) {
                    return Err(ParseError::UnexpectedToken(TokenKind::PlusPlus, self.current.line, self.current.col));
                }
                if let Expression::Identifier(ref name) = lhs {
                    if self.is_strict && (name == "eval" || name == "arguments") {
                        return Err(ParseError::UnexpectedToken(TokenKind::Ident(name.clone()), self.current.line, self.current.col));
                    }
                }
                lhs = Expression::Update {
                    op: UpdateOp::Inc,
                    is_prefix: false,
                    expr: Box::new(lhs),
                };
                continue;
            }

            if self.check(&TokenKind::MinusMinus) && !self.current.has_preceding_line_terminator {
                self.advance()?;
                if !matches!(&lhs, Expression::Identifier(_) | Expression::Member { .. } | Expression::Index { .. }) {
                    return Err(ParseError::UnexpectedToken(TokenKind::MinusMinus, self.current.line, self.current.col));
                }
                if let Expression::Identifier(ref name) = lhs {
                    if self.is_strict && (name == "eval" || name == "arguments") {
                        return Err(ParseError::UnexpectedToken(TokenKind::Ident(name.clone()), self.current.line, self.current.col));
                    }
                }
                lhs = Expression::Update {
                    op: UpdateOp::Dec,
                    is_prefix: false,
                    expr: Box::new(lhs),
                };
                continue;
            }

            let compound_bin_op = match self.current.kind {
                TokenKind::Eq => Some(None),
                TokenKind::PlusEq => Some(Some(BinaryOp::Add)),
                TokenKind::MinusEq => Some(Some(BinaryOp::Sub)),
                TokenKind::StarEq => Some(Some(BinaryOp::Mul)),
                TokenKind::SlashEq => Some(Some(BinaryOp::Div)),
                TokenKind::PercentEq => Some(Some(BinaryOp::Mod)),
                TokenKind::LtLtEq => Some(Some(BinaryOp::Shl)),
                TokenKind::GtGtEq => Some(Some(BinaryOp::Shr)),
                TokenKind::GtGtGtEq => Some(Some(BinaryOp::Ushr)),
                TokenKind::StarStarEq => Some(Some(BinaryOp::Exp)),
                TokenKind::AmpEq => Some(Some(BinaryOp::BitwiseAnd)),
                TokenKind::PipeEq => Some(Some(BinaryOp::BitwiseOr)),
                TokenKind::CaretEq => Some(Some(BinaryOp::BitwiseXor)),
                _ => None,
            };

            if let Some(opt_bin_op) = compound_bin_op {
                if min_bp > 1 {
                    break;
                }
                self.advance()?; // =, +=, -=, etc.
                let mut value = self.parse_expression(1)?;
                if let Some(bin_op) = opt_bin_op {
                    value = Expression::Binary {
                        op: bin_op,
                        left: Box::new(lhs.clone()),
                        right: Box::new(value),
                    };
                }
                match lhs {
                    Expression::Identifier(name) => {
                        if self.is_strict && (name == "eval" || name == "arguments") {
                            return Err(ParseError::UnexpectedToken(TokenKind::Ident(name), self.current.line, self.current.col));
                        }
                        lhs = Expression::Assignment { target: name, value: Box::new(value) };
                    }
                    Expression::Member { object, property } => {
                        lhs = Expression::MemberAssignment { object, property, value: Box::new(value) };
                    }
                    Expression::Index { object, index } => {
                        lhs = Expression::IndexAssignment { object, index, value: Box::new(value) };
                    }
                    _ => {
                        return Err(ParseError::InvalidAssignmentTarget(
                            self.current.line,
                            self.current.col,
                        ));
                    }
                }
                continue;
            }

            if min_bp == 0 && self.current.kind == TokenKind::Comma {
                self.advance()?;
                let next_expr = self.parse_expression(1)?;
                lhs = match lhs {
                    Expression::Sequence(mut exprs) => {
                        exprs.push(next_expr);
                        Expression::Sequence(exprs)
                    }
                    single => Expression::Sequence(vec![single, next_expr]),
                };
                continue;
            }

            let (op, l_bp, r_bp) = match self.current.kind {
                TokenKind::QuestionQuestion => (BinaryOp::NullishCoalescing, 2, 3),
                TokenKind::PipePipe => (BinaryOp::LogicalOr, 2, 3),
                TokenKind::AmpAmp => (BinaryOp::LogicalAnd, 4, 5),
                TokenKind::Pipe => (BinaryOp::BitwiseOr, 6, 7),
                TokenKind::Caret => (BinaryOp::BitwiseXor, 8, 9),
                TokenKind::Amp => (BinaryOp::BitwiseAnd, 10, 11),
                TokenKind::EqEq => (BinaryOp::Eq, 12, 13),
                TokenKind::NotEq => (BinaryOp::NotEq, 12, 13),
                TokenKind::EqEqEq => (BinaryOp::StrictEq, 12, 13),
                TokenKind::NotEqEq => (BinaryOp::StrictNotEq, 12, 13),
                TokenKind::Lt => (BinaryOp::Lt, 14, 15),
                TokenKind::LtEq => (BinaryOp::Lte, 14, 15),
                TokenKind::Gt => (BinaryOp::Gt, 14, 15),
                TokenKind::GtEq => (BinaryOp::Gte, 14, 15),
                TokenKind::Instanceof => (BinaryOp::Instanceof, 14, 15),
                TokenKind::In if self.allow_in => (BinaryOp::In, 14, 15),
                TokenKind::LtLt => (BinaryOp::Shl, 15, 16),
                TokenKind::GtGt => (BinaryOp::Shr, 15, 16),
                TokenKind::GtGtGt => (BinaryOp::Ushr, 15, 16),
                TokenKind::Plus => (BinaryOp::Add, 16, 17),
                TokenKind::Minus => (BinaryOp::Sub, 16, 17),
                TokenKind::Star => (BinaryOp::Mul, 18, 19),
                TokenKind::Slash => (BinaryOp::Div, 18, 19),
                TokenKind::Percent => (BinaryOp::Mod, 18, 19),
                TokenKind::StarStar => (BinaryOp::Exp, 21, 20),
                _ => break,
            };

            if l_bp < min_bp {
                break;
            }

            if op == BinaryOp::Exp && matches!(lhs, Expression::Unary { .. }) {
                return Err(ParseError::UnexpectedToken(TokenKind::StarStar, self.current.line, self.current.col));
            }

            self.advance()?;
            let rhs = self.parse_expression(r_bp)?;
            lhs = Expression::Binary { op, left: Box::new(lhs), right: Box::new(rhs) };
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expression, ParseError> {
        let tok = self.advance()?;
        match tok.kind {
            TokenKind::Number(n) => Ok(Expression::Literal(Literal::Number(n))),
            TokenKind::BigInt(b) => Ok(Expression::Literal(Literal::BigInt(b))),
            TokenKind::StringLit { value, has_escape, has_legacy_escape, .. } => Ok(Expression::Literal(Literal::String { value, has_escape, has_legacy_escape })),
            TokenKind::TemplateLiteral { quasis, raw_exprs } => {
                if raw_exprs.is_empty() {
                    let s = quasis.into_iter().next().unwrap_or_default();
                    return Ok(Expression::Literal(Literal::String { value: s, has_escape: false, has_legacy_escape: false }));
                }

                let mut expr: Option<Expression> = None;
                for (i, quasi) in quasis.into_iter().enumerate() {
                    if !quasi.is_empty() || expr.is_none() {
                        let lit = Expression::Literal(Literal::String { value: quasi, has_escape: false, has_legacy_escape: false });
                        expr = match expr {
                            Some(prev) => Some(Expression::Binary {
                                op: BinaryOp::Add,
                                left: Box::new(prev),
                                right: Box::new(lit),
                            }),
                            None => Some(lit),
                        };
                    }
                    if i < raw_exprs.len() {
                        let sub_expr = Parser::new(&raw_exprs[i])?.parse_expression(0)?;
                        expr = match expr {
                            Some(prev) => Some(Expression::Binary {
                                op: BinaryOp::Add,
                                left: Box::new(prev),
                                right: Box::new(sub_expr),
                            }),
                            None => Some(sub_expr),
                        };
                    }
                }
                Ok(expr.unwrap_or(Expression::Literal(Literal::String { value: String::new(), has_escape: false, has_legacy_escape: false })))
            }
            TokenKind::RegExp { pattern, flags } => Ok(Expression::Literal(Literal::RegExp { pattern, flags })),
            TokenKind::True => Ok(Expression::Literal(Literal::Boolean(true))),
            TokenKind::False => Ok(Expression::Literal(Literal::Boolean(false))),
            TokenKind::Null => Ok(Expression::Literal(Literal::Null)),
            TokenKind::Undefined => Ok(Expression::Literal(Literal::Undefined)),
            TokenKind::Ident(s) => {
                if self.is_strict && is_strict_reserved_word(&s) {
                    return Err(ParseError::UnexpectedToken(TokenKind::Ident(s), tok.line, tok.col));
                }
                if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
                    let prev_in_func = self.in_function;
                    self.in_function = true;
                    let body = if self.check(&TokenKind::LBrace) {
                        match self.parse_statement()? {
                            Statement::Block(stmts) => stmts,
                            other => vec![other],
                        }
                    } else {
                        let expr = self.parse_expression(1)?;
                        vec![Statement::Return(Some(expr))]
                    };
                    self.in_function = prev_in_func;
                    Ok(Expression::ArrowExpr {
                        params: vec![s],
                        body,
                    })
                } else {
                    Ok(Expression::Identifier(s))
                }
            }
            TokenKind::This => Ok(Expression::This),
            TokenKind::Super => {
                if self.check(&TokenKind::LParen) {
                    if !self.in_derived_constructor {
                        return Err(ParseError::UnexpectedToken(TokenKind::Super, tok.line, tok.col));
                    }
                } else if self.check(&TokenKind::Dot) || self.check(&TokenKind::LBracket) {
                    if !self.in_method && !self.in_derived_constructor {
                        return Err(ParseError::UnexpectedToken(TokenKind::Super, tok.line, tok.col));
                    }
                } else {
                    return Err(ParseError::UnexpectedToken(TokenKind::Super, tok.line, tok.col));
                }
                Ok(Expression::Super)
            }
            TokenKind::LBracket => self.parse_array_literal(),
            TokenKind::LBrace => self.parse_object_literal(),
            TokenKind::Function => self.parse_function_expr(),
            TokenKind::Class => self.parse_class_expr(),
            TokenKind::PlusPlus => {
                let expr = self.parse_expression(22)?;
                if !matches!(&expr, Expression::Identifier(_) | Expression::Member { .. } | Expression::Index { .. }) {
                    return Err(ParseError::UnexpectedToken(TokenKind::PlusPlus, tok.line, tok.col));
                }
                if let Expression::Identifier(ref name) = expr {
                    if self.is_strict && (name == "eval" || name == "arguments") {
                        return Err(ParseError::UnexpectedToken(TokenKind::Ident(name.clone()), self.current.line, self.current.col));
                    }
                }
                Ok(Expression::Update {
                    op: UpdateOp::Inc,
                    is_prefix: true,
                    expr: Box::new(expr),
                })
            }
            TokenKind::MinusMinus => {
                let expr = self.parse_expression(22)?;
                if !matches!(&expr, Expression::Identifier(_) | Expression::Member { .. } | Expression::Index { .. }) {
                    return Err(ParseError::UnexpectedToken(TokenKind::MinusMinus, tok.line, tok.col));
                }
                if let Expression::Identifier(ref name) = expr {
                    if self.is_strict && (name == "eval" || name == "arguments") {
                        return Err(ParseError::UnexpectedToken(TokenKind::Ident(name.clone()), self.current.line, self.current.col));
                    }
                }
                Ok(Expression::Update {
                    op: UpdateOp::Dec,
                    is_prefix: true,
                    expr: Box::new(expr),
                })
            }
            TokenKind::Plus => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::Pos, expr: Box::new(expr) })
            }
            TokenKind::Minus => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::Neg, expr: Box::new(expr) })
            }
            TokenKind::Bang => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::Not, expr: Box::new(expr) })
            }
            TokenKind::Tilde => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::BitNot, expr: Box::new(expr) })
            }
            TokenKind::Typeof => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::Typeof, expr: Box::new(expr) })
            }
            TokenKind::Void => {
                let expr = self.parse_expression(22)?;
                Ok(Expression::Unary { op: UnaryOp::Void, expr: Box::new(expr) })
            }
            TokenKind::Delete => {
                let expr = self.parse_expression(22)?;
                if self.is_strict && matches!(&expr, Expression::Identifier(_)) {
                    return Err(ParseError::UnexpectedToken(TokenKind::Delete, self.current.line, self.current.col));
                }
                Ok(Expression::Unary { op: UnaryOp::Delete, expr: Box::new(expr) })
            }
            TokenKind::LParen => {
                if self.match_token(&TokenKind::RParen)? {
                    if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
                        let prev_in_func = self.in_function;
                        self.in_function = true;
                        let body = if self.check(&TokenKind::LBrace) {
                            match self.parse_statement()? {
                                Statement::Block(stmts) => stmts,
                                other => vec![other],
                            }
                        } else {
                            let expr = self.parse_expression(1)?;
                            vec![Statement::Return(Some(expr))]
                        };
                        self.in_function = prev_in_func;
                        return Ok(Expression::ArrowExpr {
                            params: vec![],
                            body,
                        });
                    }
                    return Err(ParseError::UnexpectedToken(TokenKind::RParen, tok.line, tok.col));
                }
                let mut exprs = Vec::new();
                let mut rest_param = None;
                if self.match_token(&TokenKind::DotDotDot)? {
                    let id = self.expect_identifier()?;
                    rest_param = Some(format!("...{}", id));
                } else {
                    exprs.push(self.parse_expression(1)?);
                    while self.match_token(&TokenKind::Comma)? {
                        if self.check(&TokenKind::RParen) {
                            break;
                        }
                        if self.match_token(&TokenKind::DotDotDot)? {
                            let id = self.expect_identifier()?;
                            rest_param = Some(format!("...{}", id));
                            break;
                        }
                        exprs.push(self.parse_expression(1)?);
                    }
                }
                self.expect(TokenKind::RParen)?;
                if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
                    let mut params = Vec::new();
                    let mut defaults = Vec::new();
                    for e in &exprs {
                        match e {
                            Expression::Identifier(name) => params.push(name.clone()),
                            Expression::Assignment { target, value } => {
                                params.push(target.clone());
                                defaults.push(Statement::If {
                                    condition: Expression::Binary {
                                        op: BinaryOp::StrictEq,
                                        left: Box::new(Expression::Identifier(target.clone())),
                                        right: Box::new(Expression::Literal(Literal::Undefined)),
                                    },
                                    then_branch: Box::new(Statement::Expression(Expression::Assignment {
                                        target: target.clone(),
                                        value: value.clone(),
                                    })),
                                    else_branch: None,
                                });
                            }
                            Expression::Array(_) | Expression::Object(_) => {
                                let name = format!("__param_{}", self.destruct_counter);
                                self.destruct_counter += 1;
                                params.push(name.clone());
                                defaults.push(Statement::If {
                                    condition: Expression::Binary {
                                        op: BinaryOp::Eq,
                                        left: Box::new(Expression::Identifier(name)),
                                        right: Box::new(Expression::Literal(Literal::Null)),
                                    },
                                    then_branch: Box::new(Statement::Throw(Expression::New {
                                        callee: Box::new(Expression::Identifier("TypeError".to_string())),
                                        args: vec![Expression::Literal(Literal::String {
                                            value: "Cannot destructure 'undefined' or 'null'".to_string(),
                                            has_escape: false,
                                            has_legacy_escape: false,
                                        })],
                                    })),
                                    else_branch: None,
                                });
                            }
                            _ => {}
                        }
                    }
                    if let Some(r) = rest_param {
                        params.push(r);
                    }
                    let prev_in_func = self.in_function;
                    self.in_function = true;
                    let mut body = if self.check(&TokenKind::LBrace) {
                        self.parse_function_body(&params, None)?
                    } else {
                        let expr = self.parse_expression(1)?;
                        vec![Statement::Return(Some(expr))]
                    };
                    self.in_function = prev_in_func;
                    Self::apply_parameter_defaults(&mut body, defaults);
                    return Ok(Expression::ArrowExpr {
                        params,
                        body,
                    });
                }
                if rest_param.is_some() {
                    return Err(ParseError::UnexpectedToken(TokenKind::RParen, tok.line, tok.col));
                }
                if exprs.len() == 1 {
                    Ok(Expression::Grouping(Box::new(exprs.pop().unwrap())))
                } else {
                    Ok(Expression::Sequence(exprs))
                }
            }
            TokenKind::Async => {
                if self.match_token(&TokenKind::Function)? {
                    let is_generator = self.match_token(&TokenKind::Star)?;
                    let name = if let TokenKind::Ident(id) = &self.current.kind {
                        if self.is_strict && (is_strict_reserved_word(id) || id == "eval" || id == "arguments") {
                            return Err(ParseError::UnexpectedToken(TokenKind::Ident(id.clone()), self.current.line, self.current.col));
                        }
                        let n = id.clone();
                        self.advance()?;
                        Some(n)
                    } else {
                        None
                    };

                    let (params, defaults) = self.parse_formal_parameters()?;
                    let mut body = self.parse_function_body(&params, name.as_deref())?;
                    Self::apply_parameter_defaults(&mut body, defaults);

                    if is_generator {
                        Ok(Expression::AsyncGeneratorExpr { name, params, body })
                    } else {
                        Ok(Expression::AsyncFunctionExpr { name, params, body })
                    }
                } else {
                    let expr = self.parse_expression(20)?;
                    Ok(expr)
                }
            }
            TokenKind::New => {
                if self.match_token(&TokenKind::Dot)? {
                    let ident = self.expect_identifier()?;
                    if ident == "target" {
                        if !self.in_non_arrow_function {
                            return Err(ParseError::UnexpectedToken(TokenKind::Ident(ident), tok.line, tok.col));
                        }
                        return Ok(Expression::NewTarget);
                    } else {
                        return Err(ParseError::UnexpectedToken(TokenKind::Ident(ident), tok.line, tok.col));
                    }
                }
                let mut callee = self.parse_prefix()?;
                while self.check(&TokenKind::Dot) {
                    self.advance()?;
                    let prop_tok = self.advance()?;
                    let prop = match prop_tok.kind.as_prop_name() {
                        Some(id) => id,
                        None => return Err(ParseError::UnexpectedToken(prop_tok.kind, prop_tok.line, prop_tok.col)),
                    };
                    if prop.starts_with('#') && !self.private_names_stack.iter().any(|set| set.contains(&prop)) {
                        return Err(ParseError::UnexpectedToken(
                            prop_tok.kind,
                            prop_tok.line,
                            prop_tok.col,
                        ));
                    }
                    callee = Expression::Member { object: Box::new(callee), property: prop };
                }
                let mut args = Vec::new();
                if self.match_token(&TokenKind::LParen)? {
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expression(1)?);
                            if !self.match_token(&TokenKind::Comma)? {
                                break;
                            }
                            if self.check(&TokenKind::RParen) {
                                break;
                            }
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                }
                Ok(Expression::New { callee: Box::new(callee), args })
            }
            TokenKind::As => Ok(Expression::Identifier("as".to_string())),
            TokenKind::From => Ok(Expression::Identifier("from".to_string())),
            TokenKind::Implements
            | TokenKind::Interface
            | TokenKind::Package
            | TokenKind::Private
            | TokenKind::Protected
            | TokenKind::Public
            | TokenKind::Static => {
                if self.is_strict {
                    return Err(ParseError::UnexpectedToken(tok.kind, tok.line, tok.col));
                }
                let s = tok.kind.as_prop_name().unwrap_or_default();
                if !self.current.has_preceding_line_terminator && self.match_token(&TokenKind::FatArrow)? {
                    let prev_in_func = self.in_function;
                    self.in_function = true;
                    let body = if self.check(&TokenKind::LBrace) {
                        match self.parse_statement()? {
                            Statement::Block(stmts) => stmts,
                            other => vec![other],
                        }
                    } else {
                        let expr = self.parse_expression(1)?;
                        vec![Statement::Return(Some(expr))]
                    };
                    self.in_function = prev_in_func;
                    Ok(Expression::ArrowExpr {
                        params: vec![s],
                        body,
                    })
                } else {
                    Ok(Expression::Identifier(s))
                }
            }
            TokenKind::Await => {
                let can_await = (self.in_async || (self.is_module && !self.in_function)) && !self.in_param;
                if can_await {
                    let expr = self.parse_expression(20)?;
                    Ok(Expression::Await(Box::new(expr)))
                } else if self.is_module || self.is_strict {
                    Err(ParseError::UnexpectedToken(TokenKind::Await, tok.line, tok.col))
                } else {
                    Ok(Expression::Identifier("await".to_string()))
                }
            }
            TokenKind::Yield => {
                if self.is_strict && !self.in_generator {
                    return Err(ParseError::UnexpectedToken(TokenKind::Yield, tok.line, tok.col));
                }
                if !self.in_generator {
                    if self.check(&TokenKind::Eq)
                        || self.check(&TokenKind::Semicolon)
                        || self.check(&TokenKind::Comma)
                        || self.check(&TokenKind::RParen)
                        || self.check(&TokenKind::RBracket)
                        || self.check(&TokenKind::RBrace)
                        || self.check(&TokenKind::Eof)
                    {
                        return Ok(Expression::Identifier("yield".to_string()));
                    }
                }
                if self.check(&TokenKind::Semicolon)
                    || self.check(&TokenKind::RParen)
                    || self.check(&TokenKind::RBrace)
                    || self.check(&TokenKind::Eof)
                {
                    Ok(Expression::Yield(None))
                } else {
                    let expr = self.parse_expression(0)?;
                    Ok(Expression::Yield(Some(Box::new(expr))))
                }
            }
            TokenKind::Import => {
                self.expect(TokenKind::LParen)?;
                let spec = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                Ok(Expression::DynamicImport(Box::new(spec)))
            }
            TokenKind::DotDotDot => {
                let expr = self.parse_expression(1)?;
                Ok(Expression::Spread(Box::new(expr)))
            }
            other => Err(ParseError::UnexpectedToken(other, tok.line, tok.col)),
        }
    }

    fn parse_object_literal(&mut self) -> Result<Expression, ParseError> {
        let mut props = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            // Check for getter: get <prop>() { ... }
            if self.check_ident("get")
                && self.peeked.kind != TokenKind::Colon
                && self.peeked.kind != TokenKind::LParen
                && self.peeked.kind != TokenKind::Comma
                && self.peeked.kind != TokenKind::RBrace
            {
                self.advance()?; // 'get'
                let key = self.parse_property_name()?;
                self.expect(TokenKind::LParen)?;
                self.expect(TokenKind::RParen)?;
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                self.in_method = true;
                self.in_derived_constructor = false;
                let body = self.parse_function_body(&[], None)?;
                self.in_method = prev_method;
                self.in_derived_constructor = prev_ctor;
                props.push(ObjectProperty::Getter(
                    key,
                    Expression::FunctionExpr {
                        name: None,
                        params: vec![],
                        body,
                    },
                ));
                if self.check(&TokenKind::Comma) {
                    self.advance()?;
                }
                continue;
            }

            // Check for setter: set <prop>(param) { ... }
            if self.check_ident("set")
                && self.peeked.kind != TokenKind::Colon
                && self.peeked.kind != TokenKind::LParen
                && self.peeked.kind != TokenKind::Comma
                && self.peeked.kind != TokenKind::RBrace
            {
                self.advance()?; // 'set'
                let key = self.parse_property_name()?;
                self.expect(TokenKind::LParen)?;
                let param = self.expect_identifier()?;
                self.expect(TokenKind::RParen)?;
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                self.in_method = true;
                self.in_derived_constructor = false;
                let body = self.parse_function_body(&[param.clone()], None)?;
                self.in_method = prev_method;
                self.in_derived_constructor = prev_ctor;
                props.push(ObjectProperty::Setter(
                    key,
                    Expression::FunctionExpr {
                        name: None,
                        params: vec![param],
                        body,
                    },
                ));
                if self.check(&TokenKind::Comma) {
                    self.advance()?;
                }
                continue;
            }

            // Check for generator method: * <prop>() { ... }
            if self.match_token(&TokenKind::Star)? {
                let key = self.parse_property_name()?;
                let (params, defaults) = self.parse_formal_parameters()?;
                let mut body = self.parse_function_body(&params, None)?;
                Self::apply_parameter_defaults(&mut body, defaults);
                let fn_name = match &key {
                    PropertyKey::Literal(s) => Some(s.clone()),
                    PropertyKey::Computed(_) => None,
                };
                props.push(ObjectProperty::KeyValue(
                    key,
                    Expression::GeneratorExpr {
                        name: fn_name,
                        params,
                        body,
                    },
                ));
                if !self.match_token(&TokenKind::Comma)? || self.check(&TokenKind::RBrace) {
                    break;
                }
                continue;
            }

            // Check for async method: async <prop>() { ... } or async * <prop>() { ... }
            if self.check(&TokenKind::Async)
                && self.peeked.kind != TokenKind::Colon
                && self.peeked.kind != TokenKind::Comma
                && self.peeked.kind != TokenKind::RBrace
                && self.peeked.kind != TokenKind::LParen
            {
                self.advance()?; // 'async'
                let is_gen = self.match_token(&TokenKind::Star)?;
                let key = self.parse_property_name()?;
                let (params, defaults) = self.parse_formal_parameters()?;
                let mut body = self.parse_function_body(&params, None)?;
                Self::apply_parameter_defaults(&mut body, defaults);
                let fn_name = match &key {
                    PropertyKey::Literal(s) => Some(s.clone()),
                    PropertyKey::Computed(_) => None,
                };
                if is_gen {
                    props.push(ObjectProperty::KeyValue(
                        key,
                        Expression::AsyncGeneratorExpr { name: fn_name, params, body },
                    ));
                } else {
                    props.push(ObjectProperty::KeyValue(
                        key,
                        Expression::AsyncFunctionExpr { name: fn_name, params, body },
                    ));
                }
                if !self.match_token(&TokenKind::Comma)? || self.check(&TokenKind::RBrace) {
                    break;
                }
                continue;
            }

            let (key, is_ident) = self.parse_property_name_meta()?;
            let val = if self.match_token(&TokenKind::Colon)? {
                self.parse_expression(1)?
            } else if self.check(&TokenKind::LParen) {
                // Method shorthand: foo() { ... } or [expr]() { ... }
                let (params, defaults) = self.parse_formal_parameters()?;
                let prev_ctor = self.in_derived_constructor;
                let prev_method = self.in_method;
                self.in_method = true;
                self.in_derived_constructor = false;
                let mut body = self.parse_function_body(&params, None)?;
                self.in_method = prev_method;
                self.in_derived_constructor = prev_ctor;
                Self::apply_parameter_defaults(&mut body, defaults);
                let fn_name = match &key {
                    PropertyKey::Literal(s) => Some(s.clone()),
                    PropertyKey::Computed(_) => None,
                };
                Expression::MethodExpr {
                    name: fn_name,
                    params,
                    body,
                }
            } else if is_ident {
                match &key {
                    PropertyKey::Literal(id) => Expression::Identifier(id.clone()),
                    _ => unreachable!(),
                }
            } else {
                return Err(ParseError::UnexpectedToken(self.current.kind.clone(), self.current.line, self.current.col));
            };

            props.push(ObjectProperty::KeyValue(key, val));
            if !self.match_token(&TokenKind::Comma)? || self.check(&TokenKind::RBrace) {
                break;
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Expression::Object(props))
    }

    fn parse_array_literal(&mut self) -> Result<Expression, ParseError> {
        let mut elements = Vec::new();
        while !self.check(&TokenKind::RBracket) && self.current.kind != TokenKind::Eof {
            if self.match_token(&TokenKind::Comma)? {
                elements.push(Expression::Literal(Literal::Undefined));
                continue;
            }
            elements.push(self.parse_expression(1)?);
            if self.match_token(&TokenKind::Comma)? {
                continue;
            } else {
                break;
            }
        }
        self.expect(TokenKind::RBracket)?;
        Ok(Expression::Array(elements))
    }

    fn parse_function_expr(&mut self) -> Result<Expression, ParseError> {
        let is_generator = self.match_token(&TokenKind::Star)?;
        let prev_gen = self.in_generator;
        if is_generator {
            self.in_generator = true;
        }
        let name = if let TokenKind::Ident(id) = &self.current.kind {
            if self.is_strict && (is_strict_reserved_word(id) || id == "eval" || id == "arguments") {
                return Err(ParseError::UnexpectedToken(TokenKind::Ident(id.clone()), self.current.line, self.current.col));
            }
            let n = id.clone();
            self.advance()?;
            Some(n)
        } else {
            None
        };

        let (params, defaults) = self.parse_formal_parameters()?;
        let mut body = self.parse_function_body(&params, name.as_deref())?;
        Self::apply_parameter_defaults(&mut body, defaults);

        self.in_generator = prev_gen;
        if is_generator {
            Ok(Expression::GeneratorExpr { name, params, body })
        } else {
            Ok(Expression::FunctionExpr { name, params, body })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_binary_precedence() {
        let mut parser = Parser::new("1 + 2 * 3").unwrap();
        let expr = parser.parse_expression(0).unwrap();
        assert_eq!(
            expr,
            Expression::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expression::Literal(Literal::Number(1.0))),
                right: Box::new(Expression::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(Expression::Literal(Literal::Number(2.0))),
                    right: Box::new(Expression::Literal(Literal::Number(3.0))),
                }),
            }
        );
    }

    #[test]
    fn test_parse_function_and_call() {
        let mut parser = Parser::new("function add(a, b) { return a + b; } add(10, 20);").unwrap();
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.body.len(), 2);
    }

    #[test]
    fn test_parse_object_and_array() {
        let mut parser = Parser::new("let obj = { x: 1, y: [10, 20] };").unwrap();
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.body.len(), 1);
    }

    #[test]
    fn test_parse_for_and_ternary() {
        let mut parser = Parser::new("for (let i = 0; i < 10; i = i + 1) { let res = i > 5 ? 1 : 0; }").unwrap();
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.body.len(), 1);
    }

    #[test]
    fn test_parse_async_and_modules() {
        let code = "async function fetchData() { let res = await p; return res; } import { a, b as c } from 'module'; export default 42; new Promise(fn);";
        let mut parser = Parser::new_module(code).unwrap();
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.body.len(), 4);
    }

    #[test]
    fn test_parse_default_parameters() {
        let code = "function foo(a, b = 42) { return a + b; } const bar = (x = 10) => x * 2;";
        let mut parser = Parser::new(code).unwrap();
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.body.len(), 2);
    }
}
