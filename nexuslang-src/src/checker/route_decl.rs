use std::collections::HashSet;

use crate::ast::{HttpMethod, QueryParam, RouteAuthGuard, Span, Stmt, Type};
use crate::hir::{self, HirDeclId, HirProgram, HirRefId, HirScopeId, HirSymbolKind};

use super::{ensure_assignable, type_name};
use super::{resolver::ResolvedProgram, CheckResult, Checker, Scope};

impl Checker {
    pub(super) fn collect_route_declaration(
        &mut self,
        method: &HttpMethod,
        path: &str,
        params: &[String],
        query_params: &[QueryParam],
        auth: &Option<RouteAuthGuard>,
        span: Span,
        route_signatures: &mut HashSet<(&'static str, String)>,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        let signature = (route_method_name(method), path.to_string());
        if !route_signatures.insert(signature) {
            return Err(self.error(
                span,
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
                    span,
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

        self.symbols.set_top_level(
            HirSymbolKind::Route,
            path,
            resolved.top_level_symbol(HirSymbolKind::Route, path),
        );

        Ok(())
    }

    pub(super) fn check_route_auth_guard(
        &self,
        path: &str,
        guard: &RouteAuthGuard,
    ) -> CheckResult<()> {
        let config = if self.auth_symbol(&guard.auth).is_some() {
            self.auths.get(&guard.auth)
        } else {
            None
        };
        let Some(config) = config else {
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

    pub(super) fn check_route(
        &self,
        hir: &HirProgram<'_>,
        method: &HttpMethod,
        path: &str,
        params: &[String],
        query_params: &[QueryParam],
        auth: &Option<RouteAuthGuard>,
        body: &[Stmt],
        span: Span,
        decl: Option<HirDeclId>,
        hir_scope: Option<HirScopeId>,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        if let Some(guard) = auth {
            self.check_route_auth_guard(path, guard)?;
            self.link_route_auth_guard_ref(hir, decl, guard);
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

        let mut scope = Scope::default().with_hir_scope(hir_scope);
        for param in params {
            let symbol = self.resolve_binding_symbol(
                &scope,
                decl,
                resolved,
                HirSymbolKind::RouteParameter,
                param,
                span,
            );
            self.produce_symbol_metadata(symbol, &Type::String);
            scope.define_with_symbol(param, Type::String, false, symbol);
        }
        for param in query_params {
            let symbol = self.resolve_binding_symbol(
                &scope,
                decl,
                resolved,
                HirSymbolKind::QueryParameter,
                &param.name,
                param.span,
            );
            self.produce_symbol_metadata(symbol, &param.ty);
            scope.define_with_symbol(&param.name, param.ty.clone(), false, symbol);
        }

        self.ensure_route_expr_with_hir(hir, value, &scope, method)?;
        let actual = self.infer_route_return_expr_with_hir(hir, value, &scope)?;
        self.ensure_route_return_type(path, &actual, *return_span)
    }

    fn link_route_auth_guard_ref(
        &self,
        hir: &HirProgram<'_>,
        decl: Option<HirDeclId>,
        guard: &RouteAuthGuard,
    ) {
        let Some(reference) = hir_route_auth_ref(hir, decl) else {
            return;
        };
        if let Some(symbol) = self.auth_symbol(&guard.auth) {
            self.link_hir_reference_symbol(reference, symbol);
            let _ = self.typed_hir_reference_symbol(reference);
        }
    }
}

fn hir_route_auth_ref(hir: &HirProgram<'_>, decl: Option<HirDeclId>) -> Option<HirRefId> {
    let decl = hir.decl(decl?)?;
    let hir::HirDeclBody::Route {
        auth: Some(guard), ..
    } = &decl.body
    else {
        return None;
    };

    Some(guard.auth)
}

fn route_method_name(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Delete => "DELETE",
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
