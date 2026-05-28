use std::collections::HashMap;

use crate::ast::{Expr, Stmt};
use crate::hir::{self, HirExprId, HirProgram, HirScopeId, HirSymbolId, HirSymbolKind};

#[derive(Debug, Default, Clone)]
pub(super) struct CheckerSymbols {
    pub(super) functions: HashMap<String, HirSymbolId>,
    pub(super) models: HashMap<String, HirSymbolId>,
    pub(super) auths: HashMap<String, HirSymbolId>,
    pub(super) workflows: HashMap<String, HirSymbolId>,
    pub(super) routes: HashMap<String, Vec<HirSymbolId>>,
    pub(super) model_fields: HashMap<(String, String), HirSymbolId>,
    pub(super) exprs: HashMap<usize, HirExprId>,
    pub(super) stmt_scopes: HashMap<usize, HirScopeId>,
    pub(super) stmt_binding_scopes: HashMap<usize, HirScopeId>,
}

impl CheckerSymbols {
    pub(super) fn index_hir(&mut self, hir: &HirProgram<'_>) {
        self.functions.clear();
        self.models.clear();
        self.auths.clear();
        self.workflows.clear();
        self.routes.clear();
        self.exprs.clear();
        self.stmt_scopes.clear();
        self.stmt_binding_scopes.clear();
        self.model_fields.clear();

        for expr in &hir.exprs {
            let key = expr.source as *const Expr as usize;
            self.exprs.insert(key, expr.id);
        }

        for decl in &hir.decls {
            match &decl.body {
                hir::HirDeclBody::Function { body, .. } | hir::HirDeclBody::Route { body, .. } => {
                    self.index_hir_stmts(hir, body)
                }
                hir::HirDeclBody::Workflow { steps } => {
                    for step in steps {
                        self.index_hir_stmts(hir, &step.body);
                    }
                }
                hir::HirDeclBody::Statement { stmt } => self.index_hir_stmt(hir, stmt),
                hir::HirDeclBody::Model { fields } => {
                    let Some(model_name) = decl.name else {
                        continue;
                    };
                    for field in fields {
                        self.model_fields.insert(
                            (model_name.to_string(), field.name.to_string()),
                            field.symbol,
                        );
                    }
                }
                hir::HirDeclBody::Auth { .. }
                | hir::HirDeclBody::Invoice { .. }
                | hir::HirDeclBody::Import { .. } => {}
            }
        }
    }

    pub(super) fn index_hir_stmts(&mut self, hir: &HirProgram<'_>, stmts: &[hir::HirStmt<'_>]) {
        for stmt in stmts {
            self.index_hir_stmt(hir, stmt);
        }
    }

    pub(super) fn index_hir_stmt(&mut self, hir: &HirProgram<'_>, stmt: &hir::HirStmt<'_>) {
        let key = stmt.source as *const Stmt as usize;
        self.stmt_scopes.insert(key, stmt.scope);

        match &stmt.kind {
            hir::HirStmtKind::Let { symbol, .. }
            | hir::HirStmtKind::Const { symbol, .. }
            | hir::HirStmtKind::For { symbol, .. } => {
                if let Some(symbol) = hir.symbol(*symbol) {
                    self.stmt_binding_scopes.insert(key, symbol.scope);
                }
            }
            _ => {}
        }

        match &stmt.kind {
            hir::HirStmtKind::If {
                then_body,
                else_body,
                ..
            } => {
                self.index_hir_stmts(hir, then_body);
                if let Some(else_body) = else_body {
                    self.index_hir_stmts(hir, else_body);
                }
            }
            hir::HirStmtKind::While { body, .. } | hir::HirStmtKind::For { body, .. } => {
                self.index_hir_stmts(hir, body);
            }
            _ => {}
        }
    }

    pub(super) fn set_top_level(
        &mut self,
        kind: HirSymbolKind,
        name: &str,
        symbol: Option<HirSymbolId>,
    ) {
        let Some(symbol) = symbol else {
            return;
        };

        match kind {
            HirSymbolKind::Function => {
                self.functions.insert(name.to_string(), symbol);
            }
            HirSymbolKind::Model => {
                self.models.insert(name.to_string(), symbol);
            }
            HirSymbolKind::Auth => {
                self.auths.insert(name.to_string(), symbol);
            }
            HirSymbolKind::Workflow => {
                self.workflows.insert(name.to_string(), symbol);
            }
            HirSymbolKind::Route => {
                self.routes
                    .entry(name.to_string())
                    .or_default()
                    .push(symbol);
            }
            _ => {}
        }
    }

    pub(super) fn expr_id(&self, expr: &Expr) -> Option<HirExprId> {
        self.exprs.get(&(expr as *const Expr as usize)).copied()
    }

    pub(super) fn stmt_scope(&self, stmt: &Stmt) -> Option<HirScopeId> {
        self.stmt_scopes
            .get(&(stmt as *const Stmt as usize))
            .copied()
    }

    pub(super) fn stmt_binding_scope(&self, stmt: &Stmt) -> Option<HirScopeId> {
        self.stmt_binding_scopes
            .get(&(stmt as *const Stmt as usize))
            .copied()
    }

    pub(super) fn model_field(&self, model: &str, field: &str) -> Option<HirSymbolId> {
        self.model_fields
            .get(&(model.to_string(), field.to_string()))
            .copied()
    }
}
