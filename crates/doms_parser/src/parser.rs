//! Pratt parser for JavaScript syntax.

use crate::ast::*;
use crate::lexer::{LexError, Lexer, Token, TokenKind};
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
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Token,
    peeked: Token,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token()?;
        let peeked = lexer.next_token()?;
        Ok(Self { lexer, current, peeked })
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

    fn match_token(&mut self, kind: &TokenKind) -> Result<bool, ParseError> {
        if self.check(kind) {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
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

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut body = Vec::new();
        while self.current.kind != TokenKind::Eof {
            body.push(self.parse_statement()?);
        }
        Ok(Program { body })
    }

    pub fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.current.kind {
            TokenKind::Let => self.parse_var_decl(VarKind::Let),
            TokenKind::Const => self.parse_var_decl(VarKind::Const),
            TokenKind::Var => self.parse_var_decl(VarKind::Var),
            TokenKind::Async => {
                self.advance()?;
                if self.check(&TokenKind::Function) {
                    self.parse_async_function_decl()
                } else {
                    let expr = self.parse_expression(0)?;
                    self.match_token(&TokenKind::Semicolon)?;
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
            TokenKind::Import => self.parse_import_statement(),
            TokenKind::Export => self.parse_export_statement(),
            TokenKind::If => self.parse_if_statement(),
            TokenKind::While => self.parse_while_statement(),
            TokenKind::For => self.parse_for_statement(),
            TokenKind::Break => {
                self.advance()?;
                self.match_token(&TokenKind::Semicolon)?;
                Ok(Statement::Break)
            }
            TokenKind::Continue => {
                self.advance()?;
                self.match_token(&TokenKind::Semicolon)?;
                Ok(Statement::Continue)
            }
            TokenKind::Try => self.parse_try_statement(),
            TokenKind::Throw => {
                self.advance()?;
                let expr = self.parse_expression(0)?;
                self.match_token(&TokenKind::Semicolon)?;
                Ok(Statement::Throw(expr))
            }
            TokenKind::Return => self.parse_return_statement(),
            TokenKind::LBrace => self.parse_block_statement(),
            TokenKind::Semicolon => {
                self.advance()?;
                Ok(Statement::Empty)
            }
            _ => {
                let expr = self.parse_expression(0)?;
                self.match_token(&TokenKind::Semicolon)?;
                Ok(Statement::Expression(expr))
            }
        }
    }

    fn parse_var_decl(&mut self, kind: VarKind) -> Result<Statement, ParseError> {
        self.advance()?; // let / const / var
        let name_tok = self.advance()?;
        let name = match name_tok.kind {
            TokenKind::Ident(id) => id,
            other => return Err(ParseError::UnexpectedToken(other, name_tok.line, name_tok.col)),
        };

        let init =
            if self.match_token(&TokenKind::Eq)? { Some(self.parse_expression(0)?) } else { None };

        self.match_token(&TokenKind::Semicolon)?;
        Ok(Statement::VarDecl { kind, name, init })
    }

    fn parse_function_signature_and_body(
        &mut self,
    ) -> Result<(String, Vec<String>, Vec<Statement>), ParseError> {
        let name_tok = self.advance()?;
        let name = match name_tok.kind {
            TokenKind::Ident(id) => id,
            other => return Err(ParseError::UnexpectedToken(other, name_tok.line, name_tok.col)),
        };

        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let param_tok = self.advance()?;
                match param_tok.kind {
                    TokenKind::Ident(id) => params.push(id),
                    other => {
                        return Err(ParseError::UnexpectedToken(
                            other,
                            param_tok.line,
                            param_tok.col,
                        ));
                    }
                }
                if !self.match_token(&TokenKind::Comma)? {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen)?;

        self.expect(TokenKind::LBrace)?;
        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            body.push(self.parse_statement()?);
        }
        self.expect(TokenKind::RBrace)?;

        Ok((name, params, body))
    }

    fn parse_function_decl_body(&mut self) -> Result<Statement, ParseError> {
        let (name, params, body) = self.parse_function_signature_and_body()?;
        Ok(Statement::FunctionDecl { name, params, body })
    }

    fn parse_async_function_decl(&mut self) -> Result<Statement, ParseError> {
        self.expect(TokenKind::Function)?;
        let (name, params, body) = self.parse_function_signature_and_body()?;
        Ok(Statement::AsyncFunctionDecl { name, params, body })
    }

    fn parse_generator_decl(&mut self) -> Result<Statement, ParseError> {
        let (name, params, body) = self.parse_function_signature_and_body()?;
        Ok(Statement::GeneratorDecl { name, params, body })
    }

    fn parse_import_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // import
        let mut specifiers = Vec::new();
        let mut is_default = false;
        let mut is_all = false;

        if let TokenKind::Ident(default_name) = &self.current.kind {
            let name = default_name.clone();
            self.advance()?;
            specifiers.push((name, None));
            is_default = true;
            if self.match_token(&TokenKind::Comma)? {
                // optional comma after default
            }
        }

        if self.match_token(&TokenKind::Star)? {
            self.expect(TokenKind::As)?;
            let ns_tok = self.advance()?;
            if let TokenKind::Ident(ns) = ns_tok.kind {
                specifiers.push(("*".to_string(), Some(ns)));
                is_all = true;
            }
        } else if self.match_token(&TokenKind::LBrace)? {
            while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
                let name_tok = self.advance()?;
                let name = match name_tok.kind {
                    TokenKind::Ident(id) => id,
                    other => return Err(ParseError::UnexpectedToken(other, name_tok.line, name_tok.col)),
                };
                let alias = if self.match_token(&TokenKind::As)? {
                    let alias_tok = self.advance()?;
                    match alias_tok.kind {
                        TokenKind::Ident(id) => Some(id),
                        other => return Err(ParseError::UnexpectedToken(other, alias_tok.line, alias_tok.col)),
                    }
                } else {
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
            TokenKind::StringLit(s) => s,
            other => return Err(ParseError::UnexpectedToken(other, src_tok.line, src_tok.col)),
        };
        self.match_token(&TokenKind::Semicolon)?;
        Ok(Statement::Import { specifiers, source, is_default, is_all })
    }

    fn parse_export_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // export
        if self.match_token(&TokenKind::Default)? {
            let expr = self.parse_expression(0)?;
            self.match_token(&TokenKind::Semicolon)?;
            Ok(Statement::Export {
                decl: Some(Box::new(Statement::Expression(expr))),
                is_default: true,
            })
        } else {
            let stmt = self.parse_statement()?;
            Ok(Statement::Export {
                decl: Some(Box::new(stmt)),
                is_default: false,
            })
        }
    }

    fn parse_if_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // if
        self.expect(TokenKind::LParen)?;
        let condition = self.parse_expression(0)?;
        self.expect(TokenKind::RParen)?;

        let then_branch = Box::new(self.parse_statement()?);
        let else_branch = if self.match_token(&TokenKind::Else)? {
            Some(Box::new(self.parse_statement()?))
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
        Ok(Statement::While { condition, body })
    }

    fn parse_for_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // for
        self.expect(TokenKind::LParen)?;
        let init = if self.match_token(&TokenKind::Semicolon)? {
            None
        } else if self.check(&TokenKind::Let) || self.check(&TokenKind::Const) || self.check(&TokenKind::Var) {
            let kind = match self.current.kind {
                TokenKind::Let => VarKind::Let,
                TokenKind::Const => VarKind::Const,
                _ => VarKind::Var,
            };
            Some(Box::new(self.parse_var_decl(kind)?))
        } else {
            let expr = self.parse_expression(0)?;
            self.expect(TokenKind::Semicolon)?;
            Some(Box::new(Statement::Expression(expr)))
        };

        let condition = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression(0)?)
        };
        self.expect(TokenKind::Semicolon)?;

        let update = if self.check(&TokenKind::RParen) {
            None
        } else {
            Some(self.parse_expression(0)?)
        };
        self.expect(TokenKind::RParen)?;

        let body = Box::new(self.parse_statement()?);
        Ok(Statement::For { init, condition, update, body })
    }

    fn parse_try_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // try
        let try_block = Box::new(self.parse_block_statement()?);
        let mut catch_param = None;
        let mut catch_block = None;
        if self.match_token(&TokenKind::Catch)? {
            if self.match_token(&TokenKind::LParen)? {
                let p_tok = self.advance()?;
                match p_tok.kind {
                    TokenKind::Ident(id) => catch_param = Some(id),
                    other => return Err(ParseError::UnexpectedToken(other, p_tok.line, p_tok.col)),
                }
                self.expect(TokenKind::RParen)?;
            }
            catch_block = Some(Box::new(self.parse_block_statement()?));
        }
        let mut finally_block = None;
        if self.match_token(&TokenKind::Finally)? {
            finally_block = Some(Box::new(self.parse_block_statement()?));
        }
        Ok(Statement::TryCatch { try_block, catch_param, catch_block, finally_block })
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // return
        if self.check(&TokenKind::Semicolon)
            || self.check(&TokenKind::RBrace)
            || self.check(&TokenKind::Eof)
        {
            self.match_token(&TokenKind::Semicolon)?;
            Ok(Statement::Return(None))
        } else {
            let expr = self.parse_expression(0)?;
            self.match_token(&TokenKind::Semicolon)?;
            Ok(Statement::Return(Some(expr)))
        }
    }

    fn parse_block_statement(&mut self) -> Result<Statement, ParseError> {
        self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            stmts.push(self.parse_statement()?);
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Statement::Block(stmts))
    }

    pub fn parse_expression(&mut self, min_bp: u8) -> Result<Expression, ParseError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            // Postfix / Infix operations
            if self.check(&TokenKind::LParen) {
                // Function call
                self.advance()?;
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expression(0)?);
                        if !self.match_token(&TokenKind::Comma)? {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RParen)?;
                lhs = Expression::Call { callee: Box::new(lhs), args };
                continue;
            }

            if self.check(&TokenKind::Dot) {
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
                lhs = Expression::Member { object: Box::new(lhs), property: prop };
                continue;
            }

            if self.check(&TokenKind::LBracket) {
                // Index access: lhs[expr]
                self.advance()?;
                let index = self.parse_expression(0)?;
                self.expect(TokenKind::RBracket)?;
                lhs = Expression::Index { object: Box::new(lhs), index: Box::new(index) };
                continue;
            }

            if self.check(&TokenKind::Question) {
                if min_bp > 1 {
                    break;
                }
                self.advance()?; // ?
                let then_expr = self.parse_expression(0)?;
                self.expect(TokenKind::Colon)?;
                let else_expr = self.parse_expression(0)?;
                lhs = Expression::Conditional {
                    condition: Box::new(lhs),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                };
                continue;
            }

            if self.check(&TokenKind::Eq) {
                // Assignment
                if min_bp > 1 {
                    break;
                }
                self.advance()?; // =
                let value = self.parse_expression(1)?;
                match lhs {
                    Expression::Identifier(name) => {
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

            let (op, l_bp, r_bp) = match self.current.kind {
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
                TokenKind::Plus => (BinaryOp::Add, 16, 17),
                TokenKind::Minus => (BinaryOp::Sub, 16, 17),
                TokenKind::Star => (BinaryOp::Mul, 18, 19),
                TokenKind::Slash => (BinaryOp::Div, 18, 19),
                TokenKind::Percent => (BinaryOp::Mod, 18, 19),
                _ => break,
            };

            if l_bp < min_bp {
                break;
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
            TokenKind::StringLit(s) => Ok(Expression::Literal(Literal::String(s))),
            TokenKind::True => Ok(Expression::Literal(Literal::Boolean(true))),
            TokenKind::False => Ok(Expression::Literal(Literal::Boolean(false))),
            TokenKind::Null => Ok(Expression::Literal(Literal::Null)),
            TokenKind::Undefined => Ok(Expression::Literal(Literal::Undefined)),
            TokenKind::Ident(s) => Ok(Expression::Identifier(s)),
            TokenKind::This => Ok(Expression::This),
            TokenKind::LBracket => self.parse_array_literal(),
            TokenKind::LBrace => self.parse_object_literal(),
            TokenKind::Function => self.parse_function_expr(),
            TokenKind::Minus => {
                let expr = self.parse_expression(20)?;
                Ok(Expression::Unary { op: UnaryOp::Neg, expr: Box::new(expr) })
            }
            TokenKind::Bang => {
                let expr = self.parse_expression(20)?;
                Ok(Expression::Unary { op: UnaryOp::Not, expr: Box::new(expr) })
            }
            TokenKind::Tilde => {
                let expr = self.parse_expression(20)?;
                Ok(Expression::Unary { op: UnaryOp::BitNot, expr: Box::new(expr) })
            }
            TokenKind::Typeof => {
                let expr = self.parse_expression(20)?;
                Ok(Expression::Unary { op: UnaryOp::Typeof, expr: Box::new(expr) })
            }
            TokenKind::LParen => {
                let expr = self.parse_expression(0)?;
                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }
            TokenKind::New => {
                let mut callee = self.parse_prefix()?;
                while self.check(&TokenKind::Dot) {
                    self.advance()?;
                    let prop_tok = self.advance()?;
                    let prop = match prop_tok.kind.as_prop_name() {
                        Some(id) => id,
                        None => return Err(ParseError::UnexpectedToken(prop_tok.kind, prop_tok.line, prop_tok.col)),
                    };
                    callee = Expression::Member { object: Box::new(callee), property: prop };
                }
                let mut args = Vec::new();
                if self.match_token(&TokenKind::LParen)? {
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expression(0)?);
                            if !self.match_token(&TokenKind::Comma)? {
                                break;
                            }
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                }
                Ok(Expression::New { callee: Box::new(callee), args })
            }
            TokenKind::Await => {
                let expr = self.parse_expression(20)?;
                Ok(Expression::Await(Box::new(expr)))
            }
            TokenKind::Yield => {
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
            other => Err(ParseError::UnexpectedToken(other, tok.line, tok.col)),
        }
    }

    fn parse_object_literal(&mut self) -> Result<Expression, ParseError> {
        let mut props = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            let key_tok = self.advance()?;
            let key = match key_tok.kind {
                TokenKind::StringLit(s) => s,
                other => match other.as_prop_name() {
                    Some(id) => id,
                    None => return Err(ParseError::UnexpectedToken(other, key_tok.line, key_tok.col)),
                },
            };
            self.expect(TokenKind::Colon)?;
            let val = self.parse_expression(0)?;
            props.push((key, val));
            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Expression::Object(props))
    }

    fn parse_array_literal(&mut self) -> Result<Expression, ParseError> {
        let mut elements = Vec::new();
        while !self.check(&TokenKind::RBracket) && self.current.kind != TokenKind::Eof {
            elements.push(self.parse_expression(0)?);
            if !self.match_token(&TokenKind::Comma)? {
                break;
            }
        }
        self.expect(TokenKind::RBracket)?;
        Ok(Expression::Array(elements))
    }

    fn parse_function_expr(&mut self) -> Result<Expression, ParseError> {
        let name = if let TokenKind::Ident(id) = &self.current.kind {
            let n = id.clone();
            self.advance()?;
            Some(n)
        } else {
            None
        };

        self.expect(TokenKind::LParen)?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let param_tok = self.advance()?;
                match param_tok.kind {
                    TokenKind::Ident(id) => params.push(id),
                    other => return Err(ParseError::UnexpectedToken(other, param_tok.line, param_tok.col)),
                }
                if !self.match_token(&TokenKind::Comma)? {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen)?;

        self.expect(TokenKind::LBrace)?;
        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && self.current.kind != TokenKind::Eof {
            body.push(self.parse_statement()?);
        }
        self.expect(TokenKind::RBrace)?;

        Ok(Expression::FunctionExpr { name, params, body })
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
        let mut parser = Parser::new(code).unwrap();
        let prog = parser.parse_program().unwrap();
        assert_eq!(prog.body.len(), 4);
    }
}
