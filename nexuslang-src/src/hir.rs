use crate::ast::{
    BinOp, Decl, Expr, HttpMethod, InvoiceField, InvoiceItem, Program, QueryParam, Span, Stmt,
    Type, UnaryOp,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HirDeclId(usize);

impl HirDeclId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HirSymbolId(usize);

impl HirSymbolId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HirExprId(usize);

impl HirExprId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HirRefId(usize);

impl HirRefId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HirTypeId(usize);

impl HirTypeId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HirScopeId(usize);

impl HirScopeId {
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Identifies a module within a multi-module module graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HirModuleId(pub usize);

impl HirModuleId {
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Cross-module reference: points to a symbol in another module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HirSymbolRef {
    pub module: HirModuleId,
    pub symbol: HirSymbolId,
}

/// Visibility of a declaration: Public (exported) or Private.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HirVisibility {
    Private,
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HirDeclKind {
    Function,
    Model,
    Workflow,
    Auth,
    Route,
    Invoice,
    Import,
    Statement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HirSymbolKind {
    Function,
    Model,
    Workflow,
    Auth,
    Route,
    Parameter,
    RouteParameter,
    QueryParameter,
    LetBinding,
    ConstBinding,
    ForBinding,
    ModelField,
    WorkflowStep,
    InvoiceField,
    ImportedSymbol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HirScopeKind {
    TopLevel,
    Function,
    Model,
    Workflow,
    WorkflowStep,
    Auth,
    Route,
    Invoice,
    Block,
    Loop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HirReferenceKind {
    AuthModel,
    AuthIdentityField,
    AuthRoleField,
    RouteAuthGuard,
    ObjectField,
    ModulePath,
    ImportSymbol,
}

#[derive(Debug, Clone)]
pub struct HirProgram<'a> {
    pub decls: Vec<HirDecl<'a>>,
    pub symbols: Vec<HirSymbol<'a>>,
    pub exprs: Vec<HirExpr<'a>>,
    pub references: Vec<HirReference<'a>>,
    pub scopes: Vec<HirScope>,
}

impl<'a> HirProgram<'a> {
    pub fn decl(&self, id: HirDeclId) -> Option<&HirDecl<'a>> {
        self.decls.get(id.index())
    }

    pub fn symbol(&self, id: HirSymbolId) -> Option<&HirSymbol<'a>> {
        self.symbols.get(id.index())
    }

    pub fn expr(&self, id: HirExprId) -> Option<&HirExpr<'a>> {
        self.exprs.get(id.index())
    }

    pub fn reference(&self, id: HirRefId) -> Option<&HirReference<'a>> {
        self.references.get(id.index())
    }

    pub fn scope(&self, id: HirScopeId) -> Option<&HirScope> {
        self.scopes.get(id.index())
    }

    pub fn symbols_named<'b>(
        &'b self,
        name: &'b str,
    ) -> impl Iterator<Item = &'b HirSymbol<'a>> + 'b {
        self.symbols
            .iter()
            .filter(move |symbol| symbol.name == name)
    }
}

#[derive(Debug, Clone)]
pub struct HirDecl<'a> {
    pub id: HirDeclId,
    pub kind: HirDeclKind,
    pub name: Option<&'a str>,
    pub span: Span,
    pub symbol: Option<HirSymbolId>,
    pub body: HirDeclBody<'a>,
    pub scope: Option<HirScopeId>,
    pub visibility: HirVisibility,
}

#[derive(Debug, Clone)]
pub enum HirDeclBody<'a> {
    Function {
        params: Vec<HirSymbolId>,
        return_type: &'a Type,
        body: Vec<HirStmt<'a>>,
    },
    Model {
        fields: Vec<HirModelField<'a>>,
    },
    Workflow {
        steps: Vec<HirWorkflowStep<'a>>,
    },
    Auth {
        model: HirRefId,
        identity: HirRefId,
        role: Option<HirRefId>,
    },
    Route {
        method: &'a HttpMethod,
        path: &'a str,
        params: Vec<HirSymbolId>,
        query_params: Vec<HirRouteQueryParam<'a>>,
        auth: Option<HirRouteAuthGuard<'a>>,
        body: Vec<HirStmt<'a>>,
    },
    Invoice {
        fields: Vec<HirInvoiceField>,
        items: Vec<HirInvoiceItem>,
    },
    Import {
        module: HirRefId,
        imported: HirRefId,
        alias: HirSymbolId,
        resolved: Option<HirSymbolRef>,
    },
    Statement {
        stmt: HirStmt<'a>,
    },
}

#[derive(Debug, Clone)]
pub struct HirSymbol<'a> {
    pub id: HirSymbolId,
    pub name: &'a str,
    pub kind: HirSymbolKind,
    pub decl: Option<HirDeclId>,
    pub ty: Option<&'a Type>,
    pub span: Span,
    pub scope: HirScopeId,
}

#[derive(Debug, Clone)]
pub struct HirScope {
    pub id: HirScopeId,
    pub kind: HirScopeKind,
    pub parent: Option<HirScopeId>,
    pub decl: Option<HirDeclId>,
    pub span: Span,
    pub symbols: Vec<HirSymbolId>,
}

