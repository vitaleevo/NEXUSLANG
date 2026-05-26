use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostic::{Diagnostic, DiagnosticStage};

type CheckResult<T> = Result<T, Diagnostic>;

#[derive(Debug, Clone)]
struct FunctionSig {
    params: Vec<(String, Type)>,
    return_type: Type,
}

#[derive(Debug, Default, Clone)]
struct Scope {
    vars: HashMap<String, Type>,
    consts: HashSet<String>,
}

impl Scope {
    fn define(&mut self, name: &str, ty: Type, is_const: bool) {
        self.vars.insert(name.to_string(), ty);
        if is_const {
            self.consts.insert(name.to_string());
        }
    }

    fn assign(&mut self, name: &str, ty: &Type) -> Result<(), String> {
        if self.consts.contains(name) {
            return Err(format!("Constante '{}' não pode ser reatribuída", name));
        }

        let Some(existing) = self.vars.get(name) else {
            return Err(format!("Variável '{}' não definida", name));
        };

        ensure_assignable(existing, ty)
            .map_err(|e| format!("Tipo inválido ao atribuir '{}': {}", name, e))
    }

    fn get(&self, name: &str) -> Option<&Type> {
        self.vars.get(name)
    }
}

pub struct Checker {
    functions: HashMap<String, FunctionSig>,
    models: HashMap<String, Vec<Field>>,
    auths: HashMap<String, AuthConfig>,
    workflows: HashSet<String>,
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}

impl Checker {
    pub fn new() -> Self {
        Checker {
            functions: HashMap::new(),
            models: HashMap::new(),
            auths: HashMap::new(),
            workflows: HashSet::new(),
        }
    }

    pub fn check(&mut self, program: &Program) -> Result<(), String> {
        self.check_diagnostic(program)
            .map_err(|diagnostic| diagnostic.to_string())
    }

    pub fn check_diagnostic(&mut self, program: &Program) -> CheckResult<()> {
        self.collect_decls(program)?;
        self.check_decls(program)
    }

    fn error(&self, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(DiagnosticStage::Checker, message).with_span(span)
    }

