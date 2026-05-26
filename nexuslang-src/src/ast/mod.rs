/// NexusLang AST — nós da árvore sintática abstracta

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(line: usize, column: usize) -> Self {
        Span { line, column }
    }

    pub fn unknown() -> Self {
        Span { line: 0, column: 0 }
    }

    pub fn is_known(self) -> bool {
        self.line != 0 && self.column != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    String,
    Int,
    Float,
    Bool,
    Money,
    Date,
    Array(Box<Type>),
    Optional(Box<Type>),
    Model(String),
    Nil,
    Void,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
    pub unique: bool,
    pub index: bool,
    pub min: Option<Expr>,
    pub max: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ObjectField {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct QueryParam {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub name: String,
    pub model: String,
    pub identity: String,
    pub role: Option<String>,
    pub password_min: usize,
    pub session_ttl_minutes: u64,
    pub idle_ttl_minutes: u64,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RouteAuthGuard {
    pub auth: String,
    pub role: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Expr {
    // Literais
    Integer {
        value: i64,
        span: Span,
    },
    Float {
        value: f64,
        span: Span,
    },
    StringLit {
        value: String,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    Money {
        value: f64,
        currency: String,
        span: Span,
    },
    Array {
        items: Vec<Expr>,
        span: Span,
    },
    Object {
        model: String,
        fields: Vec<ObjectField>,
        span: Span,
    },
    Nil {
        span: Span,
    },

    // Identificador / variável
    Ident {
        name: String,
        span: Span,
    },

    FieldAccess {
        object: Box<Expr>,
        field: String,
        span: Span,
    },

    // Operações binárias
    BinOp {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
        span: Span,
    },

    // Operação unária
    UnaryOp {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },

    // Chamada de função
    Call {
        name: String,
        args: Vec<Expr>,
        span: Span,
    },

    // Acesso a método estático: Employee::all()
    StaticCall {
        ty: String,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Array { span, .. }
            | Expr::Object { span, .. }
            | Expr::Ident { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::BinOp { span, .. }
            | Expr::UnaryOp { span, .. }
            | Expr::Call { span, .. }
            | Expr::StaticCall { span, .. }
            | Expr::Integer { span, .. }
            | Expr::Float { span, .. }
            | Expr::StringLit { span, .. }
            | Expr::Bool { span, .. }
            | Expr::Money { span, .. }
            | Expr::Nil { span } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    // let x = expr  /  let x: type = expr
    Let {
        name: String,
        ty: Option<Type>,
        value: Expr,
        span: Span,
    },

    // const x = expr
    Const {
        name: String,
        ty: Option<Type>,
        value: Expr,
        span: Span,
    },

    // x = expr  (reatribuição)
    Assign {
        name: String,
        value: Expr,
        span: Span,
    },

    // return expr
    Return {
        value: Expr,
        span: Span,
    },

    // print(expr)
    Print {
        value: Expr,
        span: Span,
    },

    // if condition { ... } else { ... }
    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
        span: Span,
    },

    // while condition { ... }
    While {
        condition: Expr,
        body: Vec<Stmt>,
        span: Span,
    },

    // for item in collection { ... }
    For {
        var: String,
        iterable: Expr,
        body: Vec<Stmt>,
        span: Span,
    },

    // expr como statement (chamadas de função sozinhas, etc.)
    ExprStmt {
        expr: Expr,
        span: Span,
    },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. }
            | Stmt::Const { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Print { span, .. }
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::For { span, .. }
            | Stmt::ExprStmt { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Debug, Clone)]
pub struct InvoiceField {
    pub key: String,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct InvoiceItem {
    pub description: Expr,
    pub qty: Expr,
    pub price: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WorkflowStep {
    pub name: String,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Top-level declarations
#[derive(Debug, Clone)]
pub enum Decl {
    // fn name(params) -> type { body }
    Function {
        name: String,
        params: Vec<(String, Type)>,
        return_type: Type,
        body: Vec<Stmt>,
        span: Span,
    },

    // model Name { fields }
    Model {
        name: String,
        fields: Vec<Field>,
        span: Span,
    },

    // workflow Name { step a, step b, ... }
    Workflow {
        name: String,
        steps: Vec<WorkflowStep>,
        span: Span,
    },

    // auth Name { model: User identity: email ... }
    Auth {
        config: AuthConfig,
    },

    // route METHOD /path auth(Name) { body }
    Route {
        method: HttpMethod,
        path: String,
        params: Vec<String>,
        query_params: Vec<QueryParam>,
        auth: Option<RouteAuthGuard>,
        body: Vec<Stmt>,
        span: Span,
    },

    // invoice { key: value, item "..." qty 1 price 100 kz, ... }
    Invoice {
        fields: Vec<InvoiceField>,
        items: Vec<InvoiceItem>,
        span: Span,
    },

    // Statement no topo (scripts)
    Statement(Stmt),
}

impl Decl {
    pub fn span(&self) -> Span {
        match self {
            Decl::Function { span, .. }
            | Decl::Model { span, .. }
            | Decl::Workflow { span, .. }
            | Decl::Route { span, .. }
            | Decl::Invoice { span, .. } => *span,
            Decl::Auth { config } => config.span,
            Decl::Statement(stmt) => stmt.span(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Program {
    pub decls: Vec<Decl>,
}