#[derive(Debug, Clone)]
pub struct HirReference<'a> {
    pub id: HirRefId,
    pub kind: HirReferenceKind,
    pub name: &'a str,
    pub owner: HirDeclId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirModelField<'a> {
    pub symbol: HirSymbolId,
    pub name: &'a str,
    pub ty: &'a Type,
    pub default: Option<HirExprId>,
    pub unique: bool,
    pub index: bool,
    pub min: Option<HirExprId>,
    pub max: Option<HirExprId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirWorkflowStep<'a> {
    pub symbol: HirSymbolId,
    pub name: &'a str,
    pub body: Vec<HirStmt<'a>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirRouteQueryParam<'a> {
    pub symbol: HirSymbolId,
    pub name: &'a str,
    pub ty: &'a Type,
    pub default: Option<HirExprId>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct HirRouteAuthGuard<'a> {
    pub auth: HirRefId,
    pub role: Option<&'a str>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirInvoiceField {
    pub symbol: HirSymbolId,
    pub value: HirExprId,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct HirInvoiceItem {
    pub description: HirExprId,
    pub qty: HirExprId,
    pub price: HirExprId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirStmt<'a> {
    pub kind: HirStmtKind<'a>,
    pub span: Span,
    pub source: &'a Stmt,
    pub scope: HirScopeId,
}

#[derive(Debug, Clone)]
pub enum HirStmtKind<'a> {
    Let {
        symbol: HirSymbolId,
        ty: Option<&'a Type>,
        value: HirExprId,
    },
    Const {
        symbol: HirSymbolId,
        ty: Option<&'a Type>,
        value: HirExprId,
    },
    Assign {
        name: &'a str,
        value: HirExprId,
    },
    Return {
        value: HirExprId,
    },
    Print {
        value: HirExprId,
    },
    If {
        condition: HirExprId,
        then_body: Vec<HirStmt<'a>>,
        else_body: Option<Vec<HirStmt<'a>>>,
    },
    While {
        condition: HirExprId,
        body: Vec<HirStmt<'a>>,
    },
    For {
        symbol: HirSymbolId,
        iterable: HirExprId,
        body: Vec<HirStmt<'a>>,
    },
    ExprStmt {
        expr: HirExprId,
    },
}

#[derive(Debug, Clone)]
pub struct HirExpr<'a> {
    pub id: HirExprId,
    pub kind: HirExprKind<'a>,
    pub span: Span,
    pub source: &'a Expr,
    pub scope: HirScopeId,
}

#[derive(Debug, Clone)]
pub enum HirExprKind<'a> {
    Integer(i64),
    Float(f64),
    String(&'a str),
    Bool(bool),
    Money {
        value: f64,
        currency: &'a str,
    },
    Array {
        items: Vec<HirExprId>,
    },
    Object {
        model: &'a str,
        fields: Vec<HirObjectField<'a>>,
    },
    Nil,
    Ident {
        name: &'a str,
    },
    FieldAccess {
        object: HirExprId,
        field: &'a str,
    },
    Binary {
        left: HirExprId,
        op: HirBinaryOp,
        right: HirExprId,
    },
    Unary {
        op: HirUnaryOp,
        expr: HirExprId,
    },
    Call {
        name: &'a str,
        args: Vec<HirExprId>,
    },
    StaticCall {
        ty: &'a str,
        method: &'a str,
        args: Vec<HirExprId>,
    },
}

#[derive(Debug, Clone)]
pub struct HirObjectField<'a> {
    pub name: &'a str,
    pub field_ref: HirRefId,
    pub value: HirExprId,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirBinaryOp {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirUnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Default)]
pub struct HirTypeTable {
    types: Vec<Type>,
}

impl HirTypeTable {
    pub fn intern(&mut self, ty: &Type) -> HirTypeId {
        if let Some(index) = self.types.iter().position(|candidate| candidate == ty) {
            return HirTypeId(index);
        }

        let id = HirTypeId(self.types.len());
        self.types.push(ty.clone());
        id
    }

    pub fn get(&self, id: HirTypeId) -> Option<&Type> {
        self.types.get(id.index())
    }

    pub fn all(&self) -> &[Type] {
        &self.types
    }
}

#[derive(Debug, Clone)]
pub struct HirCheckedMetadata {
    pub types: HirTypeTable,
    exprs: Vec<HirExprMetadata>,
    symbols: Vec<HirSymbolMetadata>,
    references: Vec<HirReferenceMetadata>,
}

impl HirCheckedMetadata {
    pub fn new(expr_count: usize) -> Self {
        Self::with_counts(expr_count, 0)
    }

    pub fn with_counts(expr_count: usize, symbol_count: usize) -> Self {
        Self::with_reference_counts(expr_count, symbol_count, 0)
    }

    pub fn with_reference_counts(
        expr_count: usize,
        symbol_count: usize,
        reference_count: usize,
    ) -> Self {
        Self {
            types: HirTypeTable::default(),
            exprs: (0..expr_count)
                .map(|index| HirExprMetadata {
                    expr: HirExprId(index),
                    ty: None,
                    symbol: None,
                })
                .collect(),
            symbols: (0..symbol_count)
                .map(|index| HirSymbolMetadata {
                    symbol: HirSymbolId(index),
                    ty: None,
                })
                .collect(),
            references: (0..reference_count)
                .map(|index| HirReferenceMetadata {
                    reference: HirRefId(index),
                    symbol: None,
                })
                .collect(),
        }
    }

    pub fn expr_type(&self, expr: HirExprId) -> Option<HirTypeId> {
        self.exprs
            .get(expr.index())
            .and_then(|metadata| metadata.ty)
    }

    pub fn expr_symbol(&self, expr: HirExprId) -> Option<HirSymbolId> {
        self.exprs
            .get(expr.index())
            .and_then(|metadata| metadata.symbol)
    }

    pub fn symbol_type(&self, symbol: HirSymbolId) -> Option<HirTypeId> {
        self.symbols
            .get(symbol.index())
            .and_then(|metadata| metadata.ty)
    }

    pub fn reference_symbol(&self, reference: HirRefId) -> Option<HirSymbolId> {
        self.references
            .get(reference.index())
            .and_then(|metadata| metadata.symbol)
    }

    pub fn ty(&self, ty: HirTypeId) -> Option<&Type> {
        self.types.get(ty)
    }

