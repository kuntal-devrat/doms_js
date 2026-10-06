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
pub struct VarDeclarator {
    pub name: String,
    pub init: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub test: Option<Expression>,
    pub consequent: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Expression(Expression),
    VarDecl { kind: VarKind, declarations: Vec<VarDeclarator> },
    Block(Vec<Statement>),
    If { condition: Expression, then_branch: Box<Statement>, else_branch: Option<Box<Statement>> },
    While { condition: Expression, body: Box<Statement> },
    For { init: Option<Box<Statement>>, condition: Option<Expression>, update: Option<Expression>, body: Box<Statement> },
    Switch { discriminant: Expression, cases: Vec<SwitchCase> },
    Break,
    Continue,
    TryCatch { try_block: Box<Statement>, catch_param: Option<String>, catch_block: Option<Box<Statement>>, finally_block: Option<Box<Statement>> },
    Throw(Expression),
    Return(Option<Expression>),
    FunctionDecl { name: String, params: Vec<String>, body: Vec<Statement> },
    AsyncFunctionDecl { name: String, params: Vec<String>, body: Vec<Statement> },
    GeneratorDecl { name: String, params: Vec<String>, body: Vec<Statement> },
    Import { specifiers: Vec<(String, Option<String>)>, source: String, is_default: bool, is_all: bool },
    Export { decl: Option<Box<Statement>>, is_default: bool },
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Identifier(String),
    This,
    Binary { op: BinaryOp, left: Box<Expression>, right: Box<Expression> },
    Unary { op: UnaryOp, expr: Box<Expression> },
    Assignment { target: String, value: Box<Expression> },
    MemberAssignment { object: Box<Expression>, property: String, value: Box<Expression> },
    IndexAssignment { object: Box<Expression>, index: Box<Expression>, value: Box<Expression> },
    Call { callee: Box<Expression>, args: Vec<Expression> },
    New { callee: Box<Expression>, args: Vec<Expression> },
    Member { object: Box<Expression>, property: String },
    Index { object: Box<Expression>, index: Box<Expression> },
    Object(Vec<(String, Expression)>),
    Array(Vec<Expression>),
    Conditional { condition: Box<Expression>, then_expr: Box<Expression>, else_expr: Box<Expression> },
    FunctionExpr { name: Option<String>, params: Vec<String>, body: Vec<Statement> },
    Await(Box<Expression>),
    Yield(Option<Box<Expression>>),
    DynamicImport(Box<Expression>),
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
    Instanceof,
    In,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Pos,
    Neg,
    Not,
    BitNot,
    Typeof,
    Void,
    Delete,
}
