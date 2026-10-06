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
            TokenKind::Function => self.parse_function_decl(),
            TokenKind::If => self.parse_if_statement(),
            TokenKind::While => self.parse_while_statement(),
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

    fn parse_function_decl(&mut self) -> Result<Statement, ParseError> {
        self.advance()?; // function
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

        Ok(Statement::FunctionDecl { name, params, body })
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
                let prop = match prop_tok.kind {
                    TokenKind::Ident(id) => id,
                    other => {
                        return Err(ParseError::UnexpectedToken(
                            other,
                            prop_tok.line,
                            prop_tok.col,
                        ));
                    }
                };
                lhs = Expression::Member { object: Box::new(lhs), property: prop };
                continue;
            }

            if self.check(&TokenKind::Eq) {
                // Assignment
                if min_bp > 1 {
                    break;
                }
                let target = match lhs {
                    Expression::Identifier(name) => name,
                    _ => {
                        return Err(ParseError::InvalidAssignmentTarget(
                            self.current.line,
                            self.current.col,
                        ));
                    }
                };
                self.advance()?; // =
                let value = self.parse_expression(1)?;
                lhs = Expression::Assignment { target, value: Box::new(value) };
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
            other => Err(ParseError::UnexpectedToken(other, tok.line, tok.col)),
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
}