    pub fn exprs(&self) -> &[HirExprMetadata] {
        &self.exprs
    }

    pub fn symbols(&self) -> &[HirSymbolMetadata] {
        &self.symbols
    }

    pub fn references(&self) -> &[HirReferenceMetadata] {
        &self.references
    }

    pub fn set_expr_type(&mut self, expr: HirExprId, ty: &Type) -> HirTypeId {
        let ty = self.types.intern(ty);
        if let Some(metadata) = self.exprs.get_mut(expr.index()) {
            metadata.ty = Some(ty);
        }
        ty
    }

    pub fn set_expr_symbol(&mut self, expr: HirExprId, symbol: HirSymbolId) {
        if let Some(metadata) = self.exprs.get_mut(expr.index()) {
            metadata.symbol = Some(symbol);
        }
    }

    pub fn set_symbol_type(&mut self, symbol: HirSymbolId, ty: &Type) -> HirTypeId {
        let ty = self.types.intern(ty);
        if let Some(metadata) = self.symbols.get_mut(symbol.index()) {
            metadata.ty = Some(ty);
        }
        ty
    }

    pub fn set_reference_symbol(&mut self, reference: HirRefId, symbol: HirSymbolId) {
        if let Some(metadata) = self.references.get_mut(reference.index()) {
            metadata.symbol = Some(symbol);
        }
    }
}

impl Default for HirCheckedMetadata {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HirExprMetadata {
    pub expr: HirExprId,
    pub ty: Option<HirTypeId>,
    pub symbol: Option<HirSymbolId>,
}

#[derive(Debug, Clone, Copy)]
pub struct HirSymbolMetadata {
    pub symbol: HirSymbolId,
    pub ty: Option<HirTypeId>,
}

#[derive(Debug, Clone, Copy)]
pub struct HirReferenceMetadata {
    pub reference: HirRefId,
    pub symbol: Option<HirSymbolId>,
}

pub fn lower_program(program: &Program) -> HirProgram<'_> {
    lower_checked_program(program)
}

pub fn lower_checked_program(program: &Program) -> HirProgram<'_> {
    let mut lowerer = HirLowerer::new();
    for decl in &program.decls {
        lowerer.lower_decl(decl);
    }
    lowerer.finish()
}

struct HirLowerer<'a> {
    hir: HirProgram<'a>,
    root_scope: HirScopeId,
    /// When `Some`, the next `push_decl` will use this visibility instead
    /// of the one in the struct. Used by `lower_decl_exported`.
    override_visibility: Option<HirVisibility>,
}

impl<'a> HirLowerer<'a> {
    fn new() -> Self {
        let root_scope = HirScopeId(0);
        Self {
            hir: HirProgram {
                decls: Vec::new(),
                symbols: Vec::new(),
                exprs: Vec::new(),
                references: Vec::new(),
                scopes: vec![HirScope {
                    id: root_scope,
                    kind: HirScopeKind::TopLevel,
                    parent: None,
                    decl: None,
                    span: Span::unknown(),
                    symbols: Vec::new(),
                }],
            },
            root_scope,
            override_visibility: None,
        }
    }

