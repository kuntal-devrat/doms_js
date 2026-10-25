//! # DOMS Parser
//!
//! Lexer, AST definitions, and Pratt parser for JavaScript in the DOMS engine.

#![allow(clippy::all)]

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use lexer::{LexError, Lexer, Token, TokenKind};
pub use parser::{ParseError, Parser};

/// Parse a JavaScript source code string into an AST [`Program`].
pub fn parse(source: &str) -> Result<Program, ParseError> {
    let mut parser = Parser::new(source)?;
    parser.parse_program()
}

/// Parse a JavaScript module source code string into an AST [`Program`].
pub fn parse_module(source: &str) -> Result<Program, ParseError> {
    let mut parser = Parser::new_module(source)?;
    parser.parse_program()
}

/// Parse a JavaScript source code string into an AST [`Program`] in strict mode.
pub fn parse_strict(source: &str) -> Result<Program, ParseError> {
    let mut parser = Parser::new_strict(source)?;
    parser.parse_program()
}

/// Parse an eval JavaScript source code string into an AST [`Program`].
pub fn parse_eval(
    source: &str,
    is_strict: bool,
    in_method: bool,
    in_function: bool,
    private_names: Option<&std::collections::HashMap<String, String>>,
) -> Result<Program, ParseError> {
    let mut parser = Parser::new_eval(source, is_strict, in_method, in_function, private_names)?;
    parser.parse_program()
}

pub fn contains_arguments_program(prog: &Program) -> bool {
    contains_arguments_stmts(&prog.body)
}

pub fn contains_arguments_stmts(stmts: &[Statement]) -> bool {
    stmts.iter().any(contains_arguments_stmt)
}

pub fn contains_arguments_stmt(stmt: &Statement) -> bool {
    match stmt {
        Statement::Expression(expr) => contains_arguments_expr(expr),
        Statement::VarDecl { declarations, .. } => {
            declarations.iter().any(|d| d.init.as_ref().map_or(false, contains_arguments_expr))
        }
        Statement::Block(stmts) => contains_arguments_stmts(stmts),
        Statement::If { condition, then_branch, else_branch } => {
            contains_arguments_expr(condition)
                || contains_arguments_stmt(then_branch)
                || else_branch.as_ref().map_or(false, |b| contains_arguments_stmt(b))
        }
        Statement::While { condition, body } | Statement::DoWhile { condition, body } => {
            contains_arguments_expr(condition) || contains_arguments_stmt(body)
        }
        Statement::For { init, condition, update, body } => {
            init.as_ref().map_or(false, |s| contains_arguments_stmt(s))
                || condition.as_ref().map_or(false, contains_arguments_expr)
                || update.as_ref().map_or(false, contains_arguments_expr)
                || contains_arguments_stmt(body)
        }
        Statement::ForIn { right, body, .. } | Statement::ForOf { right, body, .. } => {
            contains_arguments_expr(right) || contains_arguments_stmt(body)
        }
        Statement::Labeled { body, .. } => contains_arguments_stmt(body),
        Statement::Switch { discriminant, cases } => {
            contains_arguments_expr(discriminant)
                || cases.iter().any(|c| {
                    c.test.as_ref().map_or(false, contains_arguments_expr)
                        || contains_arguments_stmts(&c.consequent)
                })
        }
        Statement::TryCatch { try_block, catch_block, finally_block, .. } => {
            contains_arguments_stmt(try_block)
                || catch_block.as_ref().map_or(false, |b| contains_arguments_stmt(b))
                || finally_block.as_ref().map_or(false, |b| contains_arguments_stmt(b))
        }
        Statement::Throw(expr) => contains_arguments_expr(expr),
        Statement::Return(opt_expr) => opt_expr.as_ref().map_or(false, contains_arguments_expr),
        Statement::With { object, body } => {
            contains_arguments_expr(object) || contains_arguments_stmt(body)
        }
        Statement::FunctionDecl { .. }
        | Statement::AsyncFunctionDecl { .. }
        | Statement::GeneratorDecl { .. }
        | Statement::AsyncGeneratorDecl { .. } => false,
        Statement::ClassDecl(c) => c.super_class.as_ref().map_or(false, |s| contains_arguments_expr(s)),
        Statement::Export { decl: Some(d), .. } => contains_arguments_stmt(d),
        Statement::Break(_)
        | Statement::Continue(_)
        | Statement::Empty
        | Statement::Debugger
        | Statement::Import { .. }
        | Statement::Export { .. } => false,
    }
}

