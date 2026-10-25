use super::{ParseError, Parser, is_strict_reserved_word};
use crate::ast::*;
use crate::lexer::TokenKind;

impl<'a> Parser<'a> {
    pub(crate) fn skip_balanced(
        &mut self,
        open: TokenKind,
        close: TokenKind,
    ) -> Result<(), ParseError> {
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

    pub(crate) fn parse_formal_parameters(
        &mut self,
    ) -> Result<(Vec<String>, Vec<Statement>), ParseError> {
        let prev_in_param = self.in_param;
        self.in_param = true;
        self.current_params_simple = true;
        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        let mut defaults = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                if self.match_token(&TokenKind::DotDotDot)? {
                    self.current_params_simple = false;
                    let mut destruct_decls = Vec::new();
                    let (name, is_destruct) = if self.match_token(&TokenKind::LBracket)? {
                        let name = format!("__rest_{}", self.destruct_counter);
                        self.destruct_counter += 1;
                        self.parse_array_destructuring_into(&name, &mut destruct_decls)?;
                        (name, true)
                    } else if self.match_token(&TokenKind::LBrace)? {
                        let name = format!("__rest_{}", self.destruct_counter);
                        self.destruct_counter += 1;
                        self.parse_object_destructuring_into(&name, &mut destruct_decls)?;
                        (name, true)
                    } else {
                        let id = self.expect_identifier()?;
                        if self.match_token(&TokenKind::Colon)? {
                            self.skip_type_annotation()?;
                        }
                        (id, false)
                    };
                    params.push(format!("...{}", name));
                    if is_destruct {
                        defaults.push(Statement::If {
                            condition: Expression::Binary {
                                op: BinaryOp::Eq,
                                left: Box::new(Expression::Identifier(name.clone())),
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
                    break;
                }
                while Self::is_ts_modifier(&self.current.kind)
                    && matches!(
                        &self.peeked.kind,
                        TokenKind::Ident(_) | TokenKind::LBracket | TokenKind::LBrace
                    )
                {
                    self.advance()?;
                }
                let mut destruct_decls = Vec::new();
                let (id, is_destruct) = if self.match_token(&TokenKind::LBracket)? {
                    self.current_params_simple = false;
                    let name = format!("__param_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    self.parse_array_destructuring_into(&name, &mut destruct_decls)?;
                    (name, true)
                } else if self.match_token(&TokenKind::LBrace)? {
                    self.current_params_simple = false;
                    let name = format!("__param_{}", self.destruct_counter);
                    self.destruct_counter += 1;
                    self.parse_object_destructuring_into(&name, &mut destruct_decls)?;
                    (name, true)
                } else {
                    (self.expect_identifier()?, false)
                };
                if self.match_token(&TokenKind::Question)? {
                    // Optional TypeScript parameter `x?: Type`
                }
                if self.match_token(&TokenKind::Colon)? {
                    self.skip_type_annotation()?;
                }
                if self.match_token(&TokenKind::Eq)? {
                    self.current_params_simple = false;
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

    pub(crate) fn apply_parameter_defaults(body: &mut Vec<Statement>, defaults: Vec<Statement>) {
        if defaults.is_empty() {
            return;
        }
        let mut insert_idx = 0;
        for stmt in body.iter() {
            if let Statement::Expression(Expression::Literal(Literal::String {
                value,
                has_escape,
                ..
            })) = stmt
            {
                if value == "use strict" && !has_escape {
                    insert_idx += 1;
                    continue;
                }
            }
            break;
        }
        body.splice(insert_idx..insert_idx, defaults);
    }

    pub(crate) fn parse_function_signature_and_body_opt_name(
        &mut self,
        allow_anon: bool,
    ) -> Result<(String, Vec<String>, Vec<Statement>), ParseError> {
        let prev_in_method = self.in_method;
        let prev_in_class_ctor = self.in_class_constructor;
        let prev_in_derived_ctor = self.in_derived_constructor;
        self.in_method = false;
        self.in_class_constructor = false;
        self.in_derived_constructor = false;
        let res = (|| -> Result<(String, Vec<String>, Vec<Statement>), ParseError> {
            let name = if allow_anon && !matches!(&self.current.kind, TokenKind::Ident(_)) {
                "".to_string()
            } else {
                self.expect_identifier()?
            };
            if self.check(&TokenKind::Lt) {
                self.skip_type_parameters()?;
            }
            let (params, defaults) = self.parse_formal_parameters()?;
            if self.match_token(&TokenKind::Colon)? {
                self.skip_type_annotation()?;
            }
            let mut body =
                self.parse_function_body(&params, if name.is_empty() { None } else { Some(&name) })?;
            Self::apply_parameter_defaults(&mut body, defaults);
            Ok((name, params, body))
        })();
        self.in_method = prev_in_method;
        self.in_class_constructor = prev_in_class_ctor;
        self.in_derived_constructor = prev_in_derived_ctor;
        res
    }

    pub(crate) fn parse_function_signature_and_body(
        &mut self,
    ) -> Result<(String, Vec<String>, Vec<Statement>), ParseError> {
        self.parse_function_signature_and_body_opt_name(false)
    }

    pub(crate) fn validate_strict_params_and_name(
        params: &[String],
        name: Option<&str>,
        line: usize,
        col: usize,
    ) -> Result<(), ParseError> {
        if let Some(n) = name {
            if is_strict_reserved_word(n) || n == "eval" || n == "arguments" {
                return Err(ParseError::UnexpectedToken(
                    TokenKind::Ident(n.to_string()),
                    line,
                    col,
                ));
            }
        }
        let mut seen = std::collections::HashSet::new();
        for p in params {
            let clean = p.trim_start_matches("...");
            if is_strict_reserved_word(clean) || clean == "eval" || clean == "arguments" {
                return Err(ParseError::UnexpectedToken(
                    TokenKind::Ident(clean.to_string()),
                    line,
                    col,
                ));
            }
            if !seen.insert(clean) {
                return Err(ParseError::UnexpectedToken(
                    TokenKind::Ident(clean.to_string()),
                    line,
                    col,
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_duplicate_params(
        params: &[String],
        line: usize,
        col: usize,
    ) -> Result<(), ParseError> {
        let mut seen = std::collections::HashSet::new();
        for p in params {
            let clean = p.trim_start_matches("...");
            if !seen.insert(clean) {
                return Err(ParseError::UnexpectedToken(
                    TokenKind::Ident(clean.to_string()),
                    line,
                    col,
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn parse_function_body(
        &mut self,
        params: &[String],
        name: Option<&str>,
    ) -> Result<Vec<Statement>, ParseError> {
        self.parse_function_body_internal(params, name, false)
    }

    pub(crate) fn parse_arrow_function_body(
        &mut self,
        params: &[String],
    ) -> Result<Vec<Statement>, ParseError> {
        self.parse_function_body_internal(params, None, true)
    }

    fn parse_function_body_internal(
        &mut self,
        params: &[String],
        name: Option<&str>,
        is_arrow: bool,
    ) -> Result<Vec<Statement>, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let prev_strict = self.is_strict;
        let prev_in_func = self.in_function;
        let prev_in_non_arrow = self.in_non_arrow_function;
        let prev_in_static = self.in_static_block;
        self.in_function = true;
        self.in_static_block = false;
        if !is_arrow {
            self.in_non_arrow_function = true;
        }
        if prev_strict {
            Self::validate_strict_params_and_name(
                params,
                name,
                self.current.line,
                self.current.col,
            )?;
        } else if self.in_generator || self.in_async || !self.current_params_simple {
            Self::validate_duplicate_params(params, self.current.line, self.current.col)?;
        }
        let mut body = Vec::new();
        let mut in_directive_prologue = true;
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            let stmt = self.parse_statement()?;
            if in_directive_prologue {
                if let Statement::Expression(Expression::Literal(Literal::String {
                    ref value,
                    has_escape,
                    has_legacy_escape,
                })) = stmt
                {
                    if has_legacy_escape && self.is_strict {
                        return Err(ParseError::UnexpectedToken(
                            TokenKind::StringLit {
                                value: value.clone(),
                                has_escape,
                                has_legacy_escape,
                                has_unpaired_surrogate: false,
                            },
                            self.current.line,
                            self.current.col,
                        ));
                    }
                    if value == "use strict" && !has_escape {
                        if !self.current_params_simple {
                            return Err(ParseError::General(
                                "Illegal 'use strict' directive in function with non-simple parameter list".to_string(),
                            ));
                        }
                        if !self.is_strict {
                            self.is_strict = true;
                            self.lexer.is_strict = true;
                            if body.iter().any(|s| {
                                matches!(
                                    s,
                                    Statement::Expression(Expression::Literal(Literal::String {
                                        has_legacy_escape: true,
                                        ..
                                    }))
                                )
                            }) {
                                return Err(ParseError::UnexpectedToken(
                                    TokenKind::StringLit {
                                        value: value.clone(),
                                        has_escape,
                                        has_legacy_escape,
                                        has_unpaired_surrogate: false,
                                    },
                                    self.current.line,
                                    self.current.col,
                                ));
                            }
                            Self::validate_strict_params_and_name(
                                params,
                                name,
                                self.current.line,
                                self.current.col,
                            )?;
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
        self.in_static_block = prev_in_static;

        self.validate_block_declarations(&body, true)?;
        let mut lex_names = Vec::new();
        for stmt in &body {
            match stmt {
                Statement::VarDecl { kind, declarations } if *kind != VarKind::Var => {
                    for decl in declarations {
                        if !decl.name.starts_with("__") && !decl.name.starts_with('@') {
                            lex_names.push(decl.name.clone());
                        }
                    }
                }
                Statement::ClassDecl(class_def) => {
                    if let Some(ref name) = class_def.name {
                        lex_names.push(name.clone());
                    }
                }
                Statement::FunctionDecl { name, .. } if self.is_strict => {
                    lex_names.push(name.clone());
                }
                Statement::AsyncFunctionDecl { name, .. }
                | Statement::GeneratorDecl { name, .. }
                | Statement::AsyncGeneratorDecl { name, .. } => {
                    lex_names.push(name.clone());
                }
                _ => {}
            }
        }
        for p in params {
            let param_name = if let Some(rest) = p.strip_prefix("...") { rest } else { p.as_str() };
            if lex_names.iter().any(|n| n == param_name) {
                return Err(ParseError::General(format!(
                    "Identifier '{}' has already been declared",
                    param_name
                )));
            }
        }
        Ok(body)
    }

    pub(crate) fn parse_function_decl_body(&mut self) -> Result<Statement, ParseError> {
        let prev_gen = self.in_generator;
        let prev_async = self.in_async;
        self.in_generator = false;
        self.in_async = false;
        let res = self.parse_function_signature_and_body();
        self.in_generator = prev_gen;
        self.in_async = prev_async;
        let (name, params, body) = res?;
        Ok(Statement::FunctionDecl { name, params, body })
    }

    pub(crate) fn parse_async_function_decl(&mut self) -> Result<Statement, ParseError> {
        self.expect(TokenKind::Function)?;
        let is_generator = self.match_token(&TokenKind::Star)?;
        let prev_gen = self.in_generator;
        let prev_async = self.in_async;
        let prev_in_static = self.in_static_block;
        self.in_static_block = false;
        let name = self.expect_identifier()?;
        self.in_generator = is_generator;
        self.in_async = true;
        if self.check(&TokenKind::Lt) {
            self.skip_type_parameters()?;
        }
        let (params, defaults) = self.parse_formal_parameters()?;
        if self.match_token(&TokenKind::Colon)? {
            self.skip_type_annotation()?;
        }
        let mut body = self.parse_function_body(&params, Some(&name))?;
        Self::apply_parameter_defaults(&mut body, defaults);
        self.in_generator = prev_gen;
        self.in_async = prev_async;
        self.in_static_block = prev_in_static;
        if is_generator {
            Ok(Statement::AsyncGeneratorDecl { name, params, body })
        } else {
            Ok(Statement::AsyncFunctionDecl { name, params, body })
        }
    }

    pub(crate) fn parse_generator_decl(&mut self) -> Result<Statement, ParseError> {
        let prev_gen = self.in_generator;
        let prev_async = self.in_async;
        let prev_in_static = self.in_static_block;
        self.in_static_block = false;
        self.in_async = false;
        let name = self.expect_identifier()?;
        self.in_generator = true;
        if self.check(&TokenKind::Lt) {
            self.skip_type_parameters()?;
        }
        let (params, defaults) = self.parse_formal_parameters()?;
        if self.match_token(&TokenKind::Colon)? {
            self.skip_type_annotation()?;
        }
        let mut body = self.parse_function_body(&params, Some(&name))?;
        Self::apply_parameter_defaults(&mut body, defaults);
        self.in_generator = prev_gen;
        self.in_async = prev_async;
        self.in_static_block = prev_in_static;
        Ok(Statement::GeneratorDecl { name, params, body })
    }

    pub(crate) fn parse_function_expr(&mut self) -> Result<Expression, ParseError> {
        let prev_in_method = self.in_method;
        let prev_in_class_ctor = self.in_class_constructor;
        let prev_in_derived_ctor = self.in_derived_constructor;
        self.in_method = false;
        self.in_class_constructor = false;
        self.in_derived_constructor = false;
        let prev_in_static = self.in_static_block;
        self.in_static_block = false;
        let is_generator = self.match_token(&TokenKind::Star)?;
        let prev_gen = self.in_generator;
        let prev_async = self.in_async;
        self.in_generator = is_generator;
        self.in_async = false;
        let res = (|| -> Result<Expression, ParseError> {
            let name = if let TokenKind::Ident(id) = &self.current.kind {
                if self.is_strict && (is_strict_reserved_word(id) || id == "eval" || id == "arguments")
                {
                    return Err(ParseError::UnexpectedToken(
                        TokenKind::Ident(id.clone()),
                        self.current.line,
                        self.current.col,
                    ));
                }
                let n = id.clone();
                self.advance()?;
                Some(n)
            } else if !self.is_strict && (!is_generator || !prev_gen) && self.check(&TokenKind::Yield) {
                self.advance()?;
                Some("yield".to_string())
            } else if !self.is_module && !self.in_async && self.check(&TokenKind::Await) {
                self.advance()?;
                Some("await".to_string())
            } else {
                None
            };

            if self.check(&TokenKind::Lt) {
                self.skip_type_parameters()?;
            }

            let (params, defaults) = self.parse_formal_parameters()?;
            if self.match_token(&TokenKind::Colon)? {
                self.skip_type_annotation()?;
            }
            let mut body = self.parse_function_body(&params, name.as_deref())?;
            Self::apply_parameter_defaults(&mut body, defaults);

            if is_generator {
                Ok(Expression::GeneratorExpr { name, params, body })
            } else {
                Ok(Expression::FunctionExpr { name, params, body })
            }
        })();
        self.in_generator = prev_gen;
        self.in_async = prev_async;
        self.in_static_block = prev_in_static;
        self.in_method = prev_in_method;
        self.in_class_constructor = prev_in_class_ctor;
        self.in_derived_constructor = prev_in_derived_ctor;
        res
    }
}