    fn collect_decls(&mut self, program: &Program) -> CheckResult<()> {
        for decl in &program.decls {
            if let Decl::Model { name, fields, .. } = decl {
                if self.models.contains_key(name) {
                    return Err(self.error(
                        decl.span(),
                        format!("Model '{}' declarado mais de uma vez", name),
                    ));
                }
                if reserved_openapi_component_name(name) {
                    return Err(self.error(
                        decl.span(),
                        format!(
                            "Model '{}' usa nome reservado para componentes OpenAPI internos: NexusError, NexusPage_* ou NexusList_*",
                            name
                        ),
                    ));
                }
                let mut seen_fields = HashSet::new();
                for field in fields {
                    if !seen_fields.insert(field.name.as_str()) {
                        return Err(self.error(
                            field.span,
                            format!("Campo '{}.{}' declarado mais de uma vez", name, field.name),
                        ));
                    }
                }
                self.models.insert(name.clone(), fields.clone());
            }
            if let Decl::Workflow { name, .. } = decl {
                if !self.workflows.insert(name.clone()) {
                    return Err(self.error(
                        decl.span(),
                        format!("Workflow '{}' declarado mais de uma vez", name),
                    ));
                }
            }
            if let Decl::Auth { config } = decl {
                if self.auths.contains_key(&config.name) {
                    return Err(self.error(
                        config.span,
                        format!("Auth '{}' declarado mais de uma vez", config.name),
                    ));
                }
                self.auths.insert(config.name.clone(), config.clone());
            }
        }

        let mut route_signatures = HashSet::new();

        for decl in &program.decls {
            match decl {
                Decl::Function {
                    name,
                    params,
                    return_type,
                    ..
                } => {
                    if self.functions.contains_key(name) {
                        return Err(self.error(
                            decl.span(),
                            format!("Função '{}' declarada mais de uma vez", name),
                        ));
                    }
                    for (_, ty) in params {
                        self.ensure_known_type(ty, decl.span())?;
                    }
                    self.ensure_known_type(return_type, decl.span())?;
                    self.functions.insert(
                        name.clone(),
                        FunctionSig {
                            params: params.clone(),
                            return_type: return_type.clone(),
                        },
                    );
                }
                Decl::Model { name, fields, .. } => {
                    let default_scope = Scope::default();
                    for field in fields {
                        self.ensure_known_type(&field.ty, field.span)
                            .map_err(|diagnostic| {
                                self.error(
                                    field.span,
                                    format!(
                                        "Campo '{}.{}': {}",
                                        name, field.name, diagnostic.message
                                    ),
                                )
                            })?;
                        if field.unique && !unique_constraint_type_supported(&field.ty) {
                            return Err(self.error(
                                field.span,
                                format!(
                                    "Campo '{}.{}': unique so suporta string, int, float, bool, money, date ou opcionais desses tipos",
                                    name, field.name
                                ),
                            ));
                        }
                        if field.index && !index_constraint_type_supported(&field.ty) {
                            return Err(self.error(
                                field.span,
                                format!(
                                    "Campo '{}.{}': index so suporta string, int, float, bool, money, date ou opcionais desses tipos",
                                    name, field.name
                                ),
                            ));
                        }
                        if field.min.is_some() || field.max.is_some() {
                            self.check_model_min_max_constraints(name, field, &default_scope)?;
                        }
                        if let Some(default) = &field.default {
                            self.ensure_static_model_default(default)
                                .map_err(|diagnostic| {
                                    let span = Span::new(
                                        diagnostic.line.unwrap_or(field.span.line),
                                        diagnostic.column.unwrap_or(field.span.column),
                                    );
                                    self.error(
                                        span,
                                        format!(
                                            "Campo '{}.{}': {}",
                                            name, field.name, diagnostic.message
                                        ),
                                    )
                                })?;
                            let actual = self.infer_expr(default, &default_scope)?;
                            ensure_assignable(&field.ty, &actual).map_err(|e| {
                                let span = if default.span().is_known() {
                                    default.span()
                                } else {
                                    field.span
                                };
                                self.error(
                                    span,
                                    format!(
                                        "Campo '{}.{}' default invalido: {}",
                                        name, field.name, e
                                    ),
                                )
                            })?;
                            validate_default_against_min_max(
                                &field.ty, default, &field.min, &field.max,
                            )
                            .map_err(|message| {
                                let span = if default.span().is_known() {
                                    default.span()
                                } else {
                                    field.span
                                };
                                self.error(
                                    span,
                                    format!("Campo '{}.{}': {}", name, field.name, message),
                                )
                            })?;
                        }
                    }
                }
                Decl::Route {
                    method,
                    path,
                    params,
                    query_params,
                    auth,
                    ..
                } => {
                    let signature = (route_method_name(method), path.as_str());
                    if !route_signatures.insert(signature) {
                        return Err(self.error(
                            decl.span(),
                            format!(
                                "Route {} '{}' declarada mais de uma vez",
                                route_method_name(method),
                                path
                            ),
                        ));
                    }
                    let mut seen = HashSet::new();
                    for param in params {
                        if !seen.insert(param) {
                            return Err(self.error(
                                decl.span(),
                                format!(
                                    "Route '{}' declara parâmetro '{}' mais de uma vez",
                                    path, param
                                ),
                            ));
                        }
                    }
                    let default_scope = Scope::default();
                    for param in query_params {
                        self.ensure_known_type(&param.ty, param.span)?;
                        if !query_param_type_supported(&param.ty) {
                            return Err(self.error(
                                param.span,
                                format!(
                                    "Route '{}' query param '{}' usa tipo nao suportado: {}",
                                    path,
                                    param.name,
                                    type_name(&param.ty)
                                ),
                            ));
                        }
                        if let Some(default) = &param.default {
                            self.ensure_static_default_expr(default, "default de query param")?;
                            let actual = self.infer_expr(default, &default_scope)?;
                            ensure_query_default_assignable(&param.ty, &actual).map_err(|e| {
                                let span = if default.span().is_known() {
                                    default.span()
                                } else {
                                    param.span
                                };
                                self.error(
                                    span,
                                    format!(
                                        "Route '{}' query param '{}' default invalido: {}",
                                        path, param.name, e
                                    ),
                                )
                            })?;
                        }
                        if !seen.insert(&param.name) {
                            return Err(self.error(
                                param.span,
                                format!(
                                    "Route '{}' declara parâmetro '{}' mais de uma vez",
                                    path, param.name
                                ),
                            ));
                        }
                    }
                    if let Some(guard) = auth {
                        self.check_route_auth_guard(path, guard)?;
                    }
                }
                Decl::Auth { config } => {
                    self.check_auth_config(config)?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn check_auth_config(&self, config: &AuthConfig) -> CheckResult<()> {
        let fields = self.models.get(&config.model).ok_or_else(|| {
            self.error(
                config.span,
                format!(
                    "Auth '{}' referencia model '{}' inexistente",
                    config.name, config.model
                ),
            )
        })?;

        let Some(identity) = fields.iter().find(|field| field.name == config.identity) else {
            return Err(self.error(
                config.span,
                format!(
                    "Auth '{}' identity '{}.{}' nao existe",
                    config.name, config.model, config.identity
                ),
            ));
        };
        if !matches!(identity.ty, Type::String) || !identity.unique {
            return Err(self.error(
                identity.span,
                format!(
                    "Auth '{}' identity '{}.{}' deve ser string unique",
                    config.name, config.model, config.identity
                ),
            ));
        }

        if let Some(role) = &config.role {
            let Some(role_field) = fields.iter().find(|field| field.name == *role) else {
                return Err(self.error(
                    config.span,
                    format!(
                        "Auth '{}' role '{}.{}' nao existe",
                        config.name, config.model, role
                    ),
                ));
            };
            if !matches!(role_field.ty, Type::String) {
                return Err(self.error(
                    role_field.span,
                    format!(
                        "Auth '{}' role '{}.{}' deve ser string",
                        config.name, config.model, role
                    ),
                ));
            }
        }

        if config.password_min < 15 {
            return Err(self.error(
                config.span,
                format!("Auth '{}' password_min deve ser pelo menos 15", config.name),
            ));
        }
        if config.session_ttl_minutes == 0 || config.idle_ttl_minutes == 0 {
            return Err(self.error(
                config.span,
                format!("Auth '{}' TTLs devem ser maiores que zero", config.name),
            ));
        }
        if config.idle_ttl_minutes > config.session_ttl_minutes {
            return Err(self.error(
                config.span,
                format!(
                    "Auth '{}' idle_ttl_minutes nao pode exceder session_ttl_minutes",
                    config.name
                ),
            ));
        }

        Ok(())
    }

    fn check_route_auth_guard(&self, path: &str, guard: &RouteAuthGuard) -> CheckResult<()> {
        let Some(config) = self.auths.get(&guard.auth) else {
            return Err(self.error(
                guard.span,
                format!("Route '{}' usa auth '{}' inexistente", path, guard.auth),
            ));
        };
        if guard.role.is_some() && config.role.is_none() {
            return Err(self.error(
                guard.span,
                format!(
                    "Route '{}' exige role, mas Auth '{}' nao declarou role",
                    path, guard.auth
                ),
            ));
        }
        Ok(())
    }

    fn ensure_static_model_default(&self, expr: &Expr) -> CheckResult<()> {
        self.ensure_static_default_expr(expr, "default de model field")
    }

    fn check_model_min_max_constraints(
        &self,
        model_name: &str,
        field: &Field,
        scope: &Scope,
    ) -> CheckResult<()> {
        if !min_max_constraint_type_supported(&field.ty) {
            return Err(self.error(
                field.span,
                format!(
                    "Campo '{}.{}': min/max so suporta string, int, float, money, date ou opcionais desses tipos",
                    model_name, field.name
                ),
            ));
        }

        if let Some(min) = &field.min {
            self.check_model_min_max_bound(model_name, field, "min", min, scope)?;
        }
        if let Some(max) = &field.max {
            self.check_model_min_max_bound(model_name, field, "max", max, scope)?;
        }
        if let (Some(min), Some(max)) = (&field.min, &field.max) {
            ensure_min_max_bounds_ordered(&field.ty, min, max).map_err(|message| {
                self.error(
                    field.span,
                    format!("Campo '{}.{}': {}", model_name, field.name, message),
                )
            })?;
        }

        Ok(())
    }

    fn check_model_min_max_bound(
        &self,
        model_name: &str,
        field: &Field,
        constraint: &str,
        expr: &Expr,
        scope: &Scope,
    ) -> CheckResult<()> {
        self.ensure_static_min_max_expr(expr, constraint)?;
        let actual = self.infer_expr(expr, scope)?;
        ensure_min_max_bound_assignable(&field.ty, constraint, &actual).map_err(|message| {
            let span = if expr.span().is_known() {
                expr.span()
            } else {
                field.span
            };
            self.error(
                span,
                format!("Campo '{}.{}': {}", model_name, field.name, message),
            )
        })?;
        validate_min_max_bound_literal(&field.ty, constraint, expr).map_err(|message| {
            let span = if expr.span().is_known() {
                expr.span()
            } else {
                field.span
            };
            self.error(
                span,
                format!("Campo '{}.{}': {}", model_name, field.name, message),
            )
        })
    }

    fn ensure_static_min_max_expr(&self, expr: &Expr, constraint: &str) -> CheckResult<()> {
        match expr {
            Expr::Integer { .. }
            | Expr::Float { .. }
            | Expr::StringLit { .. }
            | Expr::Money { .. } => Ok(()),
            Expr::Array { span, .. }
            | Expr::Object { span, .. }
            | Expr::Bool { span, .. }
            | Expr::Nil { span }
            | Expr::Ident { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::BinOp { span, .. }
            | Expr::UnaryOp { span, .. }
            | Expr::Call { span, .. }
            | Expr::StaticCall { span, .. } => Err(self.error(
                *span,
                format!("constraint '{}' de model field nesta fase deve ser literal numerico, string ou money", constraint),
            )),
        }
    }

    fn ensure_static_default_expr(&self, expr: &Expr, context: &str) -> CheckResult<()> {
        match expr {
            Expr::Integer { .. }
            | Expr::Float { .. }
            | Expr::StringLit { .. }
            | Expr::Bool { .. }
            | Expr::Money { .. }
            | Expr::Nil { .. } => Ok(()),
            Expr::Array { items, .. } => {
                for item in items {
                    self.ensure_static_default_expr(item, context)?;
                }
                Ok(())
            }
            Expr::Object { span, .. }
            | Expr::Ident { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::BinOp { span, .. }
            | Expr::UnaryOp { span, .. }
            | Expr::Call { span, .. }
            | Expr::StaticCall { span, .. } => Err(self.error(
                *span,
                format!("{context} nesta fase deve ser literal, nil ou array literal"),
            )),
        }
    }

    fn check_decls(&self, program: &Program) -> CheckResult<()> {
        let mut top_scope = Scope::default();

        for decl in &program.decls {
            if let Decl::Statement(stmt) = decl {
                self.check_stmt(stmt, &mut top_scope, &Type::Unknown)?;
            }
        }

        for decl in &program.decls {
            match decl {
                Decl::Function {
                    name,
                    params,
                    return_type,
                    body,
                    span,
                } => {
                    let mut scope = top_scope.clone();
                    for (name, ty) in params {
                        scope.define(name, ty.clone(), false);
                    }
                    self.check_stmts(body, &mut scope, return_type)?;
                    if *return_type != Type::Void && !block_guarantees_return(body) {
                        return Err(self.error(
                            *span,
                            format!(
                                "Funcao '{}' deve retornar {} em todos os caminhos",
                                name,
                                type_name(return_type)
                            ),
                        ));
                    }
                }
                Decl::Route {
                    method,
                    path,
                    params,
                    query_params,
                    auth,
                    body,
                    span,
                } => self.check_route(method, path, params, query_params, auth, body, *span)?,
                Decl::Invoice {
                    fields,
                    items,
                    span,
                } => {
                    self.check_invoice_contract(fields, items, *span)?;
                    for field in fields {
                        let actual = self.infer_expr(&field.value, &top_scope)?;
                        self.check_invoice_field(&field.key, &actual, field.span)?;
                    }
                    for item in items {
                        let description = self.infer_expr(&item.description, &top_scope)?;
                        ensure_assignable(&Type::String, &description).map_err(|e| {
                            self.error(
                                item.span,
                                format!("Invoice item description inválida: {}", e),
                            )
                        })?;
                        let qty = self.infer_expr(&item.qty, &top_scope)?;
                        if !matches!(qty, Type::Int | Type::Float) {
                            return Err(self.error(
                                item.span,
                                format!(
                                    "Invoice item qty espera int ou float, encontrado {}",
                                    type_name(&qty)
                                ),
                            ));
                        }
                        let price = self.infer_expr(&item.price, &top_scope)?;
                        ensure_assignable(&Type::Money, &price).map_err(|e| {
                            self.error(item.span, format!("Invoice item price inválido: {}", e))
                        })?;
                    }
                }
                Decl::Statement(_) => {}
                Decl::Auth { .. } => {}
                Decl::Workflow { steps, .. } => {
                    for step in steps {
                        let mut scope = top_scope.clone();
                        self.check_stmts(&step.body, &mut scope, &Type::Unknown)?;
                    }
                }
                Decl::Model { .. } => {}
            }
        }

        Ok(())
    }

    fn check_stmts(
        &self,
        stmts: &[Stmt],
        scope: &mut Scope,
        expected_return: &Type,
    ) -> CheckResult<()> {
        for stmt in stmts {
            self.check_stmt(stmt, scope, expected_return)?;
        }
        Ok(())
    }

    fn check_stmt(
        &self,
        stmt: &Stmt,
        scope: &mut Scope,
        expected_return: &Type,
    ) -> CheckResult<()> {
        match stmt {
            Stmt::Let {
                name,
                ty,
                value,
                span,
            } => self.check_binding(name, ty, value, false, scope, *span),
            Stmt::Const {
                name,
                ty,
                value,
                span,
            } => self.check_binding(name, ty, value, true, scope, *span),
            Stmt::Assign { name, value, span } => {
                let value_ty = self.infer_expr(value, scope)?;
                scope
                    .assign(name, &value_ty)
                    .map_err(|message| self.error(*span, message))
            }
            Stmt::Return { value, span } => {
                let actual = self.infer_expr(value, scope)?;
                if *expected_return == Type::Void {
                    return Err(
                        self.error(*span, "Funcao sem tipo de retorno nao pode retornar valor")
                    );
                }
                if *expected_return != Type::Unknown {
                    ensure_assignable(expected_return, &actual).map_err(|e| {
                        self.error(*span, format!("Tipo de retorno inválido: {}", e))
                    })?;
                }
                Ok(())
            }
            Stmt::Print { value, .. } => {
                self.infer_expr(value, scope)?;
                Ok(())
            }
            Stmt::ExprStmt { expr, .. } => {
                self.infer_expr(expr, scope)?;
                Ok(())
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
                span,
            } => {
                let cond_ty = self.infer_expr(condition, scope)?;
                ensure_assignable(&Type::Bool, &cond_ty)
                    .map_err(|e| self.error(*span, format!("Condição de if inválida: {}", e)))?;
                self.check_stmts(then_body, scope, expected_return)?;
                if let Some(stmts) = else_body {
                    self.check_stmts(stmts, scope, expected_return)?;
                }
                Ok(())
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                let cond_ty = self.infer_expr(condition, scope)?;
                ensure_assignable(&Type::Bool, &cond_ty)
                    .map_err(|e| self.error(*span, format!("Condição de while inválida: {}", e)))?;
                self.check_stmts(body, scope, expected_return)
            }
            Stmt::For {
                var,
                iterable,
                body,
                span,
            } => {
                let iter_ty = self.infer_expr(iterable, scope)?;
                let item_ty = match iter_ty {
                    Type::Array(inner) => *inner,
                    Type::Unknown => Type::Unknown,
                    other => {
                        return Err(self.error(
                            *span,
                            format!("For espera array, encontrado {}", type_name(&other)),
                        ));
                    }
                };
                scope.define(var, item_ty, false);
                self.check_stmts(body, scope, expected_return)
            }
        }
    }

    fn check_binding(
        &self,
        name: &str,
        annotation: &Option<Type>,
        value: &Expr,
        is_const: bool,
        scope: &mut Scope,
        span: Span,
    ) -> CheckResult<()> {
        let inferred = self.infer_expr(value, scope)?;
        let final_ty = if let Some(expected) = annotation {
            self.ensure_known_type(expected, span)?;
            ensure_assignable(expected, &inferred)
                .map_err(|e| self.error(span, format!("Tipo inválido para '{}': {}", name, e)))?;
            expected.clone()
        } else {
            inferred
        };

        scope.define(name, final_ty, is_const);
        Ok(())
    }

    fn infer_expr(&self, expr: &Expr, scope: &Scope) -> CheckResult<Type> {
        match expr {
            Expr::Integer { .. } => Ok(Type::Int),
            Expr::Float { .. } => Ok(Type::Float),
            Expr::StringLit { .. } => Ok(Type::String),
            Expr::Bool { .. } => Ok(Type::Bool),
            Expr::Money { .. } => Ok(Type::Money),
            Expr::Nil { .. } => Ok(Type::Nil),
            Expr::Array { items, span } => {
                let mut item_type = Type::Unknown;
                for item in items {
                    let ty = self.infer_expr(item, scope)?;
                    if item_type == Type::Unknown {
                        item_type = ty;
                    } else {
                        ensure_assignable(&item_type, &ty).map_err(|e| {
                            let error_span = if item.span().is_known() {
                                item.span()
                            } else {
                                *span
                            };
                            self.error(error_span, format!("Array com tipos incompatíveis: {}", e))
                        })?;
                    }
                }
                Ok(Type::Array(Box::new(item_type)))
            }
            Expr::Object {
                model,
                fields,
                span,
            } => {
                self.check_object_fields(model, fields, *span, scope)?;
                Ok(Type::Model(model.clone()))
            }
            Expr::Ident { name, span } => scope
                .get(name)
                .cloned()
                .ok_or_else(|| self.error(*span, format!("Variável '{}' não definida", name))),
            Expr::FieldAccess {
                object,
                field,
                span,
            } => self.infer_field_access(object, field, scope, *span),
            Expr::UnaryOp { op, expr, span } => {
                let ty = self.infer_expr(expr, scope)?;
                match op {
                    UnaryOp::Neg => match ty {
                        Type::Int | Type::Float | Type::Money => Ok(ty),
                        _ => Err(self
                            .error(*span, format!("Operador '-' não aceita {}", type_name(&ty)))),
                    },
                    UnaryOp::Not => {
                        ensure_assignable(&Type::Bool, &ty).map_err(|e| {
                            self.error(*span, format!("Operador '!' inválido: {}", e))
                        })?;
                        Ok(Type::Bool)
                    }
                }
            }
            Expr::BinOp {
                left,
                op,
                right,
                span,
            } => self.infer_binop(left, op, right, scope, *span),
            Expr::Call { name, args, span } => self.infer_call(name, args, scope, *span),
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } => {
                if !self.models.contains_key(ty) {
                    return Err(self.error(*span, format!("Model '{}' não encontrado", ty)));
                }
                if method != "all" {
                    return Err(self.error(
                        *span,
                        format!("Método estático '{}::{}' não existe", ty, method),
                    ));
                }
                if !args.is_empty() {
                    return Err(self.error(
                        *span,
                        format!("{}::all() fora de route nao recebe argumentos", ty),
                    ));
                }
                Ok(Type::Array(Box::new(Type::Model(ty.clone()))))
            }
        }
    }

    fn check_object_fields(
        &self,
        model: &str,
        fields: &[ObjectField],
        span: Span,
        scope: &Scope,
    ) -> CheckResult<()> {
        let model_fields = self
            .models
            .get(model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;

        let expected = model_fields
            .iter()
            .map(|field| (field.name.as_str(), &field.ty))
            .collect::<HashMap<_, _>>();
        let mut seen = HashSet::new();

        for field in fields {
            if !seen.insert(field.name.as_str()) {
                return Err(self.error(
                    field.span,
                    format!("Campo '{}.{}' declarado mais de uma vez", model, field.name),
                ));
            }

            let Some(expected_ty) = expected.get(field.name.as_str()) else {
                return Err(self.error(
                    field.span,
                    format!("Campo '{}.{}' nao existe", model, field.name),
                ));
            };

            let actual = self.infer_expr(&field.value, scope)?;
            ensure_assignable(expected_ty, &actual).map_err(|e| {
                self.error(
                    field.span,
                    format!("Campo '{}.{}': {}", model, field.name, e),
                )
            })?;
        }

        for field in model_fields {
            if !seen.contains(field.name.as_str())
                && field.default.is_none()
                && !is_optional_type(&field.ty)
            {
                return Err(self.error(
                    span,
                    format!("Campo '{}.{}' obrigatorio ausente", model, field.name),
                ));
            }
        }

        Ok(())
    }

    fn infer_field_access(
        &self,
        object: &Expr,
        field: &str,
        scope: &Scope,
        span: Span,
    ) -> CheckResult<Type> {
        let object_ty = self.infer_expr(object, scope)?;
        let Type::Model(model) = object_ty else {
            return Err(self.error(
                span,
                format!(
                    "Acesso a campo '{}' espera model instance, encontrado {}",
                    field,
                    type_name(&object_ty)
                ),
            ));
        };

        let model_fields = self
            .models
            .get(&model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;

        model_fields
            .iter()
            .find(|candidate| candidate.name == field)
            .map(|candidate| candidate.ty.clone())
            .ok_or_else(|| self.error(span, format!("Campo '{}.{}' nao existe", model, field)))
    }

    fn infer_binop(
        &self,
        left: &Expr,
        op: &BinOp,
        right: &Expr,
        scope: &Scope,
        span: Span,
    ) -> CheckResult<Type> {
        let left_ty = self.infer_expr(left, scope)?;
        let right_ty = self.infer_expr(right, scope)?;

        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                numeric_result(op, &left_ty, &right_ty).map_err(|message| self.error(span, message))
            }
            BinOp::Eq | BinOp::NotEq => {
                ensure_assignable(&left_ty, &right_ty)
                    .map_err(|message| self.error(span, message))?;
                Ok(Type::Bool)
            }
            BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => {
                ensure_comparable(&left_ty, &right_ty)
                    .map_err(|message| self.error(span, message))?;
                Ok(Type::Bool)
            }
            BinOp::And | BinOp::Or => {
                ensure_assignable(&Type::Bool, &left_ty)
                    .map_err(|message| self.error(span, message))?;
                ensure_assignable(&Type::Bool, &right_ty)
                    .map_err(|message| self.error(span, message))?;
                Ok(Type::Bool)
            }
        }
    }

    fn infer_call(
        &self,
        name: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<Type> {
        match name {
            "print" => return Ok(Type::Void),
            "len" => {
                if args.len() != 1 {
                    return Err(self.error(span, "len() recebe exatamente 1 argumento"));
                }
                let ty = self.infer_expr(&args[0], scope)?;
                if !matches!(ty, Type::Array(_) | Type::String | Type::Unknown) {
                    return Err(self.error(span, format!("len() não aceita {}", type_name(&ty))));
                }
                return Ok(Type::Int);
            }
            "str" => {
                if args.len() != 1 {
                    return Err(self.error(span, "str() recebe exatamente 1 argumento"));
                }
                self.infer_expr(&args[0], scope)?;
                return Ok(Type::String);
            }
            "run_workflow" => {
                if args.len() != 1 {
                    return Err(self.error(span, "run_workflow() recebe exatamente 1 argumento"));
                }
                let ty = self.infer_expr(&args[0], scope)?;
                ensure_assignable(&Type::String, &ty).map_err(|e| {
                    self.error(span, format!("run_workflow() espera string: {}", e))
                })?;
                if let Expr::StringLit { value: name, .. } = &args[0] {
                    if !self.workflows.contains(name) {
                        return Err(self.error(span, format!("Workflow '{}' não encontrado", name)));
                    }
                }
                return Ok(Type::Void);
            }
            _ => {}
        }

        let sig = self
            .functions
            .get(name)
            .ok_or_else(|| self.error(span, format!("Função '{}' não definida", name)))?;

        if args.len() != sig.params.len() {
            return Err(self.error(
                span,
                format!(
                    "Função '{}' espera {} argumento(s), recebeu {}",
                    name,
                    sig.params.len(),
                    args.len()
                ),
            ));
        }

        for (arg, (_, expected)) in args.iter().zip(sig.params.iter()) {
            let actual = self.infer_expr(arg, scope)?;
            ensure_assignable(expected, &actual).map_err(|e| {
                let error_span = if arg.span().is_known() {
                    arg.span()
                } else {
                    span
                };
                self.error(
                    error_span,
                    format!("Argumento inválido em '{}': {}", name, e),
                )
            })?;
        }

        Ok(sig.return_type.clone())
    }

    fn ensure_known_type(&self, ty: &Type, span: Span) -> CheckResult<()> {
        match ty {
            Type::Unknown => Err(self.error(span, "tipo desconhecido")),
            Type::Nil => Err(self.error(span, "tipo nil nao pode ser usado como anotacao")),
            Type::Array(inner) => self.ensure_known_type(inner, span),
            Type::Optional(inner) => self.ensure_known_type(inner, span),
            Type::Model(name) if self.models.contains_key(name) => Ok(()),
            Type::Model(name) => {
                Err(self.error(span, format!("Model type '{}' não encontrado", name)))
            }
            _ => Ok(()),
        }
    }

    fn check_route(
        &self,
        method: &HttpMethod,
        path: &str,
        params: &[String],
        query_params: &[QueryParam],
        auth: &Option<RouteAuthGuard>,
        body: &[Stmt],
        span: Span,
    ) -> CheckResult<()> {
        if let Some(guard) = auth {
            self.check_route_auth_guard(path, guard)?;
        }
        if body.len() != 1 {
            return Err(self.error(
                span,
                format!("Route '{}' deve conter um unico return direto", path),
            ));
        }

        let Stmt::Return {
            value,
            span: return_span,
        } = &body[0]
        else {
            return Err(self.error(
                body.first().map(Stmt::span).unwrap_or(span),
                format!("Route '{}' deve conter um unico return direto", path),
            ));
        };

        let mut scope = Scope::default();
        for param in params {
            scope.define(param, Type::String, false);
        }
        for param in query_params {
            scope.define(&param.name, param.ty.clone(), false);
        }

        self.ensure_route_expr(value, &scope, method)?;
        let actual = self.infer_route_return_expr(value, &scope)?;
        self.ensure_route_return_type(path, &actual, *return_span)
    }

    fn check_invoice_contract(
        &self,
        fields: &[InvoiceField],
        items: &[InvoiceItem],
        span: Span,
    ) -> CheckResult<()> {
        let mut keys = HashSet::new();
        let mut has_customer = false;
        let mut has_currency = false;
        let mut has_total = false;

        for field in fields {
            if !keys.insert(field.key.as_str()) {
                return Err(self.error(
                    field.span,
                    format!("Invoice field '{}' declarado mais de uma vez", field.key),
                ));
            }
            has_customer |= field.key == "customer";
            has_currency |= field.key == "currency";
            has_total |= field.key == "total";
        }

        if !has_customer {
            return Err(self.error(span, "Invoice deve declarar customer"));
        }
        if !has_currency {
            return Err(self.error(span, "Invoice deve declarar currency"));
        }
        if items.is_empty() && !has_total {
            return Err(self.error(span, "Invoice deve declarar item ou total"));
        }

        Ok(())
    }

    fn infer_route_return_expr(&self, expr: &Expr, scope: &Scope) -> CheckResult<Type> {
        if let Expr::StaticCall {
            ty,
            method,
            args,
            span,
        } = expr
        {
            if ty == "Auth" {
                return self.infer_auth_return_expr(method, args, *span);
            }
            if method == "all" {
                self.check_model_all_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "page" {
                self.check_model_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "create" {
                if !self.models.contains_key(ty) {
                    return Err(self.error(*span, format!("Model '{}' nao encontrado", ty)));
                }
                if !args.is_empty() {
                    return Err(
                        self.error(*span, format!("{}::create() nao recebe argumentos", ty))
                    );
                }
                return Ok(Type::Model(ty.clone()));
            }
            if method == "find" {
                self.check_model_lookup_call(ty, args, scope, *span, "find")?;
                return Ok(Type::Model(ty.clone()));
            }
            if method == "where" {
                self.check_model_where_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_page" {
                self.check_model_where_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_not" {
                self.check_model_where_not_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_not_page" {
                self.check_model_where_not_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_not_in" {
                self.check_model_where_not_in_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_not_in_page" {
                self.check_model_where_not_in_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_not_in_optional" {
                self.check_model_where_not_in_optional_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_not_in_optional_page" {
                self.check_model_where_not_in_optional_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_optional" {
                self.check_model_where_optional_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_optional_page" {
                self.check_model_where_optional_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_in" {
                self.check_model_where_in_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_in_page" {
                self.check_model_where_in_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_in_optional" {
                self.check_model_where_in_optional_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_in_optional_page" {
                self.check_model_where_in_optional_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_compare" {
                self.check_model_where_compare_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_compare_page" {
                self.check_model_where_compare_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_text" {
                self.check_model_where_text_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_text_page" {
                self.check_model_where_text_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_between" {
                self.check_model_where_between_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_between_page" {
                self.check_model_where_between_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_all" {
                self.check_model_where_all_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_all_page" {
                self.check_model_where_all_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_any" {
                self.check_model_where_any_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "where_any_page" {
                self.check_model_where_any_page_call(ty, args, scope, *span)?;
                return Ok(Type::Array(Box::new(Type::Model(ty.clone()))));
            }
            if method == "update" {
                self.check_model_lookup_call(ty, args, scope, *span, "update")?;
                return Ok(Type::Model(ty.clone()));
            }
            if method == "delete" {
                self.check_model_lookup_call(ty, args, scope, *span, "delete")?;
                return Ok(Type::Model(ty.clone()));
            }
        }

        self.infer_expr(expr, scope)
    }

    fn infer_auth_return_expr(&self, method: &str, args: &[Expr], span: Span) -> CheckResult<Type> {
        match method {
            "register" | "login" => {
                let config = self.check_auth_config_arg(method, args, span)?;
                Ok(Type::Model(config.model.clone()))
            }
            "user" => {
                if !args.is_empty() {
                    return Err(self.error(span, "Auth::user() nao recebe argumentos"));
                }
                Ok(Type::String)
            }
            "logout" => {
                if !args.is_empty() {
                    return Err(self.error(span, "Auth::logout() nao recebe argumentos"));
                }
                Ok(Type::Bool)
            }
            _ => Err(self.error(
                span,
                format!("Metodo estatico 'Auth::{}' nao existe", method),
            )),
        }
    }

    fn check_auth_config_arg<'a>(
        &'a self,
        method: &str,
        args: &[Expr],
        span: Span,
    ) -> CheckResult<&'a AuthConfig> {
        if args.len() != 1 {
            return Err(self.error(span, format!("Auth::{}() recebe exatamente 1 auth", method)));
        }
        let Expr::Ident { name, .. } = &args[0] else {
            return Err(self.error(
                args[0].span(),
                format!("Auth::{}() espera nome de auth", method),
            ));
        };
        self.auths
            .get(name)
            .ok_or_else(|| self.error(args[0].span(), format!("Auth '{}' nao declarado", name)))
    }

    fn check_model_all_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if !self.models.contains_key(model) {
            return Err(self.error(span, format!("Model '{}' nao encontrado", model)));
        }
        match args.len() {
            0 => Ok(()),
            2 if starts_ordering_args(args) => {
                self.check_ordering_args(model, "all", &args[0], &args[1])
            }
            2 => self.check_pagination_args(model, "all", &args[0], &args[1], scope),
            4 => {
                self.check_ordering_args(model, "all", &args[0], &args[1])?;
                self.check_pagination_args(model, "all", &args[2], &args[3], scope)
            }
            _ => Err(self.error(
                span,
                format!(
                    "{}::all() recebe zero argumentos, limit e offset, ordenacao ou ordenacao com paginacao",
                    model
                ),
            )),
        }
    }

    fn check_model_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if !self.models.contains_key(model) {
            return Err(self.error(span, format!("Model '{}' nao encontrado", model)));
        }
        match args.len() {
            2 => self.check_pagination_args(model, "page", &args[0], &args[1], scope),
            4 => {
                self.check_ordering_args(model, "page", &args[0], &args[1])?;
                self.check_pagination_args(model, "page", &args[2], &args[3], scope)
            }
            _ => Err(self.error(
                span,
                format!(
                    "{}::page() recebe limit/offset ou ordenacao com limit/offset",
                    model
                ),
            )),
        }
    }

    fn check_model_where_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 2 && args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where() recebe campo e valor, com ordenacao e limit/offset opcionais",
                    model
                ),
            ));
        }
        self.check_model_lookup_args(model, args, scope, span, "where")?;
        if args.len() == 4 {
            if starts_ordering_args(&args[2..]) {
                self.check_ordering_args(model, "where", &args[2], &args[3])?;
            } else {
                self.check_pagination_args(model, "where", &args[2], &args[3], scope)?;
            }
        } else if args.len() == 6 {
            self.check_ordering_args(model, "where", &args[2], &args[3])?;
            self.check_pagination_args(model, "where", &args[4], &args[5], scope)?;
        }
        Ok(())
    }