    fn finish(self) -> HirProgram<'a> {
        self.hir
    }

    fn next_decl_id(&self) -> HirDeclId {
        HirDeclId(self.hir.decls.len())
    }

    fn push_decl(&mut self, mut decl: HirDecl<'a>) {
        if let Some(visibility) = self.override_visibility.take() {
            decl.visibility = visibility;
        }
        self.hir.decls.push(decl);
    }

    fn push_scope(
        &mut self,
        kind: HirScopeKind,
        parent: Option<HirScopeId>,
        decl: Option<HirDeclId>,
        span: Span,
    ) -> HirScopeId {
        let id = HirScopeId(self.hir.scopes.len());
        self.hir.scopes.push(HirScope {
            id,
            kind,
            parent,
            decl,
            span,
            symbols: Vec::new(),
        });
        id
    }

    fn push_symbol(
        &mut self,
        name: &'a str,
        kind: HirSymbolKind,
        decl: Option<HirDeclId>,
        ty: Option<&'a Type>,
        span: Span,
        scope: HirScopeId,
    ) -> HirSymbolId {
        let id = HirSymbolId(self.hir.symbols.len());
        self.hir.symbols.push(HirSymbol {
            id,
            name,
            kind,
            decl,
            ty,
            span,
            scope,
        });
        if let Some(scope) = self.hir.scopes.get_mut(scope.index()) {
            scope.symbols.push(id);
        }
        id
    }

    fn push_expr(
        &mut self,
        source: &'a Expr,
        kind: HirExprKind<'a>,
        span: Span,
        scope: HirScopeId,
    ) -> HirExprId {
        let id = HirExprId(self.hir.exprs.len());
        self.hir.exprs.push(HirExpr {
            id,
            kind,
            span,
            source,
            scope,
        });
        id
    }

    fn push_reference(
        &mut self,
        name: &'a str,
        kind: HirReferenceKind,
        owner: HirDeclId,
        span: Span,
    ) -> HirRefId {
        let id = HirRefId(self.hir.references.len());
        self.hir.references.push(HirReference {
            id,
            kind,
            name,
            owner,
            span,
        });
        id
    }

    fn lower_decl(&mut self, decl: &'a Decl) {
        match decl {
            Decl::Function {
                name,
                params,
                return_type,
                body,
                span,
            } => {
                let id = self.next_decl_id();
                let function_scope = self.push_scope(
                    HirScopeKind::Function,
                    Some(self.root_scope),
                    Some(id),
                    *span,
                );
                let symbol = self.push_symbol(
                    name.as_str(),
                    HirSymbolKind::Function,
                    Some(id),
                    None,
                    *span,
                    self.root_scope,
                );
                let params = params
                    .iter()
                    .map(|(param_name, param_type)| {
                        self.push_symbol(
                            param_name.as_str(),
                            HirSymbolKind::Parameter,
                            Some(id),
                            Some(param_type),
                            *span,
                            function_scope,
                        )
                    })
                    .collect();
                let body = self.lower_stmts(body, id, function_scope);
                self.push_decl(HirDecl {
                    id,
                    kind: HirDeclKind::Function,
                    name: Some(name.as_str()),
                    span: *span,
                    symbol: Some(symbol),
                    scope: Some(function_scope),
                    visibility: HirVisibility::Private,
                    body: HirDeclBody::Function {
                        params,
                        return_type,
                        body,
                    },
                });
            }
            Decl::Model { name, fields, span } => {
                let id = self.next_decl_id();
                let model_scope =
                    self.push_scope(HirScopeKind::Model, Some(self.root_scope), Some(id), *span);
                let symbol = self.push_symbol(
                    name.as_str(),
                    HirSymbolKind::Model,
                    Some(id),
                    None,
                    *span,
                    self.root_scope,
                );
                let fields = fields
                    .iter()
                    .map(|field| {
                        let symbol = self.push_symbol(
                            field.name.as_str(),
                            HirSymbolKind::ModelField,
                            Some(id),
                            Some(&field.ty),
                            field.span,
                            model_scope,
                        );
                        HirModelField {
                            symbol,
                            name: field.name.as_str(),
                            ty: &field.ty,
                            default: field
                                .default
                                .as_ref()
                                .map(|expr| self.lower_expr(expr, id, model_scope)),
                            unique: field.unique,
                            index: field.index,
                            min: field
                                .min
                                .as_ref()
                                .map(|expr| self.lower_expr(expr, id, model_scope)),
                            max: field
                                .max
                                .as_ref()
                                .map(|expr| self.lower_expr(expr, id, model_scope)),
                            span: field.span,
                        }
                    })
                    .collect();
                self.push_decl(HirDecl {
                    id,
                    kind: HirDeclKind::Model,
                    name: Some(name.as_str()),
                    span: *span,
                    symbol: Some(symbol),
                    scope: Some(model_scope),
                    visibility: HirVisibility::Private,
                    body: HirDeclBody::Model { fields },
                });
            }
            Decl::Workflow { name, steps, span } => {
                let id = self.next_decl_id();
                let workflow_scope = self.push_scope(
                    HirScopeKind::Workflow,
                    Some(self.root_scope),
                    Some(id),
                    *span,
                );
                let symbol = self.push_symbol(
                    name.as_str(),
                    HirSymbolKind::Workflow,
                    Some(id),
                    None,
                    *span,
                    self.root_scope,
                );
                let steps = steps
                    .iter()
                    .map(|step| {
                        let step_scope = self.push_scope(
                            HirScopeKind::WorkflowStep,
                            Some(workflow_scope),
                            Some(id),
                            step.span,
                        );
                        let symbol = self.push_symbol(
                            step.name.as_str(),
                            HirSymbolKind::WorkflowStep,
                            Some(id),
                            None,
                            step.span,
                            workflow_scope,
                        );
                        HirWorkflowStep {
                            symbol,
                            name: step.name.as_str(),
                            body: self.lower_stmts(&step.body, id, step_scope),
                            span: step.span,
                        }
                    })
                    .collect();
                self.push_decl(HirDecl {
                    id,
                    kind: HirDeclKind::Workflow,
                    name: Some(name.as_str()),
                    span: *span,
                    symbol: Some(symbol),
                    scope: Some(workflow_scope),
                    visibility: HirVisibility::Private,
                    body: HirDeclBody::Workflow { steps },
                });
            }
            Decl::Auth { config } => {
                let id = self.next_decl_id();
                let auth_scope = self.push_scope(
                    HirScopeKind::Auth,
                    Some(self.root_scope),
                    Some(id),
                    config.span,
                );
                let symbol = self.push_symbol(
                    config.name.as_str(),
                    HirSymbolKind::Auth,
                    Some(id),
                    None,
                    config.span,
                    self.root_scope,
                );
                let model = self.push_reference(
                    config.model.as_str(),
                    HirReferenceKind::AuthModel,
                    id,
                    config.span,
                );
                let identity = self.push_reference(
                    config.identity.as_str(),
                    HirReferenceKind::AuthIdentityField,
                    id,
                    config.span,
                );
                let role = config.role.as_ref().map(|role| {
                    self.push_reference(
                        role.as_str(),
                        HirReferenceKind::AuthRoleField,
                        id,
                        config.span,
                    )
                });
                self.push_decl(HirDecl {
                    id,
                    kind: HirDeclKind::Auth,
                    name: Some(config.name.as_str()),
                    span: config.span,
                    symbol: Some(symbol),
                    scope: Some(auth_scope),
                    visibility: HirVisibility::Private,
                    body: HirDeclBody::Auth {
                        model,
                        identity,
                        role,
                    },
                });
            }
            Decl::Route {
                method,
                path,
                params,
                query_params,
                auth,
                body,
                span,
            } => {
                let id = self.next_decl_id();
                let route_scope =
                    self.push_scope(HirScopeKind::Route, Some(self.root_scope), Some(id), *span);
                let symbol = self.push_symbol(
                    path.as_str(),
                    HirSymbolKind::Route,
                    Some(id),
                    None,
                    *span,
                    self.root_scope,
                );
                let params = params
                    .iter()
                    .map(|param| {
                        self.push_symbol(
                            param.as_str(),
                            HirSymbolKind::RouteParameter,
                            Some(id),
                            None,
                            *span,
                            route_scope,
                        )
                    })
                    .collect();
                let query_params = self.lower_query_params(query_params, id, route_scope);
                let auth = auth.as_ref().map(|guard| {
                    let auth = self.push_reference(
                        guard.auth.as_str(),
                        HirReferenceKind::RouteAuthGuard,
                        id,
                        guard.span,
                    );
                    HirRouteAuthGuard {
                        auth,
                        role: guard.role.as_deref(),
                        span: guard.span,
                    }
                });
                let body = self.lower_stmts(body, id, route_scope);
                self.push_decl(HirDecl {
                    id,
                    kind: HirDeclKind::Route,
                    name: Some(path.as_str()),
                    span: *span,
                    symbol: Some(symbol),
                    scope: Some(route_scope),
                    visibility: HirVisibility::Private,
                    body: HirDeclBody::Route {
                        method,
                        path,
                        params,
                        query_params,
                        auth,
                        body,
                    },
                });
            }
            Decl::Invoice {
                fields,
                items,
                span,
            } => {
                let id = self.next_decl_id();
                let invoice_scope = self.push_scope(
                    HirScopeKind::Invoice,
                    Some(self.root_scope),
                    Some(id),
                    *span,
                );
                let fields = self.lower_invoice_fields(fields, id, invoice_scope);
                let items = items
                    .iter()
                    .map(|item| self.lower_invoice_item(item, id, invoice_scope))
                    .collect();
                self.push_decl(HirDecl {
                    id,
                    kind: HirDeclKind::Invoice,
                    name: None,
                    span: *span,
                    symbol: None,
                    scope: Some(invoice_scope),
                    visibility: HirVisibility::Private,
                    body: HirDeclBody::Invoice { fields, items },
                });
            }
            Decl::Import { import } => {
                let id = self.next_decl_id();
                let module = self.push_reference(
                    import.source.as_str(),
                    HirReferenceKind::ModulePath,
                    id,
                    import.source_span,
                );
                let imported = self.push_reference(
                    import.name.as_str(),
                    HirReferenceKind::ImportSymbol,
                    id,
                    import.name_span,
                );
                let alias_name = import.alias.as_deref().unwrap_or(&import.name);
                let alias_span = import.alias_span.unwrap_or(import.name_span);
                let alias_sym = self.push_symbol(
                    alias_name,
                    HirSymbolKind::ImportedSymbol,
                    Some(id),
                    None,
                    alias_span,
                    self.root_scope,
                );
                self.push_decl(HirDecl {
                    id,
                    kind: HirDeclKind::Import,
                    name: Some(alias_name),
                    span: import.span,
                    symbol: Some(alias_sym),
                    scope: Some(self.root_scope),
                    visibility: HirVisibility::Private,
                    body: HirDeclBody::Import {
                        module,
                        imported,
                        alias: alias_sym,
                        resolved: None,
                    },
                });
            }
            Decl::Export { decl: inner, .. } => {
                // Lower the inner declaration with Public visibility.
                self.lower_decl_exported(inner);
            }
            Decl::Statement(stmt) => {
                let id = self.next_decl_id();
                let stmt = self.lower_stmt(stmt, id, self.root_scope);
                self.push_decl(HirDecl {
                    id,
                    kind: HirDeclKind::Statement,
                    name: None,
                    span: stmt.span,
                    symbol: None,
                    scope: Some(self.root_scope),
                    visibility: HirVisibility::Private,
                    body: HirDeclBody::Statement { stmt },
                });
            }
        }
    }

    /// Lower a declaration that is wrapped in `export`. The inner declaration
    /// receives `Public` visibility so the module-level resolver can collect
    /// it in the module's export table.
    fn lower_decl_exported(&mut self, decl: &'a Decl) {
        self.override_visibility = Some(HirVisibility::Public);
        self.lower_decl(decl);
    }

    fn lower_query_params(
        &mut self,
        query_params: &'a [QueryParam],
        decl: HirDeclId,
        scope: HirScopeId,
    ) -> Vec<HirRouteQueryParam<'a>> {
        query_params
            .iter()
            .map(|param| {
                let symbol = self.push_symbol(
                    param.name.as_str(),
                    HirSymbolKind::QueryParameter,
                    Some(decl),
                    Some(&param.ty),
                    param.span,
                    scope,
                );
                HirRouteQueryParam {
                    symbol,
                    name: param.name.as_str(),
                    ty: &param.ty,
                    default: param
                        .default
                        .as_ref()
                        .map(|expr| self.lower_expr(expr, decl, scope)),
                    span: param.span,
                }
            })
            .collect()
    }

    fn lower_invoice_fields(
        &mut self,
        fields: &'a [InvoiceField],
        decl: HirDeclId,
        scope: HirScopeId,
    ) -> Vec<HirInvoiceField> {
        fields
            .iter()
            .map(|field| {
                let symbol = self.push_symbol(
                    field.key.as_str(),
                    HirSymbolKind::InvoiceField,
                    Some(decl),
                    None,
                    field.span,
                    scope,
                );
                HirInvoiceField {
                    symbol,
                    value: self.lower_expr(&field.value, decl, scope),
                    span: field.span,
                }
            })
            .collect()
    }

    fn lower_invoice_item(
        &mut self,
        item: &'a InvoiceItem,
        decl: HirDeclId,
        scope: HirScopeId,
    ) -> HirInvoiceItem {
        HirInvoiceItem {
            description: self.lower_expr(&item.description, decl, scope),
            qty: self.lower_expr(&item.qty, decl, scope),
            price: self.lower_expr(&item.price, decl, scope),
            span: item.span,
        }
    }

    fn lower_stmts(
        &mut self,
        stmts: &'a [Stmt],
        decl: HirDeclId,
        scope: HirScopeId,
    ) -> Vec<HirStmt<'a>> {
        stmts
            .iter()
            .map(|stmt| self.lower_stmt(stmt, decl, scope))
            .collect()
    }

    fn lower_stmt(&mut self, stmt: &'a Stmt, decl: HirDeclId, scope: HirScopeId) -> HirStmt<'a> {
        match stmt {
            Stmt::Let {
                name,
                ty,
                value,
                span,
            } => {
                let symbol = self.push_symbol(
                    name.as_str(),
                    HirSymbolKind::LetBinding,
                    Some(decl),
                    ty.as_ref(),
                    *span,
                    scope,
                );
                HirStmt {
                    kind: HirStmtKind::Let {
                        symbol,
                        ty: ty.as_ref(),
                        value: self.lower_expr(value, decl, scope),
                    },
                    span: *span,
                    source: stmt,
                    scope,
                }
            }
            Stmt::Const {
                name,
                ty,
                value,
                span,
            } => {
                let symbol = self.push_symbol(
                    name.as_str(),
                    HirSymbolKind::ConstBinding,
                    Some(decl),
                    ty.as_ref(),
                    *span,
                    scope,
                );
                HirStmt {
                    kind: HirStmtKind::Const {
                        symbol,
                        ty: ty.as_ref(),
                        value: self.lower_expr(value, decl, scope),
                    },
                    span: *span,
                    source: stmt,
                    scope,
                }
            }
            Stmt::Assign { name, value, span } => HirStmt {
                kind: HirStmtKind::Assign {
                    name: name.as_str(),
                    value: self.lower_expr(value, decl, scope),
                },
                span: *span,
                source: stmt,
                scope,
            },
            Stmt::Return { value, span } => HirStmt {
                kind: HirStmtKind::Return {
                    value: self.lower_expr(value, decl, scope),
                },
                span: *span,
                source: stmt,
                scope,
            },
            Stmt::Print { value, span } => HirStmt {
                kind: HirStmtKind::Print {
                    value: self.lower_expr(value, decl, scope),
                },
                span: *span,
                source: stmt,
                scope,
            },
            Stmt::If {
                condition,
                then_body,
                else_body,
                span,
            } => {
                let then_scope =
                    self.push_scope(HirScopeKind::Block, Some(scope), Some(decl), *span);
                let else_scope = else_body
                    .as_ref()
                    .map(|_| self.push_scope(HirScopeKind::Block, Some(scope), Some(decl), *span));
                HirStmt {
                    kind: HirStmtKind::If {
                        condition: self.lower_expr(condition, decl, scope),
                        then_body: self.lower_stmts(then_body, decl, then_scope),
                        else_body: else_body.as_ref().zip(else_scope).map(
                            |(else_body, else_scope)| self.lower_stmts(else_body, decl, else_scope),
                        ),
                    },
                    span: *span,
                    source: stmt,
                    scope,
                }
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                let loop_scope =
                    self.push_scope(HirScopeKind::Loop, Some(scope), Some(decl), *span);
                HirStmt {
                    kind: HirStmtKind::While {
                        condition: self.lower_expr(condition, decl, scope),
                        body: self.lower_stmts(body, decl, loop_scope),
                    },
                    span: *span,
                    source: stmt,
                    scope,
                }
            }
            Stmt::For {
                var,
                iterable,
                body,
                span,
            } => {
                let loop_scope =
                    self.push_scope(HirScopeKind::Loop, Some(scope), Some(decl), *span);
                let symbol = self.push_symbol(
                    var.as_str(),
                    HirSymbolKind::ForBinding,
                    Some(decl),
                    None,
                    *span,
                    loop_scope,
                );
                HirStmt {
                    kind: HirStmtKind::For {
                        symbol,
                        iterable: self.lower_expr(iterable, decl, scope),
                        body: self.lower_stmts(body, decl, loop_scope),
                    },
                    span: *span,
                    source: stmt,
                    scope,
                }
            }
            Stmt::ExprStmt { expr, span } => HirStmt {
                kind: HirStmtKind::ExprStmt {
                    expr: self.lower_expr(expr, decl, scope),
                },
                span: *span,
                source: stmt,
                scope,
            },
        }
    }

    fn lower_expr(&mut self, expr: &'a Expr, decl: HirDeclId, scope: HirScopeId) -> HirExprId {
        match expr {
            Expr::Integer { value, span } => {
                self.push_expr(expr, HirExprKind::Integer(*value), *span, scope)
            }
            Expr::Float { value, span } => {
                self.push_expr(expr, HirExprKind::Float(*value), *span, scope)
            }
            Expr::StringLit { value, span } => {
                self.push_expr(expr, HirExprKind::String(value.as_str()), *span, scope)
            }
            Expr::Bool { value, span } => {
                self.push_expr(expr, HirExprKind::Bool(*value), *span, scope)
            }
            Expr::Money {
                value,
                currency,
                span,
            } => self.push_expr(
                expr,
                HirExprKind::Money {
                    value: *value,
                    currency: currency.as_str(),
                },
                *span,
                scope,
            ),
            Expr::Array { items, span } => {
                let items = items
                    .iter()
                    .map(|item| self.lower_expr(item, decl, scope))
                    .collect();
                self.push_expr(expr, HirExprKind::Array { items }, *span, scope)
            }
            Expr::Object {
                model,
                fields,
                span,
            } => {
                let fields = fields
                    .iter()
                    .map(|field| {
                        let field_ref = self.push_reference(
                            field.name.as_str(),
                            HirReferenceKind::ObjectField,
                            decl,
                            field.span,
                        );
                        HirObjectField {
                            name: field.name.as_str(),
                            field_ref,
                            value: self.lower_expr(&field.value, decl, scope),
                            span: field.span,
                        }
                    })
                    .collect();
                self.push_expr(
                    expr,
                    HirExprKind::Object {
                        model: model.as_str(),
                        fields,
                    },
                    *span,
                    scope,
                )
            }
            Expr::Nil { span } => self.push_expr(expr, HirExprKind::Nil, *span, scope),
            Expr::Ident { name, span } => self.push_expr(
                expr,
                HirExprKind::Ident {
                    name: name.as_str(),
                },
                *span,
                scope,
            ),
            Expr::FieldAccess {
                object,
                field,
                span,
            } => {
                let object = self.lower_expr(object, decl, scope);
                self.push_expr(
                    expr,
                    HirExprKind::FieldAccess {
                        object,
                        field: field.as_str(),
                    },
                    *span,
                    scope,
                )
            }
            Expr::BinOp {
                left,
                op,
                right,
                span,
            } => {
                let left = self.lower_expr(left, decl, scope);
                let right = self.lower_expr(right, decl, scope);
                self.push_expr(
                    expr,
                    HirExprKind::Binary {
                        left,
                        op: HirBinaryOp::from(op),
                        right,
                    },
                    *span,
                    scope,
                )
            }
            Expr::UnaryOp {
                op,
                expr: inner_expr,
                span,
            } => {
                let inner = self.lower_expr(inner_expr, decl, scope);
                self.push_expr(
                    expr,
                    HirExprKind::Unary {
                        op: HirUnaryOp::from(op),
                        expr: inner,
                    },
                    *span,
                    scope,
                )
            }
            Expr::Call { name, args, span } => {
                let args = args
                    .iter()
                    .map(|arg| self.lower_expr(arg, decl, scope))
                    .collect();
                self.push_expr(
                    expr,
                    HirExprKind::Call {
                        name: name.as_str(),
                        args,
                    },
                    *span,
                    scope,
                )
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } => {
                let args = args
                    .iter()
                    .map(|arg| self.lower_expr(arg, decl, scope))
                    .collect();
                self.push_expr(
                    expr,
                    HirExprKind::StaticCall {
                        ty: ty.as_str(),
                        method: method.as_str(),
                        args,
                    },
                    *span,
                    scope,
                )
            }
        }
    }
}

