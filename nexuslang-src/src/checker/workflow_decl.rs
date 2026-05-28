use crate::ast::{Expr, Span, Type, WorkflowStep};
use crate::hir::{self, HirDeclId, HirExprId, HirProgram, HirScopeId, HirSymbolKind};

use super::resolver::ResolvedProgram;
use super::{ensure_assignable, CheckResult, Checker, Scope};

impl Checker {
    pub(super) fn collect_workflow_declaration(
        &mut self,
        name: &str,
        span: Span,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        if !self.workflows.insert(name.to_string()) {
            return Err(self.error(
                span,
                format!("Workflow '{}' declarado mais de uma vez", name),
            ));
        }

        self.symbols.set_top_level(
            HirSymbolKind::Workflow,
            name,
            resolved.top_level_symbol(HirSymbolKind::Workflow, name),
        );

        Ok(())
    }

    pub(super) fn check_workflow_declaration(
        &self,
        hir: &HirProgram<'_>,
        steps: &[WorkflowStep],
        top_scope: &Scope,
        decl: Option<HirDeclId>,
        hir_scope: Option<HirScopeId>,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        for step in steps {
            let mut scope = top_scope.clone().with_hir_scope(hir_scope);
            self.check_stmts(hir, &step.body, &mut scope, &Type::Unknown, decl, resolved)?;
        }

        Ok(())
    }

    pub(super) fn ensure_run_workflow_arg_count(
        &self,
        arg_count: usize,
        span: Span,
    ) -> CheckResult<()> {
        if arg_count != 1 {
            return Err(self.error(span, "run_workflow() recebe exatamente 1 argumento"));
        }

        Ok(())
    }

    pub(super) fn check_run_workflow_target(
        &self,
        actual: &Type,
        static_name: Option<&str>,
        span: Span,
    ) -> CheckResult<Type> {
        ensure_assignable(&Type::String, actual)
            .map_err(|e| self.error(span, format!("run_workflow() espera string: {}", e)))?;

        if let Some(name) = static_name {
            self.ensure_workflow_exists(name, span)?;
        }

        Ok(Type::Void)
    }

    pub(super) fn link_run_workflow_target_expr(&self, expr: &Expr, name: &str) {
        if let Some(symbol) = self.workflow_symbol(name) {
            self.produce_expr_metadata(expr, &Type::String, Some(symbol));
            let _ = self.typed_hir_expr_symbol(expr);
        }
    }

    pub(super) fn link_hir_run_workflow_target_expr(&self, expr: HirExprId, name: &str) {
        if let Some(symbol) = self.workflow_symbol(name) {
            self.produce_hir_expr_metadata(expr, &Type::String, Some(symbol));
            let _ = self.typed_hir_expr_symbol_by_id(expr);
        }
    }

    pub(super) fn ast_static_workflow_name<'a>(&self, expr: &'a Expr) -> Option<&'a str> {
        match expr {
            Expr::StringLit { value, .. } => Some(value.as_str()),
            _ => None,
        }
    }

    pub(super) fn hir_static_workflow_name<'a>(
        &self,
        hir: &HirProgram<'a>,
        expr: HirExprId,
    ) -> Option<&'a str> {
        match hir.expr(expr) {
            Some(hir::HirExpr {
                kind: hir::HirExprKind::String(name),
                ..
            }) => Some(name),
            _ => None,
        }
    }

    fn ensure_workflow_exists(&self, name: &str, span: Span) -> CheckResult<()> {
        if self.workflow_symbol(name).is_none() || !self.workflows.contains(name) {
            return Err(self.error(span, format!("Workflow '{}' não encontrado", name)));
        }

        Ok(())
    }
}