    fn check_model_where_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_page() recebe campo, valor e limit/offset, com ordenacao opcional",
                    model
                ),
            ));
        }
        self.check_model_lookup_args(model, &args[..2], scope, span, "where_page")?;
        if args.len() == 6 {
            self.check_ordering_args(model, "where_page", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_page", &args[4], &args[5], scope)?;
        } else {
            self.check_pagination_args(model, "where_page", &args[2], &args[3], scope)?;
        }
        Ok(())
    }

    fn check_model_where_not_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 2 && args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_not() recebe campo e valor, com ordenacao e limit/offset opcionais",
                    model
                ),
            ));
        }
        self.check_model_lookup_args(model, args, scope, span, "where_not")?;
        if args.len() == 4 {
            if starts_ordering_args(&args[2..]) {
                self.check_ordering_args(model, "where_not", &args[2], &args[3])?;
            } else {
                self.check_pagination_args(model, "where_not", &args[2], &args[3], scope)?;
            }
        } else if args.len() == 6 {
            self.check_ordering_args(model, "where_not", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_not", &args[4], &args[5], scope)?;
        }
        Ok(())
    }

    fn check_model_where_not_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_not_page() recebe campo, valor e limit/offset, com ordenacao opcional",
                    model
                ),
            ));
        }
        self.check_model_lookup_args(model, &args[..2], scope, span, "where_not_page")?;
        if args.len() == 6 {
            self.check_ordering_args(model, "where_not_page", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_not_page", &args[4], &args[5], scope)?;
        } else {
            self.check_pagination_args(model, "where_not_page", &args[2], &args[3], scope)?;
        }
        Ok(())
    }

    fn check_model_where_optional_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 2 && args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_optional() recebe campo e valor opcional, com ordenacao e limit/offset opcionais",
                    model
                ),
            ));
        }
        self.check_model_optional_lookup_args(model, args, scope, span, "where_optional")?;
        if args.len() == 4 {
            if starts_ordering_args(&args[2..]) {
                self.check_ordering_args(model, "where_optional", &args[2], &args[3])?;
            } else {
                self.check_pagination_args(model, "where_optional", &args[2], &args[3], scope)?;
            }
        } else if args.len() == 6 {
            self.check_ordering_args(model, "where_optional", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_optional", &args[4], &args[5], scope)?;
        }
        Ok(())
    }

    fn check_model_where_optional_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_optional_page() recebe campo, valor opcional e limit/offset, com ordenacao opcional",
                    model
                ),
            ));
        }
        self.check_model_optional_lookup_args(
            model,
            &args[..2],
            scope,
            span,
            "where_optional_page",
        )?;
        if args.len() == 6 {
            self.check_ordering_args(model, "where_optional_page", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_optional_page", &args[4], &args[5], scope)?;
        } else {
            self.check_pagination_args(model, "where_optional_page", &args[2], &args[3], scope)?;
        }
        Ok(())
    }

    fn check_model_where_in_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 2 && args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_in() recebe campo e array de valores, com ordenacao e limit/offset opcionais",
                    model
                ),
            ));
        }
        self.check_model_where_in_args(model, args, scope, span, "where_in")?;
        if args.len() == 4 {
            if starts_ordering_args(&args[2..]) {
                self.check_ordering_args(model, "where_in", &args[2], &args[3])?;
            } else {
                self.check_pagination_args(model, "where_in", &args[2], &args[3], scope)?;
            }
        } else if args.len() == 6 {
            self.check_ordering_args(model, "where_in", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_in", &args[4], &args[5], scope)?;
        }
        Ok(())
    }

    fn check_model_where_in_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_in_page() recebe campo, array de valores e limit/offset, com ordenacao opcional",
                    model
                ),
            ));
        }
        self.check_model_where_in_args(model, &args[..2], scope, span, "where_in_page")?;
        if args.len() == 6 {
            self.check_ordering_args(model, "where_in_page", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_in_page", &args[4], &args[5], scope)?;
        } else {
            self.check_pagination_args(model, "where_in_page", &args[2], &args[3], scope)?;
        }
        Ok(())
    }

    fn check_model_where_not_in_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 2 && args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_not_in() recebe campo e array de valores, com ordenacao e limit/offset opcionais",
                    model
                ),
            ));
        }
        self.check_model_where_in_args(model, args, scope, span, "where_not_in")?;
        if args.len() == 4 {
            if starts_ordering_args(&args[2..]) {
                self.check_ordering_args(model, "where_not_in", &args[2], &args[3])?;
            } else {
                self.check_pagination_args(model, "where_not_in", &args[2], &args[3], scope)?;
            }
        } else if args.len() == 6 {
            self.check_ordering_args(model, "where_not_in", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_not_in", &args[4], &args[5], scope)?;
        }
        Ok(())
    }

    fn check_model_where_not_in_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_not_in_page() recebe campo, array de valores e limit/offset, com ordenacao opcional",
                    model
                ),
            ));
        }
        self.check_model_where_in_args(model, &args[..2], scope, span, "where_not_in_page")?;
        if args.len() == 6 {
            self.check_ordering_args(model, "where_not_in_page", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_not_in_page", &args[4], &args[5], scope)?;
        } else {
            self.check_pagination_args(model, "where_not_in_page", &args[2], &args[3], scope)?;
        }
        Ok(())
    }

    fn check_model_where_not_in_optional_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 2 && args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_not_in_optional() recebe campo e array opcional de valores, com ordenacao e limit/offset opcionais",
                    model
                ),
            ));
        }
        self.check_model_where_in_optional_args(model, args, scope, span, "where_not_in_optional")?;
        if args.len() == 4 {
            if starts_ordering_args(&args[2..]) {
                self.check_ordering_args(model, "where_not_in_optional", &args[2], &args[3])?;
            } else {
                self.check_pagination_args(
                    model,
                    "where_not_in_optional",
                    &args[2],
                    &args[3],
                    scope,
                )?;
            }
        } else if args.len() == 6 {
            self.check_ordering_args(model, "where_not_in_optional", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_not_in_optional", &args[4], &args[5], scope)?;
        }
        Ok(())
    }

    fn check_model_where_not_in_optional_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_not_in_optional_page() recebe campo, array opcional de valores e limit/offset, com ordenacao opcional",
                    model
                ),
            ));
        }
        self.check_model_where_in_optional_args(
            model,
            &args[..2],
            scope,
            span,
            "where_not_in_optional_page",
        )?;
        if args.len() == 6 {
            self.check_ordering_args(model, "where_not_in_optional_page", &args[2], &args[3])?;
            self.check_pagination_args(
                model,
                "where_not_in_optional_page",
                &args[4],
                &args[5],
                scope,
            )?;
        } else {
            self.check_pagination_args(
                model,
                "where_not_in_optional_page",
                &args[2],
                &args[3],
                scope,
            )?;
        }
        Ok(())
    }

    fn check_model_where_in_optional_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 2 && args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_in_optional() recebe campo e array opcional de valores, com ordenacao e limit/offset opcionais",
                    model
                ),
            ));
        }
        self.check_model_where_in_optional_args(model, args, scope, span, "where_in_optional")?;
        if args.len() == 4 {
            if starts_ordering_args(&args[2..]) {
                self.check_ordering_args(model, "where_in_optional", &args[2], &args[3])?;
            } else {
                self.check_pagination_args(model, "where_in_optional", &args[2], &args[3], scope)?;
            }
        } else if args.len() == 6 {
            self.check_ordering_args(model, "where_in_optional", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_in_optional", &args[4], &args[5], scope)?;
        }
        Ok(())
    }

    fn check_model_where_in_optional_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 4 && args.len() != 6 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_in_optional_page() recebe campo, array opcional de valores e limit/offset, com ordenacao opcional",
                    model
                ),
            ));
        }
        self.check_model_where_in_optional_args(
            model,
            &args[..2],
            scope,
            span,
            "where_in_optional_page",
        )?;
        if args.len() == 6 {
            self.check_ordering_args(model, "where_in_optional_page", &args[2], &args[3])?;
            self.check_pagination_args(model, "where_in_optional_page", &args[4], &args[5], scope)?;
        } else {
            self.check_pagination_args(model, "where_in_optional_page", &args[2], &args[3], scope)?;
        }
        Ok(())
    }

    fn check_model_where_compare_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 3 && args.len() != 5 && args.len() != 7 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_compare() recebe campo, operador e valor, com ordenacao e limit/offset opcionais",
                    model
                ),
            ));
        }
        self.check_model_compare_args(model, args, scope, span, "where_compare")?;
        if args.len() == 5 {
            if starts_ordering_args(&args[3..]) {
                self.check_ordering_args(model, "where_compare", &args[3], &args[4])?;
            } else {
                self.check_pagination_args(model, "where_compare", &args[3], &args[4], scope)?;
            }
        } else if args.len() == 7 {
            self.check_ordering_args(model, "where_compare", &args[3], &args[4])?;
            self.check_pagination_args(model, "where_compare", &args[5], &args[6], scope)?;
        }
        Ok(())
    }

    fn check_model_where_compare_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 5 && args.len() != 7 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_compare_page() recebe campo, operador, valor e limit/offset, com ordenacao opcional",
                    model
                ),
            ));
        }
        self.check_model_compare_args(model, args, scope, span, "where_compare_page")?;
        if args.len() == 7 {
            self.check_ordering_args(model, "where_compare_page", &args[3], &args[4])?;
            self.check_pagination_args(model, "where_compare_page", &args[5], &args[6], scope)?;
        } else {
            self.check_pagination_args(model, "where_compare_page", &args[3], &args[4], scope)?;
        }
        Ok(())
    }

    fn check_model_where_text_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 3 && args.len() != 5 && args.len() != 7 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_text() recebe campo, operador textual e valor, com ordenacao e limit/offset opcionais",
                    model
                ),
            ));
        }
        self.check_model_text_args(model, args, scope, span, "where_text")?;
        if args.len() == 5 {
            if starts_ordering_args(&args[3..]) {
                self.check_ordering_args(model, "where_text", &args[3], &args[4])?;
            } else {
                self.check_pagination_args(model, "where_text", &args[3], &args[4], scope)?;
            }
        } else if args.len() == 7 {
            self.check_ordering_args(model, "where_text", &args[3], &args[4])?;
            self.check_pagination_args(model, "where_text", &args[5], &args[6], scope)?;
        }
        Ok(())
    }

    fn check_model_where_text_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 5 && args.len() != 7 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_text_page() recebe campo, operador textual, valor e limit/offset, com ordenacao opcional",
                    model
                ),
            ));
        }
        self.check_model_text_args(model, args, scope, span, "where_text_page")?;
        if args.len() == 7 {
            self.check_ordering_args(model, "where_text_page", &args[3], &args[4])?;
            self.check_pagination_args(model, "where_text_page", &args[5], &args[6], scope)?;
        } else {
            self.check_pagination_args(model, "where_text_page", &args[3], &args[4], scope)?;
        }
        Ok(())
    }

    fn check_model_where_between_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 3 && args.len() != 5 && args.len() != 7 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_between() recebe campo, min e max, com ordenacao e limit/offset opcionais",
                    model
                ),
            ));
        }
        self.check_model_range_args(model, args, scope, span, "where_between")?;
        if args.len() == 5 {
            if starts_ordering_args(&args[3..]) {
                self.check_ordering_args(model, "where_between", &args[3], &args[4])?;
            } else {
                self.check_pagination_args(model, "where_between", &args[3], &args[4], scope)?;
            }
        } else if args.len() == 7 {
            self.check_ordering_args(model, "where_between", &args[3], &args[4])?;
            self.check_pagination_args(model, "where_between", &args[5], &args[6], scope)?;
        }
        Ok(())
    }

    fn check_model_where_between_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        if args.len() != 5 && args.len() != 7 {
            return Err(self.error(
                span,
                format!(
                    "{}::where_between_page() recebe campo, min, max e limit/offset, com ordenacao opcional",
                    model
                ),
            ));
        }
        self.check_model_range_args(model, args, scope, span, "where_between_page")?;
        if args.len() == 7 {
            self.check_ordering_args(model, "where_between_page", &args[3], &args[4])?;
            self.check_pagination_args(model, "where_between_page", &args[5], &args[6], scope)?;
        } else {
            self.check_pagination_args(model, "where_between_page", &args[3], &args[4], scope)?;
        }
        Ok(())
    }

    fn check_model_where_all_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        let Some(filter_arg_count) = where_all_filter_arg_count(args) else {
            return Err(self.error(
                span,
                format!(
                    "{}::where_all() recebe ao menos dois pares campo/valor",
                    model
                ),
            ));
        };

        for pair in args[..filter_arg_count].chunks(2) {
            self.check_model_lookup_args(model, pair, scope, span, "where_all")?;
        }

        if where_all_args_have_ordering(args) {
            let order_index = args.len() - 4;
            self.check_ordering_args(
                model,
                "where_all",
                &args[order_index],
                &args[order_index + 1],
            )?;
        }
        if where_all_args_have_pagination(args) {
            let limit_index = args.len() - 2;
            self.check_pagination_args(
                model,
                "where_all",
                &args[limit_index],
                &args[limit_index + 1],
                scope,
            )?;
        }

        Ok(())
    }

    fn check_model_where_all_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        let Some(filter_arg_count) = where_all_page_filter_arg_count(args) else {
            return Err(self.error(
                span,
                format!(
                    "{}::where_all_page() recebe ao menos dois pares campo/valor e limit/offset",
                    model
                ),
            ));
        };

        for pair in args[..filter_arg_count].chunks(2) {
            self.check_model_lookup_args(model, pair, scope, span, "where_all_page")?;
        }

        if where_all_args_have_ordering(args) {
            let order_index = args.len() - 4;
            self.check_ordering_args(
                model,
                "where_all_page",
                &args[order_index],
                &args[order_index + 1],
            )?;
        }
        let limit_index = args.len() - 2;
        self.check_pagination_args(
            model,
            "where_all_page",
            &args[limit_index],
            &args[limit_index + 1],
            scope,
        )?;

        Ok(())
    }

    fn check_model_where_any_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        let Some(filter_arg_count) = where_all_filter_arg_count(args) else {
            return Err(self.error(
                span,
                format!(
                    "{}::where_any() recebe ao menos dois pares campo/valor",
                    model
                ),
            ));
        };

        for pair in args[..filter_arg_count].chunks(2) {
            self.check_model_lookup_args(model, pair, scope, span, "where_any")?;
        }

        if where_all_args_have_ordering(args) {
            let order_index = args.len() - 4;
            self.check_ordering_args(
                model,
                "where_any",
                &args[order_index],
                &args[order_index + 1],
            )?;
        }
        if where_all_args_have_pagination(args) {
            let limit_index = args.len() - 2;
            self.check_pagination_args(
                model,
                "where_any",
                &args[limit_index],
                &args[limit_index + 1],
                scope,
            )?;
        }

        Ok(())
    }

    fn check_model_where_any_page_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<()> {
        let Some(filter_arg_count) = where_all_page_filter_arg_count(args) else {
            return Err(self.error(
                span,
                format!(
                    "{}::where_any_page() recebe ao menos dois pares campo/valor e limit/offset",
                    model
                ),
            ));
        };

        for pair in args[..filter_arg_count].chunks(2) {
            self.check_model_lookup_args(model, pair, scope, span, "where_any_page")?;
        }

        if where_all_args_have_ordering(args) {
            let order_index = args.len() - 4;
            self.check_ordering_args(
                model,
                "where_any_page",
                &args[order_index],
                &args[order_index + 1],
            )?;
        }
        let limit_index = args.len() - 2;
        self.check_pagination_args(
            model,
            "where_any_page",
            &args[limit_index],
            &args[limit_index + 1],
            scope,
        )?;

        Ok(())
    }

    fn check_model_lookup_call(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        if args.len() != 2 {
            return Err(self.error(
                span,
                format!("{}::{}() recebe campo e valor", model, method),
            ));
        }
        self.check_model_lookup_args(model, args, scope, span, method)
    }

    fn check_model_lookup_args(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let fields = self
            .models
            .get(model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;
        let Expr::StringLit { value: field, .. } = &args[0] else {
            return Err(self.error(
                args[0].span(),
                format!(
                    "{}::{}() espera nome de campo como string literal",
                    model, method
                ),
            ));
        };
        let Some(model_field) = fields.iter().find(|candidate| candidate.name == *field) else {
            return Err(self.error(
                args[0].span(),
                format!("Campo '{}.{}' nao existe", model, field),
            ));
        };

        let actual = self.infer_expr(&args[1], scope)?;
        ensure_assignable(&model_field.ty, &actual).map_err(|e| {
            self.error(
                args[1].span(),
                format!(
                    "{}::{}() valor invalido para '{}': {}",
                    model, method, field, e
                ),
            )
        })
    }

    fn check_model_optional_lookup_args(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let fields = self
            .models
            .get(model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;
        let Expr::StringLit { value: field, .. } = &args[0] else {
            return Err(self.error(
                args[0].span(),
                format!(
                    "{}::{}() espera nome de campo como string literal",
                    model, method
                ),
            ));
        };
        let Some(model_field) = fields.iter().find(|candidate| candidate.name == *field) else {
            return Err(self.error(
                args[0].span(),
                format!("Campo '{}.{}' nao existe", model, field),
            ));
        };

        let actual = self.infer_expr(&args[1], scope)?;
        let Type::Optional(inner) = &actual else {
            return Err(self.error(
                args[1].span(),
                format!(
                    "{}::{}() valor para '{}' deve ser opcional, encontrado {}",
                    model,
                    method,
                    field,
                    type_name(&actual)
                ),
            ));
        };
        ensure_assignable(&model_field.ty, inner).map_err(|e| {
            self.error(
                args[1].span(),
                format!(
                    "{}::{}() valor invalido para '{}': {}",
                    model, method, field, e
                ),
            )
        })
    }

    fn check_model_where_in_args(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let fields = self
            .models
            .get(model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;
        let Expr::StringLit { value: field, .. } = &args[0] else {
            return Err(self.error(
                args[0].span(),
                format!(
                    "{}::{}() espera nome de campo como string literal",
                    model, method
                ),
            ));
        };
        let Some(model_field) = fields.iter().find(|candidate| candidate.name == *field) else {
            return Err(self.error(
                args[0].span(),
                format!("Campo '{}.{}' nao existe", model, field),
            ));
        };

        let actual = self.infer_expr(&args[1], scope)?;
        let Type::Array(item_ty) = &actual else {
            return Err(self.error(
                args[1].span(),
                format!(
                    "{}::{}() valores para '{}' devem ser array, encontrado {}",
                    model,
                    method,
                    field,
                    type_name(&actual)
                ),
            ));
        };
        if matches!(item_ty.as_ref(), Type::Optional(_) | Type::Nil) {
            return Err(self.error(
                args[1].span(),
                format!(
                    "{}::{}() itens para '{}' devem ser valores concretos",
                    model, method, field
                ),
            ));
        }
        ensure_assignable(&model_field.ty, item_ty).map_err(|e| {
            self.error(
                args[1].span(),
                format!(
                    "{}::{}() item invalido para '{}': {}",
                    model, method, field, e
                ),
            )
        })
    }

    fn check_model_where_in_optional_args(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let fields = self
            .models
            .get(model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;
        let Expr::StringLit { value: field, .. } = &args[0] else {
            return Err(self.error(
                args[0].span(),
                format!(
                    "{}::{}() espera nome de campo como string literal",
                    model, method
                ),
            ));
        };
        let Some(model_field) = fields.iter().find(|candidate| candidate.name == *field) else {
            return Err(self.error(
                args[0].span(),
                format!("Campo '{}.{}' nao existe", model, field),
            ));
        };

        let actual = self.infer_expr(&args[1], scope)?;
        let Type::Optional(inner) = &actual else {
            return Err(self.error(
                args[1].span(),
                format!(
                    "{}::{}() valores para '{}' devem ser array opcional, encontrado {}",
                    model,
                    method,
                    field,
                    type_name(&actual)
                ),
            ));
        };
        let Type::Array(item_ty) = inner.as_ref() else {
            return Err(self.error(
                args[1].span(),
                format!(
                    "{}::{}() valores para '{}' devem ser array opcional, encontrado {}",
                    model,
                    method,
                    field,
                    type_name(&actual)
                ),
            ));
        };
        if matches!(item_ty.as_ref(), Type::Optional(_) | Type::Nil) {
            return Err(self.error(
                args[1].span(),
                format!(
                    "{}::{}() itens para '{}' devem ser valores concretos",
                    model, method, field
                ),
            ));
        }
        ensure_assignable(&model_field.ty, item_ty).map_err(|e| {
            self.error(
                args[1].span(),
                format!(
                    "{}::{}() item invalido para '{}': {}",
                    model, method, field, e
                ),
            )
        })
    }

    fn check_model_compare_args(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let fields = self
            .models
            .get(model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;
        let Expr::StringLit { value: field, .. } = &args[0] else {
            return Err(self.error(
                args[0].span(),
                format!(
                    "{}::{}() espera nome de campo como string literal",
                    model, method
                ),
            ));
        };
        let Some(model_field) = fields.iter().find(|candidate| candidate.name == *field) else {
            return Err(self.error(
                args[0].span(),
                format!("Campo '{}.{}' nao existe", model, field),
            ));
        };
        let Expr::StringLit {
            value: operator, ..
        } = &args[1]
        else {
            return Err(self.error(
                args[1].span(),
                format!(
                    "{}::{}() espera operador como string literal",
                    model, method
                ),
            ));
        };
        if !comparison_operator_supported(operator) {
            return Err(self.error(
                args[1].span(),
                format!(
                    "{}::{}() operador deve ser \"==\", \"!=\", \">\", \">=\", \"<\" ou \"<=\"",
                    model, method
                ),
            ));
        }

        let actual = self.infer_expr(&args[2], scope)?;
        ensure_assignable(&model_field.ty, &actual).map_err(|e| {
            self.error(
                args[2].span(),
                format!(
                    "{}::{}() valor invalido para '{}': {}",
                    model, method, field, e
                ),
            )
        })?;
        ensure_comparison_operator_allowed(operator, &model_field.ty, &actual)
            .map_err(|e| self.error(args[1].span(), format!("{}::{}() {}", model, method, e)))
    }

    fn check_model_text_args(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let fields = self
            .models
            .get(model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;
        let Expr::StringLit { value: field, .. } = &args[0] else {
            return Err(self.error(
                args[0].span(),
                format!(
                    "{}::{}() espera nome de campo como string literal",
                    model, method
                ),
            ));
        };
        let Some(model_field) = fields.iter().find(|candidate| candidate.name == *field) else {
            return Err(self.error(
                args[0].span(),
                format!("Campo '{}.{}' nao existe", model, field),
            ));
        };
        if !text_filter_type_supported(&model_field.ty) {
            return Err(self.error(
                args[0].span(),
                format!(
                    "{}::{}() campo '{}' deve ser string ou string?",
                    model, method, field
                ),
            ));
        }

        let Expr::StringLit {
            value: operator, ..
        } = &args[1]
        else {
            return Err(self.error(
                args[1].span(),
                format!(
                    "{}::{}() espera operador textual como string literal",
                    model, method
                ),
            ));
        };
        if !text_operator_supported(operator) {
            return Err(self.error(
                args[1].span(),
                format!(
                    "{}::{}() operador textual deve ser \"contains\", \"starts_with\", \"ends_with\", \"icontains\", \"istarts_with\" ou \"iends_with\"",
                    model, method
                ),
            ));
        }

        let actual = self.infer_expr(&args[2], scope)?;
        if text_filter_type_supported(&actual) {
            Ok(())
        } else {
            Err(self.error(
                args[2].span(),
                format!(
                    "{}::{}() valor para '{}' deve ser string ou string?, encontrado {}",
                    model,
                    method,
                    field,
                    type_name(&actual)
                ),
            ))
        }
    }

    fn check_model_range_args(
        &self,
        model: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let fields = self
            .models
            .get(model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;
        let Expr::StringLit { value: field, .. } = &args[0] else {
            return Err(self.error(
                args[0].span(),
                format!(
                    "{}::{}() espera nome de campo como string literal",
                    model, method
                ),
            ));
        };
        let Some(model_field) = fields.iter().find(|candidate| candidate.name == *field) else {
            return Err(self.error(
                args[0].span(),
                format!("Campo '{}.{}' nao existe", model, field),
            ));
        };
        if !comparison_order_type_supported(&model_field.ty) {
            return Err(self.error(
                args[0].span(),
                format!(
                    "{}::{}() campo '{}' deve ser ordenavel para range",
                    model, method, field
                ),
            ));
        }

        for (label, arg) in [("min", &args[1]), ("max", &args[2])] {
            let actual = self.infer_expr(arg, scope)?;
            if matches!(actual, Type::Nil | Type::Optional(_)) {
                return Err(self.error(
                    arg.span(),
                    format!(
                        "{}::{}() {} deve ser valor concreto, encontrado {}",
                        model,
                        method,
                        label,
                        type_name(&actual)
                    ),
                ));
            }
            ensure_assignable(&model_field.ty, &actual).map_err(|e| {
                self.error(
                    arg.span(),
                    format!(
                        "{}::{}() {} invalido para '{}': {}",
                        model, method, label, field, e
                    ),
                )
            })?;
        }

        Ok(())
    }

    fn check_ordering_args(
        &self,
        model: &str,
        method: &str,
        field_arg: &Expr,
        direction_arg: &Expr,
    ) -> CheckResult<()> {
        let fields = self.models.get(model).ok_or_else(|| {
            self.error(
                field_arg.span(),
                format!("Model '{}' nao encontrado", model),
            )
        })?;
        let Expr::StringLit { value: field, .. } = field_arg else {
            return Err(self.error(
                field_arg.span(),
                format!(
                    "{}::{}() espera campo de ordenacao como string literal",
                    model, method
                ),
            ));
        };
        let Some(model_field) = fields.iter().find(|candidate| candidate.name == *field) else {
            return Err(self.error(
                field_arg.span(),
                format!("Campo '{}.{}' nao existe", model, field),
            ));
        };
        if !ordering_type_supported(&model_field.ty) {
            return Err(self.error(
                field_arg.span(),
                format!(
                    "Campo '{}.{}' nao pode ser usado para ordenacao",
                    model, field
                ),
            ));
        }

        let Expr::StringLit {
            value: direction, ..
        } = direction_arg
        else {
            return Err(self.error(
                direction_arg.span(),
                format!(
                    "{}::{}() espera direcao de ordenacao como string literal",
                    model, method
                ),
            ));
        };
        if direction != "asc" && direction != "desc" {
            return Err(self.error(
                direction_arg.span(),
                format!(
                    "{}::{}() direcao de ordenacao deve ser \"asc\" ou \"desc\"",
                    model, method
                ),
            ));
        }

        Ok(())
    }

    fn check_pagination_args(
        &self,
        model: &str,
        method: &str,
        limit: &Expr,
        offset: &Expr,
        scope: &Scope,
    ) -> CheckResult<()> {
        let limit_ty = self.infer_expr(limit, scope)?;
        ensure_assignable(&Type::Int, &limit_ty).map_err(|e| {
            self.error(
                limit.span(),
                format!("{}::{}() limit invalido: {}", model, method, e),
            )
        })?;
        if let Expr::Integer { value, span } = limit {
            if *value <= 0 {
                return Err(self.error(
                    *span,
                    format!("{}::{}() limit deve ser maior que zero", model, method),
                ));
            }
        }

        let offset_ty = self.infer_expr(offset, scope)?;
        ensure_assignable(&Type::Int, &offset_ty).map_err(|e| {
            self.error(
                offset.span(),
                format!("{}::{}() offset invalido: {}", model, method, e),
            )
        })?;
        if let Expr::Integer { value, span } = offset {
            if *value < 0 {
                return Err(self.error(
                    *span,
                    format!("{}::{}() offset nao pode ser negativo", model, method),
                ));
            }
        }

        Ok(())
    }

    fn ensure_route_expr(
        &self,
        expr: &Expr,
        scope: &Scope,
        route_method: &HttpMethod,
    ) -> CheckResult<()> {
        match expr {
            Expr::Integer { .. }
            | Expr::Float { .. }
            | Expr::StringLit { .. }
            | Expr::Bool { .. }
            | Expr::Money { .. }
            | Expr::Nil { .. } => Ok(()),
            Expr::Ident { name, span } => {
                if scope.get(name).is_some() {
                    Ok(())
                } else {
                    Err(self.error(*span, format!("Parametro '{}' nao definido na route", name)))
                }
            }
            Expr::Array { items, .. } => {
                for item in items {
                    self.ensure_route_expr(item, scope, route_method)?;
                }
                Ok(())
            }
            Expr::Object { fields, .. } => {
                for field in fields {
                    self.ensure_route_expr(&field.value, scope, route_method)?;
                }
                Ok(())
            }
            Expr::FieldAccess { object, .. } => {
                self.ensure_route_expr(object, scope, route_method)?;
                self.infer_expr(expr, scope)?;
                Ok(())
            }
            Expr::BinOp {
                left,
                op,
                right,
                span,
            } => {
                if !matches!(op, BinOp::Add) {
                    return Err(self.error(
                        *span,
                        "Route HTTP nesta fase so suporta operador '+' no return",
                    ));
                }
                self.ensure_route_expr(left, scope, route_method)?;
                self.ensure_route_expr(right, scope, route_method)
            }
            Expr::UnaryOp { span, .. } => Err(self.error(
                *span,
                "Route HTTP nesta fase nao suporta operador unario no return",
            )),
            Expr::Call { name, args, span } if name == "str" => {
                if args.len() != 1 {
                    return Err(self.error(*span, "str() recebe exatamente 1 argumento"));
                }
                self.ensure_route_expr(&args[0], scope, route_method)
            }
            Expr::Call { name, span, .. } => Err(self.error(
                *span,
                format!("Route HTTP nesta fase nao suporta chamada '{}()'", name),
            )),
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if ty == "Auth" => {
                match method.as_str() {
                    "register" => {
                        if !matches!(route_method, HttpMethod::Post) {
                            return Err(self
                                .error(*span, "Auth::register() so pode ser usado em route POST"));
                        }
                        self.check_auth_config_arg(method, args, *span)?;
                    }
                    "login" => {
                        if !matches!(route_method, HttpMethod::Post) {
                            return Err(
                                self.error(*span, "Auth::login() so pode ser usado em route POST")
                            );
                        }
                        self.check_auth_config_arg(method, args, *span)?;
                    }
                    "logout" => {
                        if !matches!(route_method, HttpMethod::Post) {
                            return Err(
                                self.error(*span, "Auth::logout() so pode ser usado em route POST")
                            );
                        }
                        if !args.is_empty() {
                            return Err(self.error(*span, "Auth::logout() nao recebe argumentos"));
                        }
                    }
                    "user" => {
                        if !matches!(route_method, HttpMethod::Get) {
                            return Err(
                                self.error(*span, "Auth::user() so pode ser usado em route GET")
                            );
                        }
                        if !args.is_empty() {
                            return Err(self.error(*span, "Auth::user() nao recebe argumentos"));
                        }
                    }
                    _ => {
                        return Err(self.error(
                            *span,
                            format!("Metodo estatico 'Auth::{}' nao existe", method),
                        ))
                    }
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "all" => {
                if !args.is_empty() && !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::all(limit, offset) so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_all_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_not" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_not() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_not_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_not_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_not_page() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_not_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_not_in" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_not_in() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_not_in_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_not_in_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_not_in_page() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_not_in_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_not_in_optional" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!(
                            "{}::where_not_in_optional() so pode ser usado em route GET",
                            ty
                        ),
                    ));
                }
                self.check_model_where_not_in_optional_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_not_in_optional_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!(
                            "{}::where_not_in_optional_page() so pode ser usado em route GET",
                            ty
                        ),
                    ));
                }
                self.check_model_where_not_in_optional_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_any" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_any() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_any_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_any_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_any_page() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_any_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_in_optional_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!(
                            "{}::where_in_optional_page() so pode ser usado em route GET",
                            ty
                        ),
                    ));
                }
                self.check_model_where_in_optional_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_in_optional" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_in_optional() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_in_optional_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_in_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_in_page() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_in_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::page() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "create" => {
                if !matches!(route_method, HttpMethod::Post) {
                    return Err(self.error(
                        *span,
                        format!("{}::create() so pode ser usado em route POST", ty),
                    ));
                }
                if !self.models.contains_key(ty) {
                    return Err(self.error(*span, format!("Model '{}' nao encontrado", ty)));
                }
                if !args.is_empty() {
                    return Err(
                        self.error(*span, format!("{}::create() nao recebe argumentos", ty))
                    );
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_page() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "find" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::find() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_lookup_call(ty, args, scope, *span, "find")?;
                self.ensure_route_expr(&args[1], scope, route_method)
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_optional" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_optional() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_optional_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_optional_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!(
                            "{}::where_optional_page() so pode ser usado em route GET",
                            ty
                        ),
                    ));
                }
                self.check_model_where_optional_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_in" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_in() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_in_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_compare" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_compare() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_compare_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_compare_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!(
                            "{}::where_compare_page() so pode ser usado em route GET",
                            ty
                        ),
                    ));
                }
                self.check_model_where_compare_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_text" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_text() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_text_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_text_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_text_page() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_text_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_between" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_between() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_between_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_between_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!(
                            "{}::where_between_page() so pode ser usado em route GET",
                            ty
                        ),
                    ));
                }
                self.check_model_where_between_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_all" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_all() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_all_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "where_all_page" => {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(self.error(
                        *span,
                        format!("{}::where_all_page() so pode ser usado em route GET", ty),
                    ));
                }
                self.check_model_where_all_page_call(ty, args, scope, *span)?;
                for arg in args {
                    self.ensure_route_expr(arg, scope, route_method)?;
                }
                Ok(())
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "update" => {
                if !matches!(route_method, HttpMethod::Put) {
                    return Err(self.error(
                        *span,
                        format!("{}::update() so pode ser usado em route PUT", ty),
                    ));
                }
                self.check_model_lookup_call(ty, args, scope, *span, "update")?;
                self.ensure_route_expr(&args[1], scope, route_method)
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } if method == "delete" => {
                if !matches!(route_method, HttpMethod::Delete) {
                    return Err(self.error(
                        *span,
                        format!("{}::delete() so pode ser usado em route DELETE", ty),
                    ));
                }
                self.check_model_lookup_call(ty, args, scope, *span, "delete")?;
                self.ensure_route_expr(&args[1], scope, route_method)
            }
            Expr::StaticCall { .. } => {
                self.infer_expr(expr, scope)?;
                Ok(())
            }
        }
    }

    fn ensure_route_return_type(&self, path: &str, ty: &Type, span: Span) -> CheckResult<()> {
        match ty {
            Type::String
            | Type::Int
            | Type::Float
            | Type::Bool
            | Type::Money
            | Type::Array(_)
            | Type::Model(_) => Ok(()),
            Type::Optional(inner) => self.ensure_route_return_type(path, inner, span),
            Type::Void | Type::Unknown | Type::Nil => Err(self.error(
                span,
                format!(
                    "Route '{}' deve retornar valor HTTP concreto, encontrado {}",
                    path,
                    type_name(ty)
                ),
            )),
            Type::Date => Err(self.error(
                span,
                format!(
                    "Route '{}' nao pode retornar {} diretamente nesta fase",
                    path,
                    type_name(ty)
                ),
            )),
        }
    }

    fn check_invoice_field(&self, key: &str, actual: &Type, span: Span) -> CheckResult<()> {
        match key {
            "customer" | "service" | "currency" => ensure_assignable(&Type::String, actual)
                .map_err(|e| self.error(span, format!("Invoice field '{}' inválido: {}", key, e))),
            "discount" | "total" => ensure_assignable(&Type::Money, actual)
                .map_err(|e| self.error(span, format!("Invoice field '{}' inválido: {}", key, e))),
            "tax" => {
                if matches!(actual, Type::Int | Type::Float | Type::Unknown) {
                    Ok(())
                } else {
                    Err(self.error(
                        span,
                        format!(
                            "Invoice field 'tax' espera int ou float, encontrado {}",
                            type_name(actual)
                        ),
                    ))
                }
            }
            _ => Ok(()),
        }
    }
}