impl From<&BinOp> for HirBinaryOp {
    fn from(op: &BinOp) -> Self {
        match op {
            BinOp::Add => Self::Add,
            BinOp::Sub => Self::Sub,
            BinOp::Mul => Self::Mul,
            BinOp::Div => Self::Div,
            BinOp::Mod => Self::Mod,
            BinOp::Eq => Self::Eq,
            BinOp::NotEq => Self::NotEq,
            BinOp::Lt => Self::Lt,
            BinOp::LtEq => Self::LtEq,
            BinOp::Gt => Self::Gt,
            BinOp::GtEq => Self::GtEq,
            BinOp::And => Self::And,
            BinOp::Or => Self::Or,
        }
    }
}

impl From<&UnaryOp> for HirUnaryOp {
    fn from(op: &UnaryOp) -> Self {
        match op {
            UnaryOp::Neg => Self::Neg,
            UnaryOp::Not => Self::Not,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowering_assigns_stable_ids_for_decls_symbols_and_exprs() {
        let source = r#"
model Customer {
    name: string
}

model User {
    email: string unique
}

auth UserAuth {
    model: User
    identity: email
}

fn customer_name(customer: Customer) -> string {
    let local: string = customer.name
    return local
}

route GET /customers/:name ?(limit: int = 10) {
    return Customer::find("name", name)
}
"#;

        let first_program = crate::parse_checked_source(source).unwrap();
        let second_program = crate::parse_checked_source(source).unwrap();
        let first = lower_checked_program(&first_program);
        let second = lower_checked_program(&second_program);

        let decls: Vec<_> = first
            .decls
            .iter()
            .map(|decl| (decl.id.index(), decl.kind, decl.name))
            .collect();
        assert_eq!(
            decls,
            vec![
                (0, HirDeclKind::Model, Some("Customer")),
                (1, HirDeclKind::Model, Some("User")),
                (2, HirDeclKind::Auth, Some("UserAuth")),
                (3, HirDeclKind::Function, Some("customer_name")),
                (4, HirDeclKind::Route, Some("/customers/:name")),
            ]
        );

        let first_symbols: Vec<_> = first
            .symbols
            .iter()
            .map(|symbol| (symbol.id.index(), symbol.kind, symbol.name, symbol.decl))
            .collect();
        let second_symbols: Vec<_> = second
            .symbols
            .iter()
            .map(|symbol| (symbol.id.index(), symbol.kind, symbol.name, symbol.decl))
            .collect();
        assert_eq!(first_symbols, second_symbols);

        let first_exprs: Vec<_> = first
            .exprs
            .iter()
            .map(|expr| (expr.id.index(), expr.span))
            .collect();
        let second_exprs: Vec<_> = second
            .exprs
            .iter()
            .map(|expr| (expr.id.index(), expr.span))
            .collect();
        assert_eq!(first_exprs, second_exprs);

        let first_references: Vec<_> = first
            .references
            .iter()
            .map(|reference| {
                (
                    reference.id.index(),
                    reference.kind,
                    reference.name,
                    reference.owner,
                )
            })
            .collect();
        let second_references: Vec<_> = second
            .references
            .iter()
            .map(|reference| {
                (
                    reference.id.index(),
                    reference.kind,
                    reference.name,
                    reference.owner,
                )
            })
            .collect();
        assert_eq!(first_references, second_references);
        assert_eq!(
            first_references,
            vec![
                (0, HirReferenceKind::AuthModel, "User", HirDeclId(2)),
                (
                    1,
                    HirReferenceKind::AuthIdentityField,
                    "email",
                    HirDeclId(2)
                ),
            ]
        );
    }

    #[test]
    fn lowering_indexes_object_literal_fields_as_references() {
        let source = r#"
model Customer {
    name: string
    active: bool
}

route GET /literal {
    return Customer { name: "Ana", active: true }
}
"#;

        let first_program = crate::parse_checked_source(source).unwrap();
        let second_program = crate::parse_checked_source(source).unwrap();
        let first = lower_checked_program(&first_program);
        let second = lower_checked_program(&second_program);

        let first_references: Vec<_> = first
            .references
            .iter()
            .map(|reference| {
                (
                    reference.id.index(),
                    reference.kind,
                    reference.name,
                    reference.owner,
                )
            })
            .collect();
        let second_references: Vec<_> = second
            .references
            .iter()
            .map(|reference| {
                (
                    reference.id.index(),
                    reference.kind,
                    reference.name,
                    reference.owner,
                )
            })
            .collect();
        assert_eq!(first_references, second_references);
        assert_eq!(
            first_references,
            vec![
                (0, HirReferenceKind::ObjectField, "name", HirDeclId(1)),
                (1, HirReferenceKind::ObjectField, "active", HirDeclId(1)),
            ]
        );

        let fields = first
            .exprs
            .iter()
            .find_map(|expr| match &expr.kind {
                HirExprKind::Object { model, fields } if *model == "Customer" => Some(fields),
                _ => None,
            })
            .unwrap();
        assert_eq!(fields.len(), 2);

        for field in fields {
            let reference = first.reference(field.field_ref).unwrap();
            assert_eq!(reference.kind, HirReferenceKind::ObjectField);
            assert_eq!(reference.name, field.name);
            assert_eq!(reference.owner, HirDeclId(1));
            assert_eq!(reference.span, field.span);
        }
    }

    #[test]
    fn lowering_indexes_route_symbols_and_checked_static_calls() {
        let source = r#"
model Customer {
    name: string
}

route GET /customers/:name ?(limit: int = 10) {
    return Customer::find("name", name)
}
"#;

        let program = crate::parse_checked_source(source).unwrap();
        let hir = lower_checked_program(&program);

        assert!(hir
            .symbols_named("Customer")
            .any(|symbol| symbol.kind == HirSymbolKind::Model));
        assert!(hir
            .symbols_named("name")
            .any(|symbol| symbol.kind == HirSymbolKind::RouteParameter));
        assert!(hir
            .symbols_named("limit")
            .any(|symbol| symbol.kind == HirSymbolKind::QueryParameter));

        let static_call = hir.exprs.iter().find_map(|expr| match &expr.kind {
            HirExprKind::StaticCall { ty, method, args } => Some((*ty, *method, args.len())),
            _ => None,
        });

        assert_eq!(static_call, Some(("Customer", "find", 2)));
    }

    #[test]
    fn lowering_models_lexical_scopes_for_decls_blocks_and_loops() {
        let source = r#"
model Customer {
    name: string
}

fn summarize(customers: [Customer]) -> string {
    for customer in customers {
        let current: string = customer.name
    }
    if true {
        const label: string = "ok"
    }
    return "ok"
}

route GET /customers/:name ?(limit: int = 10) {
    return Customer::find("name", name)
}
"#;

        let program = crate::parse_checked_source(source).unwrap();
        let hir = lower_checked_program(&program);
        let root = hir.scope(HirScopeId(0)).unwrap();
        assert_eq!(root.kind, HirScopeKind::TopLevel);
        assert_eq!(root.parent, None);

        let customer_model = hir
            .symbols
            .iter()
            .find(|symbol| symbol.kind == HirSymbolKind::Model && symbol.name == "Customer")
            .unwrap();
        let function = hir
            .decls
            .iter()
            .find(|decl| decl.kind == HirDeclKind::Function)
            .unwrap();
        let route = hir
            .decls
            .iter()
            .find(|decl| decl.kind == HirDeclKind::Route)
            .unwrap();
        assert!(root.symbols.contains(&customer_model.id));
        assert!(root.symbols.contains(&function.symbol.unwrap()));
        assert!(root.symbols.contains(&route.symbol.unwrap()));

        let function_scope = hir.scope(function.scope.unwrap()).unwrap();
        assert_eq!(function_scope.kind, HirScopeKind::Function);
        assert_eq!(function_scope.parent, Some(root.id));
        let customers_param = hir
            .symbols_named("customers")
            .find(|symbol| symbol.kind == HirSymbolKind::Parameter)
            .unwrap();
        assert_eq!(customers_param.scope, function_scope.id);

        let loop_binding = hir
            .symbols_named("customer")
            .find(|symbol| symbol.kind == HirSymbolKind::ForBinding)
            .unwrap();
        let loop_scope = hir.scope(loop_binding.scope).unwrap();
        assert_eq!(loop_scope.kind, HirScopeKind::Loop);
        assert_eq!(loop_scope.parent, Some(function_scope.id));

        let loop_local = hir
            .symbols_named("current")
            .find(|symbol| symbol.kind == HirSymbolKind::LetBinding)
            .unwrap();
        assert_eq!(loop_local.scope, loop_scope.id);

        let block_local = hir
            .symbols_named("label")
            .find(|symbol| symbol.kind == HirSymbolKind::ConstBinding)
            .unwrap();
        let block_scope = hir.scope(block_local.scope).unwrap();
        assert_eq!(block_scope.kind, HirScopeKind::Block);
        assert_eq!(block_scope.parent, Some(function_scope.id));

        let route_scope = hir.scope(route.scope.unwrap()).unwrap();
        assert_eq!(route_scope.kind, HirScopeKind::Route);
        assert_eq!(route_scope.parent, Some(root.id));
        assert!(hir.symbols_named("name").any(|symbol| {
            symbol.kind == HirSymbolKind::RouteParameter && symbol.scope == route_scope.id
        }));
        assert!(hir.symbols_named("limit").any(|symbol| {
            symbol.kind == HirSymbolKind::QueryParameter && symbol.scope == route_scope.id
        }));
    }
}
