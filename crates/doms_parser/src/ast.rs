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
    DoWhile { body: Box<Statement>, condition: Expression },
    ForIn { left: String, right: Expression, body: Box<Statement> },
    ForOf { left: String, right: Expression, body: Box<Statement> },
    Labeled { label: String, body: Box<Statement> },
    Switch { discriminant: Expression, cases: Vec<SwitchCase> },
    Break(Option<String>),
    Continue(Option<String>),
    TryCatch { try_block: Box<Statement>, catch_param: Option<String>, catch_block: Option<Box<Statement>>, finally_block: Option<Box<Statement>> },
    Throw(Expression),
    Return(Option<Expression>),
    FunctionDecl { name: String, params: Vec<String>, body: Vec<Statement> },
    AsyncFunctionDecl { name: String, params: Vec<String>, body: Vec<Statement> },
    GeneratorDecl { name: String, params: Vec<String>, body: Vec<Statement> },
    Import { specifiers: Vec<(String, Option<String>)>, source: String, is_default: bool, is_all: bool },
    Export { decl: Option<Box<Statement>>, is_default: bool },
    Empty,
    Debugger,
    With { object: Expression, body: Box<Statement> },
    ClassDecl(ClassDefinition),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Identifier(String),
    This,
    Super,
    ClassExpr(Box<ClassDefinition>),
    Binary { op: BinaryOp, left: Box<Expression>, right: Box<Expression> },
    Unary { op: UnaryOp, expr: Box<Expression> },
    Assignment { target: String, value: Box<Expression> },
    MemberAssignment { object: Box<Expression>, property: String, value: Box<Expression> },
    IndexAssignment { object: Box<Expression>, index: Box<Expression>, value: Box<Expression> },
    Call { callee: Box<Expression>, args: Vec<Expression> },
    New { callee: Box<Expression>, args: Vec<Expression> },
    Member { object: Box<Expression>, property: String },
    Index { object: Box<Expression>, index: Box<Expression> },
    Object(Vec<ObjectProperty>),
    Array(Vec<Expression>),
    Conditional { condition: Box<Expression>, then_expr: Box<Expression>, else_expr: Box<Expression> },
    FunctionExpr { name: Option<String>, params: Vec<String>, body: Vec<Statement> },
    GeneratorExpr { name: Option<String>, params: Vec<String>, body: Vec<Statement> },
    Sequence(Vec<Expression>),
    Await(Box<Expression>),
    Yield(Option<Box<Expression>>),
    DynamicImport(Box<Expression>),
    Update { op: UpdateOp, is_prefix: bool, expr: Box<Expression> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOp {
    Inc,
    Dec,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Number(f64),
    String { value: String, has_escape: bool },
    Boolean(bool),
    Null,
    Undefined,
    RegExp { pattern: String, flags: String },
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
    Shl,
    Shr,
    Ushr,
    Exp,
    NullishCoalescing,
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

#[derive(Debug, Clone, PartialEq)]
pub enum PropertyKey {
    Literal(String),
    Computed(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectProperty {
    KeyValue(PropertyKey, Expression),
    Getter(PropertyKey, Expression),
    Setter(PropertyKey, Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassElement {
    pub is_static: bool,
    pub key: PropertyKey,
    pub kind: ClassElementKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassElementKind {
    Method(Expression),
    Getter(Expression),
    Setter(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDefinition {
    pub name: Option<String>,
    pub super_class: Option<Box<Expression>>,
    pub ctor_params: Vec<String>,
    pub ctor_body: Vec<Statement>,
    pub elements: Vec<ClassElement>,
}
