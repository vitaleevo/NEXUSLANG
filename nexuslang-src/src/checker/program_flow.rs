use std::collections::HashSet;

use crate::ast::{Decl, Expr, Program, Span, Type};
use crate::hir::HirProgram;

use super::{CheckResult, Checker, Diagnostic, ResolvedProgram, Scope};

impl Checker {
    pub(super) fn collect_decls(
        &mut self,
        program: &Program,
        hir: &HirProgram<'_>,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        for (index, decl) in program.decls.iter().enumerate() {
            self.enter_diagnostic_decl_owner(index);
            // Unwrap export to check the inner declaration
            let inner = decl.exported_inner().unwrap_or(decl);
            if let Decl::Model { name, fields, .. } = inner {
                self.collect_model_declaration(name, fields, inner.span(), resolved)?;
            }
            if let Decl::Workflow { name, .. } = inner {
                self.collect_workflow_declaration(name, inner.span(), resolved)?;
            }
            if let Decl::Auth { config } = inner {
                self.collect_auth_declaration(config, resolved)?;
            }
        }

        let mut route_signatures = HashSet::new();

        for (index, decl) in program.decls.iter().enumerate() {
            self.enter_diagnostic_decl_owner(index);
            let hir_decl = hir.decls.get(index);
            // Unwrap export wrappers so the inner declaration is processed
            let (inner, inner_hir_decl) = match decl {
                Decl::Export { decl: inner, .. } => {
                    // For exported declarations, the HIR decl at this index
                    // may be the inner's HIR or None. Try to find it.
                    (inner.as_ref(), hir_decl)
                }
                other => (other, hir_decl),
            };
            match inner {
                Decl::Function {
                    name,
                    params,
                    return_type,
                    ..
                } => {
                    self.collect_function_declaration(
                        name,
                        params,
                        return_type,
                        inner.span(),
                        resolved,
                    )?;
                }
                Decl::Model { name, fields, .. } => {
                    self.check_model_declaration(name, fields)?;
                }
                Decl::Route {
                    method,
                    path,
                    params,
                    query_params,
                    auth,
                    ..
                } => {
                    self.collect_route_declaration(
                        method,
                        path,
                        params,
                        query_params,
                        auth,
                        inner.span(),
                        &mut route_signatures,
                        resolved,
                    )?;
                }
                Decl::Auth { config } => {
                    self.check_auth_declaration(config, inner_hir_decl)?;
                }
                Decl::Import { .. } | Decl::Export { .. } => {
                    // Nested export — skip (parser prevents export of export)
                }
                _ => {}
            }
        }