fn block_guarantees_return(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_guarantees_return)
}

fn stmt_guarantees_return(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return { .. } => true,
        Stmt::If {
            then_body,
            else_body: Some(else_body),
            ..
        } => block_guarantees_return(then_body) && block_guarantees_return(else_body),
        _ => false,
    }
}

fn ensure_assignable(expected: &Type, actual: &Type) -> Result<(), String> {
    if *expected == Type::Unknown || *actual == Type::Unknown || expected == actual {
        return Ok(());
    }

    match (expected, actual) {
        (Type::Optional(_), Type::Nil) => Ok(()),
        (Type::Optional(expected_inner), Type::Optional(actual_inner)) => {
            ensure_assignable(expected_inner, actual_inner)
        }
        (Type::Optional(inner), actual) => ensure_assignable(inner, actual),
        (Type::Float, Type::Int) => Ok(()),
        (Type::Array(a), Type::Array(b)) => ensure_assignable(a, b),
        _ => Err(format!(
            "esperado {}, encontrado {}",
            type_name(expected),
            type_name(actual)
        )),
    }
}

fn ensure_query_default_assignable(expected: &Type, actual: &Type) -> Result<(), String> {
    if matches!((expected, actual), (Type::Date, Type::String)) {
        return Ok(());
    }

    match (expected, actual) {
        (Type::Optional(_), Type::Nil) => Ok(()),
        (Type::Optional(inner), actual) => ensure_query_default_assignable(inner, actual),
        _ => ensure_assignable(expected, actual),
    }
}

