//! Abstract Syntax Tree (AST) definitions for DOMS JavaScript parser.

use num_bigint::BigInt;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    VarDecl {
        kind: VarKind,
        declarations: Vec<VarDeclarator>,
    },
    Block(Vec<Statement>),
    If {
        condition: Expression,
        then_branch: Box<Statement>,
        else_branch: Option<Box<Statement>>,
    },
    While {
        condition: Expression,
        body: Box<Statement>,
    },
    For {
        init: Option<Box<Statement>>,
        condition: Option<Expression>,
        update: Option<Expression>,
        body: Box<Statement>,
    },
    DoWhile {
        body: Box<Statement>,
        condition: Expression,
    },
    ForIn {
        var_kind: Option<VarKind>,
        left: String,
        right: Expression,
        body: Box<Statement>,
    },
    ForOf {
        var_kind: Option<VarKind>,
        left: String,
        right: Expression,
        body: Box<Statement>,
        is_await: bool,
    },
    Labeled {
        label: String,
        body: Box<Statement>,
    },
    Switch {
        discriminant: Expression,
        cases: Vec<SwitchCase>,
    },
    Break(Option<String>),
    Continue(Option<String>),
    TryCatch {
        try_block: Box<Statement>,
        catch_param: Option<String>,
        catch_block: Option<Box<Statement>>,
        finally_block: Option<Box<Statement>>,
    },
    Throw(Expression),
    Return(Option<Expression>),
    FunctionDecl {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    AsyncFunctionDecl {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    GeneratorDecl {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    AsyncGeneratorDecl {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    Import {
        specifiers: Vec<(String, Option<String>)>,
        source: String,
        is_default: bool,
        is_all: bool,
    },
    Export {
        decl: Option<Box<Statement>>,
        is_default: bool,
        specifiers: Vec<(String, Option<String>)>,
        source: Option<String>,
        is_all: bool,
        all_as: Option<String>,
    },
    Empty,
    Debugger,
    With {
        object: Expression,
        body: Box<Statement>,
    },
    ClassDecl(ClassDefinition),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Identifier(String),
    This,
    Super,
    ClassExpr(Box<ClassDefinition>),
    Binary {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expression>,
    },
    Assignment {
        target: String,
        value: Box<Expression>,
    },
    MemberAssignment {
        object: Box<Expression>,
        property: String,
        value: Box<Expression>,
    },
    IndexAssignment {
        object: Box<Expression>,
        index: Box<Expression>,
        value: Box<Expression>,
    },
    Call {
        callee: Box<Expression>,
        args: Vec<Expression>,
    },
    New {
        callee: Box<Expression>,
        args: Vec<Expression>,
    },
    Member {
        object: Box<Expression>,
        property: String,
    },
    Index {
        object: Box<Expression>,
        index: Box<Expression>,
    },
    Object(Vec<ObjectProperty>),
    Array(Vec<Expression>),
    Conditional {
        condition: Box<Expression>,
        then_expr: Box<Expression>,
        else_expr: Box<Expression>,
    },
    FunctionExpr {
        name: Option<String>,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    ArrowExpr {
        params: Vec<String>,
        body: Vec<Statement>,
    },
    MethodExpr {
        name: Option<String>,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    AsyncFunctionExpr {
        name: Option<String>,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    GeneratorExpr {
        name: Option<String>,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    AsyncGeneratorExpr {
        name: Option<String>,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    NewTarget,
    Sequence(Vec<Expression>),
    Await(Box<Expression>),
    Yield(Option<Box<Expression>>),
    YieldStar(Box<Expression>),
    DynamicImport(Box<Expression>),
    Update {
        op: UpdateOp,
        is_prefix: bool,
        expr: Box<Expression>,
    },
    Grouping(Box<Expression>),
    Spread(Box<Expression>),
    PrivateIdentifier(String),
    ObjectRest {
        target: Box<Expression>,
        excluded_keys: Vec<Expression>,
    },
    DestructuringAssignment {
        pattern: Box<Expression>,
        value: Box<Expression>,
    },
    DefineField {
        target: Box<Expression>,
        property: String,
        value: Box<Expression>,
    },
    DefinePrivateAccessor {
        target: Box<Expression>,
        property: String,
        is_getter: bool,
        accessor: Box<Expression>,
    },
    DefinePrivateMethod {
        target: Box<Expression>,
        property: String,
        value: Box<Expression>,
    },
    OptionalChain(OptionalChain),
    TaggedTemplate {
        tag: Box<Expression>,
        quasis: Vec<Option<String>>,
        raw_quasis: Vec<String>,
        exprs: Vec<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptionalChain {
    pub base: Box<Expression>,
    pub chain: Vec<ChainElement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChainElement {
    Property { name: String, optional: bool },
    Index { index: Box<Expression>, optional: bool },
    Call { args: Vec<Expression>, optional: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOp {
    Inc,
    Dec,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Number(f64),
    String { value: String, has_escape: bool, has_legacy_escape: bool },
    Boolean(bool),
    Null,
    Undefined,
    Hole,
    RegExp { pattern: String, flags: String },
    BigInt(BigInt),
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
    Spread(Expression),
    CoverInitializedName(String, Expression),
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
    Field(Option<Expression>),
    StaticBlock(Vec<Statement>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDefinition {
    pub name: Option<String>,
    pub super_class: Option<Box<Expression>>,
    pub ctor_params: Vec<String>,
    pub ctor_body: Vec<Statement>,
    pub has_constructor: bool,
    pub elements: Vec<ClassElement>,
}