        self.clear_diagnostic_decl_owner();
        Ok(())
    }

    pub(super) fn ensure_static_default_expr(&self, expr: &Expr, context: &str) -> CheckResult<()> {
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

    pub(super) fn check_decls(
        &self,
        program: &Program,
        hir: &HirProgram<'_>,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        let root_scope = hir.scopes.first().map(|scope| scope.id);
        let mut top_scope = Scope::default().with_hir_scope(root_scope);

        for (index, decl) in program.decls.iter().enumerate() {
            self.enter_diagnostic_decl_owner(index);
            // Unwrap export for statement checking
            let effective = decl.exported_inner().unwrap_or(decl);
            if let Decl::Statement(stmt) = effective {
                let decl_id = hir.decls.get(index).map(|decl| decl.id);
                self.check_top_level_statement(hir, stmt, &mut top_scope, decl_id, resolved)?;
            }
        }

        for (index, decl) in program.decls.iter().enumerate() {
            self.enter_diagnostic_decl_owner(index);
            // Unwrap export: checker validates the inner declaration
            let effective = decl.exported_inner().unwrap_or(decl);
            let hir_decl = hir.decls.get(index);
            let decl_id = hir_decl.map(|decl| decl.id);
            let decl_scope = hir_decl.and_then(|decl| decl.scope);
            match effective {
                Decl::Function {
                    name,
                    params,
                    return_type,
                    body,
                    span,
                } => {
                    self.check_function_declaration(
                        hir,
                        name,
                        params,
                        return_type,
                        body,
                        *span,
                        &top_scope,
                        decl_id,
                        decl_scope,
                        resolved,
                    )?;
                }
                Decl::Route {
                    method,
                    path,
                    params,
                    query_params,
                    auth,
                    body,
                    span,
                } => self.check_route(
                    hir,
                    method,
                    path,
                    params,
                    query_params,
                    auth,
                    body,
                    *span,
                    decl_id,
                    decl_scope,
                    resolved,
                )?,
                Decl::Invoice {
                    fields,
                    items,
                    span,
                } => self.check_invoice_declaration(hir, fields, items, *span, &top_scope)?,
                Decl::Statement(_) => {}
                Decl::Auth { .. } => {}
                Decl::Workflow { steps, .. } => {
                    self.check_workflow_declaration(
                        hir, steps, &top_scope, decl_id, decl_scope, resolved,
                    )?;
                }
                Decl::Model { .. } => {}
                Decl::Import { .. } | Decl::Export { .. } => {
                    // Single-file: not checked at module level yet
                }
            }
        }

        self.clear_diagnostic_decl_owner();
        Ok(())
    }

    pub(super) fn check_decls_collecting_independent_diagnostics(
        &self,
        program: &Program,
        hir: &HirProgram<'_>,
        resolved: &ResolvedProgram<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        let root_scope = hir.scopes.first().map(|scope| scope.id);
        let mut top_scope = Scope::default().with_hir_scope(root_scope);
        let mut diagnostics = Vec::new();

        for (index, decl) in program.decls.iter().enumerate() {
            self.enter_diagnostic_decl_owner(index);
            let effective = decl.exported_inner().unwrap_or(decl);
            if let Decl::Statement(stmt) = effective {
                let decl_id = hir.decls.get(index).map(|decl| decl.id);
                if let Err(diagnostic) =
                    self.check_top_level_statement(hir, stmt, &mut top_scope, decl_id, resolved)
                {
                    self.clear_diagnostic_decl_owner();
                    return Err(vec![diagnostic]);
                }
            }
        }

        for (index, decl) in program.decls.iter().enumerate() {
            self.enter_diagnostic_decl_owner(index);
            let effective = decl.exported_inner().unwrap_or(decl);
            let hir_decl = hir.decls.get(index);
            let decl_id = hir_decl.map(|decl| decl.id);
            let decl_scope = hir_decl.and_then(|decl| decl.scope);
            let result = match effective {
                Decl::Function {
                    name,
                    params,
                    return_type,
                    body,
                    span,
                } => self.check_function_declaration(
                    hir,
                    name,
                    params,
                    return_type,
                    body,
                    *span,
                    &top_scope,
                    decl_id,
                    decl_scope,
                    resolved,
                ),
                Decl::Route {
                    method,
                    path,
                    params,
                    query_params,
                    auth,
                    body,
                    span,
                } => self.check_route(
                    hir,
                    method,
                    path,
                    params,
                    query_params,
                    auth,
                    body,
                    *span,
                    decl_id,
                    decl_scope,
                    resolved,
                ),
                Decl::Invoice {
                    fields,
                    items,
                    span,
                } => self.check_invoice_declaration(hir, fields, items, *span, &top_scope),
                Decl::Workflow { steps, .. } => self.check_workflow_declaration(
                    hir, steps, &top_scope, decl_id, decl_scope, resolved,
                ),
                Decl::Statement(_)
                | Decl::Auth { .. }
                | Decl::Model { .. }
                | Decl::Import { .. }
                | Decl::Export { .. } => Ok(()),
            };

            if let Err(diagnostic) = result {
                diagnostics.push(diagnostic);
            }
        }

        self.clear_diagnostic_decl_owner();
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    pub(super) fn ensure_known_type(&self, ty: &Type, span: Span) -> CheckResult<()> {
        match ty {
            Type::Unknown => Err(self.error(span, "tipo desconhecido")),
            Type::Nil => Err(self.error(span, "tipo nil nao pode ser usado como anotacao")),
            Type::Array(inner) => self.ensure_known_type(inner, span),
            Type::Optional(inner) => self.ensure_known_type(inner, span),
            Type::Model(name)
                if self.model_symbol(name).is_some() && self.models.contains_key(name) =>
            {
                Ok(())
            }
            Type::Model(name) => {
                Err(self.error(span, format!("Model type '{}' não encontrado", name)))
            }
            _ => Ok(()),
        }
    }
}