fn is_optional_type(ty: &Type) -> bool {
    matches!(ty, Type::Optional(_))
}

fn is_optional_or_nil_type(ty: &Type) -> bool {
    matches!(ty, Type::Optional(_) | Type::Nil)
}

fn reserved_openapi_component_name(name: &str) -> bool {
    name == "NexusError" || name.starts_with("NexusPage_") || name.starts_with("NexusList_")
}

fn route_method_name(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Delete => "DELETE",
    }
}

fn unique_constraint_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => unique_constraint_type_supported(inner),
        Type::String | Type::Int | Type::Float | Type::Bool | Type::Money | Type::Date => true,
        _ => false,
    }
}

fn index_constraint_type_supported(ty: &Type) -> bool {
    unique_constraint_type_supported(ty)
}

fn min_max_constraint_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => min_max_constraint_type_supported(inner),
        Type::String | Type::Int | Type::Float | Type::Money | Type::Date => true,
        _ => false,
    }
}

fn ensure_min_max_bound_assignable(
    field_ty: &Type,
    constraint: &str,
    actual: &Type,
) -> Result<(), String> {
    match field_ty {
        Type::Optional(inner) => ensure_min_max_bound_assignable(inner, constraint, actual),
        Type::String => {
            if matches!(actual, Type::Int) {
                Ok(())
            } else {
                Err(format!(
                    "{} em string espera int para tamanho, encontrado {}",
                    constraint,
                    type_name(actual)
                ))
            }
        }
        Type::Date => {
            if matches!(actual, Type::String) {
                Ok(())
            } else {
                Err(format!(
                    "{} em date espera string ISO, encontrado {}",
                    constraint,
                    type_name(actual)
                ))
            }
        }
        Type::Int => ensure_assignable(&Type::Int, actual)
            .map_err(|e| format!("{} invalido: {}", constraint, e)),
        Type::Float => ensure_assignable(&Type::Float, actual)
            .map_err(|e| format!("{} invalido: {}", constraint, e)),
        Type::Money => ensure_assignable(&Type::Money, actual)
            .map_err(|e| format!("{} invalido: {}", constraint, e)),
        _ => Err(format!(
            "{} nao suportado para {}",
            constraint,
            type_name(field_ty)
        )),
    }
}

