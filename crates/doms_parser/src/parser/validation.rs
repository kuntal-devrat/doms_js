use super::{ParseError, Parser};
use crate::ast::*;
use crate::TokenKind;

impl<'a> Parser<'a> {
    pub(crate) fn check_sub_statement(&self, stmt: &Statement, allow_sloppy_func: bool) -> Result<(), ParseError> {
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

    pub(crate) fn validate_block_declarations(&self, stmts: &[Statement]) -> Result<(), ParseError> {
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

    pub(crate) fn add_lexical_name(
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

    pub(crate) fn collect_var_declared_names(stmt: &Statement, vars: &mut Vec<String>) {
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

    pub(crate) fn parse_sub_statement(&mut self, allow_sloppy_func: bool) -> Result<Statement, ParseError> {
        let prev = self.in_single_statement_context;
        self.in_single_statement_context = true;
        let res = self.parse_statement();
        self.in_single_statement_context = prev;
        let stmt = res?;
        self.check_sub_statement(&stmt, allow_sloppy_func)?;
        Ok(stmt)
    }

    pub(crate) fn is_iteration_statement(stmt: &Statement) -> bool {
        match stmt {
            Statement::While { .. }
            | Statement::DoWhile { .. }
            | Statement::For { .. }
            | Statement::ForIn { .. }
            | Statement::ForOf { .. } => true,
            Statement::Labeled { body, .. } => Self::is_iteration_statement(body),
            _ => false,
        }
    }

    pub(crate) fn validate_labels_and_jumps(stmts: &[Statement]) -> Result<(), ParseError> {
        let mut active_labels = Vec::new();
        let mut iteration_labels = Vec::new();
        for stmt in stmts {
            Self::check_labels_and_jumps_stmt(stmt, &mut active_labels, &mut iteration_labels, false, false)?;
        }
        Ok(())
    }

    pub(crate) fn validate_expr_functions(expr: &Expression) -> Result<(), ParseError> {
        match expr {
            Expression::FunctionExpr { body, .. }
            | Expression::ArrowExpr { body, .. }
            | Expression::MethodExpr { body, .. }
            | Expression::AsyncFunctionExpr { body, .. }
            | Expression::GeneratorExpr { body, .. }
            | Expression::AsyncGeneratorExpr { body, .. } => {
                Self::validate_labels_and_jumps(body)?;
            }
            Expression::Binary { left, right, .. } => {
                Self::validate_expr_functions(left)?;
                Self::validate_expr_functions(right)?;
            }
            Expression::Assignment { value, .. } => {
                Self::validate_expr_functions(value)?;
            }
            Expression::MemberAssignment { object, value, .. } => {
                Self::validate_expr_functions(object)?;
                Self::validate_expr_functions(value)?;
            }
            Expression::IndexAssignment { object, index, value } => {
                Self::validate_expr_functions(object)?;
                Self::validate_expr_functions(index)?;
                Self::validate_expr_functions(value)?;
            }
            Expression::ClassExpr(class_def) => {
                Self::validate_labels_and_jumps(&class_def.ctor_body)?;
                for el in &class_def.elements {
                    match &el.kind {
                        ClassElementKind::Method(e)
                        | ClassElementKind::Getter(e)
                        | ClassElementKind::Setter(e) => {
                            Self::validate_expr_functions(e)?;
                        }
                        ClassElementKind::Field(opt_e) => {
                            if let Some(e) = opt_e {
                                Self::validate_expr_functions(e)?;
                            }
                        }
                        ClassElementKind::StaticBlock(stmts) => {
                            Self::validate_labels_and_jumps(stmts)?;
                        }
                    }
                }
            }
            Expression::Unary { expr, .. }
            | Expression::Update { expr, .. }
            | Expression::Grouping(expr)
            | Expression::Spread(expr)
            | Expression::Await(expr)
            | Expression::DynamicImport(expr) => {
                Self::validate_expr_functions(expr)?;
            }
            Expression::Yield(opt_expr) => {
                if let Some(e) = opt_expr {
                    Self::validate_expr_functions(e)?;
                }
            }
            Expression::YieldStar(expr) => {
                Self::validate_expr_functions(expr)?;
            }
            Expression::Call { callee, args } | Expression::New { callee, args } => {
                Self::validate_expr_functions(callee)?;
                for arg in args {
                    Self::validate_expr_functions(arg)?;
                }
            }
            Expression::Member { object, .. } => {
                Self::validate_expr_functions(object)?;
            }
            Expression::Index { object, index } => {
                Self::validate_expr_functions(object)?;
                Self::validate_expr_functions(index)?;
            }
            Expression::Conditional { condition, then_expr, else_expr } => {
                Self::validate_expr_functions(condition)?;
                Self::validate_expr_functions(then_expr)?;
                Self::validate_expr_functions(else_expr)?;
            }
            Expression::Sequence(exprs) | Expression::Array(exprs) => {
                for e in exprs {
                    Self::validate_expr_functions(e)?;
                }
            }
            Expression::Object(props) => {
                for p in props {
                    match p {
                        ObjectProperty::KeyValue(key, val)
                        | ObjectProperty::Getter(key, val)
                        | ObjectProperty::Setter(key, val) => {
                            if let PropertyKey::Computed(k_expr) = key {
                                Self::validate_expr_functions(k_expr)?;
                            }
                            Self::validate_expr_functions(val)?;
                        }
                        ObjectProperty::Spread(expr) => {
                            Self::validate_expr_functions(expr)?;
                        }
                    }
                }
            }
            Expression::ObjectRest { target, excluded_keys } => {
                Self::validate_expr_functions(target)?;
                for k in excluded_keys {
                    Self::validate_expr_functions(k)?;
                }
            }
            Expression::DestructuringAssignment { pattern, value } => {
                Self::validate_expr_functions(pattern)?;
                Self::validate_expr_functions(value)?;
            }
            Expression::PrivateIdentifier(_) => {}
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn check_labels_and_jumps_stmt(
        stmt: &Statement,
        active_labels: &mut Vec<String>,
        iteration_labels: &mut Vec<String>,
        in_loop: bool,
        in_switch: bool,
    ) -> Result<(), ParseError> {
        match stmt {
            Statement::Labeled { label, body } => {
                if active_labels.contains(label) {
                    return Err(ParseError::General(format!("Duplicate label '{}'", label)));
                }
                active_labels.push(label.clone());
                let is_iter = Self::is_iteration_statement(body);
                if is_iter {
                    iteration_labels.push(label.clone());
                }
                let res = Self::check_labels_and_jumps_stmt(body, active_labels, iteration_labels, in_loop, in_switch);
                active_labels.pop();
                if is_iter {
                    iteration_labels.pop();
                }
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
                if !iteration_labels.contains(label) {
                    return Err(ParseError::General(format!("Undefined continue label '{}'", label)));
                }
            }
            Statement::Continue(None) => {
                if !in_loop {
                    return Err(ParseError::General("Illegal continue statement".into()));
                }
            }
            Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
                Self::check_labels_and_jumps_stmt(body, active_labels, iteration_labels, true, in_switch)?;
            }
            Statement::For { body, .. }
            | Statement::ForIn { body, .. }
            | Statement::ForOf { body, .. } => {
                Self::check_labels_and_jumps_stmt(body, active_labels, iteration_labels, true, in_switch)?;
            }
            Statement::Block(inner) => {
                for s in inner {
                    Self::check_labels_and_jumps_stmt(s, active_labels, iteration_labels, in_loop, in_switch)?;
                }
            }
            Statement::If { then_branch, else_branch, .. } => {
                Self::check_labels_and_jumps_stmt(then_branch, active_labels, iteration_labels, in_loop, in_switch)?;
                if let Some(eb) = else_branch {
                    Self::check_labels_and_jumps_stmt(eb, active_labels, iteration_labels, in_loop, in_switch)?;
                }
            }
            Statement::Switch { cases, .. } => {
                for c in cases {
                    for s in &c.consequent {
                        Self::check_labels_and_jumps_stmt(s, active_labels, iteration_labels, in_loop, true)?;
                    }
                }
            }
            Statement::TryCatch { try_block, catch_block, finally_block, .. } => {
                Self::check_labels_and_jumps_stmt(try_block, active_labels, iteration_labels, in_loop, in_switch)?;
                if let Some(cb) = catch_block {
                    Self::check_labels_and_jumps_stmt(cb, active_labels, iteration_labels, in_loop, in_switch)?;
                }
                if let Some(fb) = finally_block {
                    Self::check_labels_and_jumps_stmt(fb, active_labels, iteration_labels, in_loop, in_switch)?;
                }
            }
            Statement::With { body, .. } => {
                Self::check_labels_and_jumps_stmt(body, active_labels, iteration_labels, in_loop, in_switch)?;
            }
            Statement::FunctionDecl { body, .. }
            | Statement::AsyncFunctionDecl { body, .. }
            | Statement::GeneratorDecl { body, .. }
            | Statement::AsyncGeneratorDecl { body, .. } => {
                Self::validate_labels_and_jumps(body)?;
            }
            Statement::Expression(expr) => {
                Self::validate_expr_functions(expr)?;
            }
            Statement::Return(Some(expr)) => {
                Self::validate_expr_functions(expr)?;
            }
            Statement::VarDecl { declarations, .. } => {
                for decl in declarations {
                    if let Some(ref init) = decl.init {
                        Self::validate_expr_functions(init)?;
                    }
                }
            }
            Statement::ClassDecl(class_def) => {
                Self::validate_labels_and_jumps(&class_def.ctor_body)?;
                for el in &class_def.elements {
                    match &el.kind {
                        ClassElementKind::Method(e)
                        | ClassElementKind::Getter(e)
                        | ClassElementKind::Setter(e) => {
                            Self::validate_expr_functions(e)?;
                        }
                        ClassElementKind::Field(opt_e) => {
                            if let Some(e) = opt_e {
                                Self::validate_expr_functions(e)?;
                            }
                        }
                        ClassElementKind::StaticBlock(stmts) => {
                            Self::validate_labels_and_jumps(stmts)?;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn validate_assignment_pattern(&self, expr: &Expression) -> Result<(), ParseError> {
        if self.trailing_comma_after_rest {
            return Err(ParseError::General("Rest element cannot have a trailing comma in assignment pattern".into()));
        }
        self.validate_assignment_pattern_inner(expr)
    }

    fn validate_assignment_pattern_inner(&self, expr: &Expression) -> Result<(), ParseError> {
        match expr {
            Expression::Identifier(name) => {
                if self.is_strict && (name == "eval" || name == "arguments") {
                    return Err(ParseError::UnexpectedToken(TokenKind::Ident(name.clone()), self.current.line, self.current.col));
                }
                Ok(())
            }
            Expression::Member { .. } | Expression::Index { .. } => Ok(()),
            Expression::Grouping(inner) => self.validate_assignment_pattern_inner(inner),
            Expression::Array(elements) => {
                let mut has_rest = false;
                for elem in elements {
                    if has_rest {
                        return Err(ParseError::General("Elements cannot follow rest element".into()));
                    }
                    match elem {
                        Expression::Literal(Literal::Hole) => {}
                        Expression::Spread(inner) => {
                            has_rest = true;
                            if matches!(&**inner, Expression::Assignment { .. } | Expression::MemberAssignment { .. } | Expression::IndexAssignment { .. } | Expression::DestructuringAssignment { .. }) {
                                return Err(ParseError::General("Rest element cannot have an initializer".into()));
                            }
                            self.validate_assignment_pattern_inner(inner)?;
                        }
                        Expression::Assignment { target, .. } => {
                            if self.is_strict && (target == "eval" || target == "arguments") {
                                return Err(ParseError::UnexpectedToken(TokenKind::Ident(target.clone()), self.current.line, self.current.col));
                            }
                        }
                        Expression::MemberAssignment { .. } | Expression::IndexAssignment { .. } => {}
                        Expression::DestructuringAssignment { pattern, .. } => {
                            self.validate_assignment_pattern_inner(pattern)?;
                        }
                        other => {
                            self.validate_assignment_pattern_inner(other)?;
                        }
                    }
                }
                Ok(())
            }
            Expression::Object(props) => {
                let mut has_rest = false;
                for prop in props {
                    if has_rest {
                        return Err(ParseError::General("Elements cannot follow rest element".into()));
                    }
                    match prop {
                        ObjectProperty::KeyValue(_key, val) => {
                            if matches!(val, Expression::MethodExpr { .. }) {
                                return Err(ParseError::General("Object pattern cannot contain methods".into()));
                            }
                            if let Expression::Assignment { target, .. } = val {
                                if self.is_strict && (target == "eval" || target == "arguments") {
                                    return Err(ParseError::UnexpectedToken(TokenKind::Ident(target.clone()), self.current.line, self.current.col));
                                }
                            } else if matches!(val, Expression::MemberAssignment { .. } | Expression::IndexAssignment { .. }) {
                                // Default initializer allowed in object pattern
                            } else if let Expression::DestructuringAssignment { pattern, .. } = val {
                                self.validate_assignment_pattern_inner(pattern)?;
                            } else {
                                self.validate_assignment_pattern_inner(val)?;
                            }
                        }
                        ObjectProperty::Getter(..) | ObjectProperty::Setter(..) => {
                            return Err(ParseError::General("Object pattern cannot contain getters or setters".into()));
                        }
                        ObjectProperty::Spread(expr) => {
                            has_rest = true;
                            self.validate_assignment_pattern_inner(expr)?;
                        }
                    }
                }
                Ok(())
            }
            _ => Err(ParseError::InvalidAssignmentTarget(self.current.line, self.current.col)),
        }
    }
}
