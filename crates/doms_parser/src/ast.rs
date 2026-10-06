//! Abstract Syntax Tree (AST) definitions for DOMS JavaScript parser.

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarKind {
    Var,
    Let,
    Const,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Expression(Expression),
    VarDecl { kind: VarKind, name: String, init: Option<Expression> },
    Block(Vec<Statement>),
    If { condition: Expression, then_branch: Box<Statement>, else_branch: Option<Box<Statement>> },
    While { condition: Expression, body: Box<Statement> },
    Return(Option<Expression>),
    FunctionDecl { name: String, params: Vec<String>, body: Vec<Statement> },
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Identifier(String),
    Binary { op: BinaryOp, left: Box<Expression>, right: Box<Expression> },
    Unary { op: UnaryOp, expr: Box<Expression> },
    Assignment { target: String, value: Box<Expression> },
    Call { callee: Box<Expression>, args: Vec<Expression> },
    Member { object: Box<Expression>, property: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    StrictEq,
    StrictNotEq,
    Lt,
    Lte,
    Gt,
    Gte,
    LogicalAnd,
    LogicalOr,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Typeof,
}