fn validate_min_max_bound_literal(
    field_ty: &Type,
    constraint: &str,
    expr: &Expr,
) -> Result<(), String> {
    match field_ty {
        Type::Optional(inner) => validate_min_max_bound_literal(inner, constraint, expr),
        Type::String => {
            let Some(value) = integer_literal_value(expr) else {
                return Ok(());
            };
            if value < 0 {
                Err(format!("{} em string nao pode ser negativo", constraint))
            } else {
                Ok(())
            }
        }
        Type::Int | Type::Float | Type::Money | Type::Date => Ok(()),
        _ => Ok(()),
    }
}

fn validate_default_against_min_max(
    field_ty: &Type,
    default: &Expr,
    min: &Option<Expr>,
    max: &Option<Expr>,
) -> Result<(), String> {
    if matches!(default, Expr::Nil { .. }) {
        return Ok(());
    }

    if let Some(min) = min {
        validate_default_min_max_bound(field_ty, default, "min", min)?;
    }
    if let Some(max) = max {
        validate_default_min_max_bound(field_ty, default, "max", max)?;
    }

    Ok(())
}

fn validate_default_min_max_bound(
    field_ty: &Type,
    default: &Expr,
    constraint: &str,
    bound: &Expr,
) -> Result<(), String> {
    let op = if constraint == "min" { ">=" } else { "<=" };
    match field_ty {
        Type::Optional(inner) => validate_default_min_max_bound(inner, default, constraint, bound),
        Type::String => {
            let Some(value) = string_literal_value(default) else {
                return Ok(());
            };
            let Some(limit) = integer_literal_value(bound) else {
                return Ok(());
            };
            let length = value.chars().count() as i64;
            if (constraint == "min" && length >= limit) || (constraint == "max" && length <= limit)
            {
                Ok(())
            } else {
                Err(format!(
                    "default viola {}: tamanho deve ser {} {}",
                    constraint, op, limit
                ))
            }
        }
        Type::Int | Type::Float => {
            let Some(value) = numeric_literal_value(default) else {
                return Ok(());
            };
            let Some(limit) = numeric_literal_value(bound) else {
                return Ok(());
            };
            if (constraint == "min" && value >= limit) || (constraint == "max" && value <= limit) {
                Ok(())
            } else {
                Err(format!(
                    "default viola {}: valor deve ser {} {}",
                    constraint,
                    op,
                    format_number_for_check(limit)
                ))
            }
        }
        Type::Money => {
            let Some((value_amount, value_currency)) = money_literal_value(default) else {
                return Ok(());
            };
            let Some((limit_amount, limit_currency)) = money_literal_value(bound) else {
                return Ok(());
            };
            if value_currency != limit_currency {
                return Err(format!(
                    "default usa moeda {}, mas {} usa {}",
                    value_currency, constraint, limit_currency
                ));
            }
            if (constraint == "min" && value_amount >= limit_amount)
                || (constraint == "max" && value_amount <= limit_amount)
            {
                Ok(())
            } else {
                Err(format!(
                    "default viola {}: valor deve ser {} {} {}",
                    constraint,
                    op,
                    format_number_for_check(limit_amount),
                    limit_currency
                ))
            }
        }
        Type::Date => {
            let Some(value) = string_literal_value(default) else {
                return Ok(());
            };
            let Some(limit) = string_literal_value(bound) else {
                return Ok(());
            };
            if (constraint == "min" && value >= limit) || (constraint == "max" && value <= limit) {
                Ok(())
            } else {
                Err(format!(
                    "default viola {}: valor deve ser {} {}",
                    constraint, op, limit
                ))
            }
        }
        _ => Ok(()),
    }
}