pub fn contains_arguments_expr(expr: &Expression) -> bool {
    match expr {
        Expression::Identifier(name) => name == "arguments",
        Expression::Literal(_) | Expression::This | Expression::Super => false,
        Expression::Binary { left, right, .. }
        | Expression::MemberAssignment { object: left, value: right, .. } => {
            contains_arguments_expr(left) || contains_arguments_expr(right)
        }
        Expression::Assignment { value, .. } => contains_arguments_expr(value),
        Expression::IndexAssignment { object, index, value } => {
            contains_arguments_expr(object)
                || contains_arguments_expr(index)
                || contains_arguments_expr(value)
        }
        Expression::Unary { expr, .. }
        | Expression::Update { expr, .. }
        | Expression::Spread(expr)
        | Expression::Grouping(expr)
        | Expression::YieldStar(expr)
        | Expression::DynamicImport(expr)
        | Expression::Await(expr) => contains_arguments_expr(expr),
        Expression::Yield(opt) => {
            opt.as_ref().map_or(false, |v| contains_arguments_expr(v))
        }
        Expression::Member { object, .. } => contains_arguments_expr(object),
        Expression::Index { object, index } => {
            contains_arguments_expr(object) || contains_arguments_expr(index)
        }
        Expression::Call { callee, args } | Expression::New { callee, args } => {
            contains_arguments_expr(callee) || args.iter().any(contains_arguments_expr)
        }
        Expression::Array(elements) => elements.iter().any(contains_arguments_expr),
        Expression::Object(props) => props.iter().any(|p| match p {
            ObjectProperty::KeyValue(key, val) => {
                let k_has = match key {
                    PropertyKey::Computed(e) => contains_arguments_expr(e),
                    _ => false,
                };
                k_has || contains_arguments_expr(val)
            }
            ObjectProperty::Spread(e) => contains_arguments_expr(e),
            ObjectProperty::Getter(key, _) | ObjectProperty::Setter(key, _) => match key {
                PropertyKey::Computed(e) => contains_arguments_expr(e),
                _ => false,
            },
            ObjectProperty::CoverInitializedName(..) => false,
        }),
        Expression::Sequence(exprs) => exprs.iter().any(contains_arguments_expr),
        Expression::Conditional { condition, then_expr, else_expr } => {
            contains_arguments_expr(condition)
                || contains_arguments_expr(then_expr)
                || contains_arguments_expr(else_expr)
        }
        Expression::ArrowExpr { body, .. } | Expression::AsyncArrowExpr { body, .. } => {
            contains_arguments_stmts(body)
        }
        Expression::FunctionExpr { .. }
        | Expression::GeneratorExpr { .. }
        | Expression::AsyncFunctionExpr { .. }
        | Expression::AsyncGeneratorExpr { .. }
        | Expression::MethodExpr { .. } => false,
        Expression::ClassExpr(c) => c.super_class.as_ref().map_or(false, |s| contains_arguments_expr(s)),
        Expression::TaggedTemplate { tag, exprs, .. } => {
            contains_arguments_expr(tag) || exprs.iter().any(contains_arguments_expr)
        }
        Expression::OptionalChain(chain) => contains_arguments_expr(&chain.base),
        Expression::DefineField { target, value, .. }
        | Expression::DefineDataProperty { target, value, .. } => {
            contains_arguments_expr(target) || contains_arguments_expr(value)
        }
        Expression::DefineComputedDataProperty { target, key, value } => {
            contains_arguments_expr(target)
                || contains_arguments_expr(key)
                || contains_arguments_expr(value)
        }
        Expression::DefinePrivateMethod { target, value, .. } => {
            contains_arguments_expr(target) || contains_arguments_expr(value)
        }
        Expression::DefinePrivateAccessor { target, accessor, .. } => {
            contains_arguments_expr(target) || contains_arguments_expr(accessor)
        }
        Expression::DestructuringAssignment { pattern, value } => {
            contains_arguments_expr(pattern) || contains_arguments_expr(value)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_script() {
        let code = "let x = 10; let y = 20; let z = x + y;";
        let prog = parse(code).unwrap();
        assert_eq!(prog.body.len(), 3);
    }

    #[test]
    fn test_yield_star_ast() {
        let code = "class A {\n  *g() {\n    yield\n    * 1\n  }\n}";
        let res = parse(code);
        assert!(res.is_err());
    }
}