fn ensure_min_max_bounds_ordered(field_ty: &Type, min: &Expr, max: &Expr) -> Result<(), String> {
    match field_ty {
        Type::Optional(inner) => ensure_min_max_bounds_ordered(inner, min, max),
        Type::String => {
            let Some(min) = integer_literal_value(min) else {
                return Ok(());
            };
            let Some(max) = integer_literal_value(max) else {
                return Ok(());
            };
            if min <= max {
                Ok(())
            } else {
                Err("min nao pode ser maior que max".to_string())
            }
        }
        Type::Int | Type::Float => {
            let Some(min) = numeric_literal_value(min) else {
                return Ok(());
            };
            let Some(max) = numeric_literal_value(max) else {
                return Ok(());
            };
            if min <= max {
                Ok(())
            } else {
                Err("min nao pode ser maior que max".to_string())
            }
        }
        Type::Money => {
            let Some((min_amount, min_currency)) = money_literal_value(min) else {
                return Ok(());
            };
            let Some((max_amount, max_currency)) = money_literal_value(max) else {
                return Ok(());
            };
            if min_currency != max_currency {
                return Err("min/max money devem usar a mesma moeda".to_string());
            }
            if min_amount <= max_amount {
                Ok(())
            } else {
                Err("min nao pode ser maior que max".to_string())
            }
        }
        Type::Date => {
            let Some(min) = string_literal_value(min) else {
                return Ok(());
            };
            let Some(max) = string_literal_value(max) else {
                return Ok(());
            };
            if min <= max {
                Ok(())
            } else {
                Err("min nao pode ser maior que max".to_string())
            }
        }
        _ => Ok(()),
    }
}

fn integer_literal_value(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Integer { value, .. } => Some(*value),
        _ => None,
    }
}

fn numeric_literal_value(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Integer { value, .. } => Some(*value as f64),
        Expr::Float { value, .. } => Some(*value),
        _ => None,
    }
}

fn money_literal_value(expr: &Expr) -> Option<(f64, &str)> {
    match expr {
        Expr::Money {
            value, currency, ..
        } => Some((*value, currency.as_str())),
        _ => None,
    }
}

fn string_literal_value(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::StringLit { value, .. } => Some(value.as_str()),
        _ => None,
    }
}

fn format_number_for_check(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        value.to_string()
    }
}

fn ordering_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => ordering_type_supported(inner),
        Type::String | Type::Int | Type::Float | Type::Bool | Type::Money | Type::Date => true,
        _ => false,
    }
}

fn comparison_operator_supported(operator: &str) -> bool {
    matches!(operator, "==" | "!=" | ">" | ">=" | "<" | "<=")
}

fn ensure_comparison_operator_allowed(
    operator: &str,
    field_ty: &Type,
    actual_ty: &Type,
) -> Result<(), String> {
    if matches!(operator, "==" | "!=") {
        if comparison_equality_type_supported(field_ty) {
            return Ok(());
        }
    } else {
        if matches!(actual_ty, Type::Nil) {
            return Err(format!("operador '{}' nao aceita nil", operator));
        }
        if comparison_order_type_supported(field_ty) {
            return Ok(());
        }
    }

    Err(format!(
        "campo do tipo {} nao suporta operador '{}'",
        type_name(field_ty),
        operator
    ))
}

fn comparison_equality_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => comparison_equality_type_supported(inner),
        Type::String | Type::Int | Type::Float | Type::Bool | Type::Money | Type::Date => true,
        _ => false,
    }
}

fn comparison_order_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => comparison_order_type_supported(inner),
        Type::String | Type::Int | Type::Float | Type::Money | Type::Date => true,
        _ => false,
    }
}

fn text_operator_supported(operator: &str) -> bool {
    matches!(
        operator,
        "contains" | "starts_with" | "ends_with" | "icontains" | "istarts_with" | "iends_with"
    )
}

fn text_filter_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => text_filter_type_supported(inner),
        Type::String => true,
        _ => false,
    }
}

fn query_param_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => query_param_type_supported(inner),
        Type::Array(inner) => query_param_array_item_type_supported(inner),
        Type::String | Type::Int | Type::Float | Type::Bool | Type::Money | Type::Date => true,
        _ => false,
    }
}

fn query_param_array_item_type_supported(ty: &Type) -> bool {
    matches!(
        ty,
        Type::String | Type::Int | Type::Float | Type::Bool | Type::Money | Type::Date
    )
}

fn where_all_filter_arg_count(args: &[Expr]) -> Option<usize> {
    if args.len() < 4 {
        return None;
    }
    let filter_arg_count = if where_all_args_have_ordering(args) {
        args.len() - 4
    } else if where_all_args_have_pagination(args) {
        args.len() - 2
    } else {
        args.len()
    };
    if filter_arg_count >= 4 && filter_arg_count % 2 == 0 {
        Some(filter_arg_count)
    } else {
        None
    }
}

fn where_all_page_filter_arg_count(args: &[Expr]) -> Option<usize> {
    if args.len() < 6 || !where_all_args_have_pagination(args) {
        return None;
    }
    let filter_arg_count = if where_all_args_have_ordering(args) {
        args.len() - 4
    } else {
        args.len() - 2
    };
    if filter_arg_count >= 4 && filter_arg_count % 2 == 0 {
        Some(filter_arg_count)
    } else {
        None
    }
}

fn where_all_args_have_pagination(args: &[Expr]) -> bool {
    args.len() >= 6 && !expr_is_string_lit(&args[args.len() - 2])
}

fn where_all_args_have_ordering(args: &[Expr]) -> bool {
    args.len() >= 8
        && expr_is_string_lit(&args[args.len() - 4])
        && expr_is_order_direction_lit(&args[args.len() - 3])
        && !expr_is_string_lit(&args[args.len() - 2])
}

fn starts_ordering_args(args: &[Expr]) -> bool {
    args.first().is_some_and(expr_is_string_lit)
}

fn expr_is_string_lit(expr: &Expr) -> bool {
    matches!(expr, Expr::StringLit { .. })
}

fn expr_is_order_direction_lit(expr: &Expr) -> bool {
    matches!(expr, Expr::StringLit { value, .. } if value == "asc" || value == "desc")
}

fn ensure_comparable(left: &Type, right: &Type) -> Result<(), String> {
    ensure_assignable(left, right)?;
    match left {
        Type::Int | Type::Float | Type::Money | Type::String | Type::Unknown => Ok(()),
        _ => Err(format!("{} não é comparável", type_name(left))),
    }
}

fn numeric_result(op: &BinOp, left: &Type, right: &Type) -> Result<Type, String> {
    if is_optional_or_nil_type(left) || is_optional_or_nil_type(right) {
        return Err(format!(
            "operacao com opcional invalida: {} e {}",
            type_name(left),
            type_name(right)
        ));
    }

    match op {
        BinOp::Add if *left == Type::String || *right == Type::String => Ok(Type::String),
        BinOp::Add | BinOp::Sub => match (left, right) {
            (Type::Money, Type::Money) => Ok(Type::Money),
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::Int, Type::Float) | (Type::Float, Type::Int) | (Type::Float, Type::Float) => {
                Ok(Type::Float)
            }
            _ => Err(format!(
                "operação numérica inválida: {} e {}",
                type_name(left),
                type_name(right)
            )),
        },
        BinOp::Mul => match (left, right) {
            (Type::Money, Type::Int)
            | (Type::Money, Type::Float)
            | (Type::Int, Type::Money)
            | (Type::Float, Type::Money) => Ok(Type::Money),
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::Int, Type::Float) | (Type::Float, Type::Int) | (Type::Float, Type::Float) => {
                Ok(Type::Float)
            }
            _ => Err(format!(
                "operação numérica inválida: {} e {}",
                type_name(left),
                type_name(right)
            )),
        },
        BinOp::Div => match (left, right) {
            (Type::Money, Type::Int) | (Type::Money, Type::Float) => Ok(Type::Money),
            (Type::Int, Type::Int)
            | (Type::Int, Type::Float)
            | (Type::Float, Type::Int)
            | (Type::Float, Type::Float) => Ok(Type::Float),
            _ => Err(format!(
                "divisão inválida: {} por {}",
                type_name(left),
                type_name(right)
            )),
        },
        BinOp::Mod => match (left, right) {
            (Type::Int, Type::Int) => Ok(Type::Int),
            _ => Err("módulo apenas aceita int".to_string()),
        },
        _ => unreachable!(),
    }
}

fn type_name(ty: &Type) -> String {
    match ty {
        Type::String => "string".to_string(),
        Type::Int => "int".to_string(),
        Type::Float => "float".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Money => "money".to_string(),
        Type::Date => "date".to_string(),
        Type::Array(inner) => format!("[{}]", type_name(inner)),
        Type::Optional(inner) => format!("{}?", type_name(inner)),
        Type::Model(name) => name.clone(),
        Type::Nil => "nil".to_string(),
        Type::Void => "void".to_string(),
        Type::Unknown => "unknown".to_string(),
    }
}
