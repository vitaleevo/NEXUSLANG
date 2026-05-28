use crate::ast::{Expr, Span, Stmt, Type};
use crate::hir::{HirDeclId, HirProgram, HirScopeId, HirSymbolKind};

use super::resolver::ResolvedProgram;
use super::{ensure_assignable, type_name, CheckResult, Checker, Scope};

impl Checker {
    pub(super) fn check_stmts(
        &self,
        hir: &HirProgram<'_>,
        stmts: &[Stmt],
        scope: &mut Scope,
        expected_return: &Type,
        decl: Option<HirDeclId>,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        for stmt in stmts {
            self.check_stmt(hir, stmt, scope, expected_return, decl, resolved)?;
        }
        Ok(())
    }

    pub(super) fn check_stmt(
        &self,
        hir: &HirProgram<'_>,
        stmt: &Stmt,
        scope: &mut Scope,
        expected_return: &Type,
        decl: Option<HirDeclId>,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        let previous_hir_scope = scope.hir_scope;
        if let Some(stmt_scope) = self.symbols.stmt_scope(stmt) {
            scope.hir_scope = Some(stmt_scope);
        }

        let result = (|| -> CheckResult<()> {
            match stmt {
                Stmt::Let {
                    name,
                    ty,
                    value,
                    span,
                } => {
                    let binding_scope = self.symbols.stmt_binding_scope(stmt).or(scope.hir_scope);
                    self.check_binding(
                        hir,
                        name,
                        ty,
                        value,
                        false,
                        scope,
                        *span,
                        decl,
                        binding_scope,
                        resolved,
                    )
                }
                Stmt::Const {
                    name,
                    ty,
                    value,
                    span,
                } => {
                    let binding_scope = self.symbols.stmt_binding_scope(stmt).or(scope.hir_scope);
                    self.check_binding(
                        hir,
                        name,
                        ty,
                        value,
                        true,
                        scope,
                        *span,
                        decl,
                        binding_scope,
                        resolved,
                    )
                }
                Stmt::Assign { name, value, span } => {
                    let value_ty = self.infer_expr_with_hir(hir, value, scope)?;
                    self.assign_in_scope(scope, name, &value_ty)
                        .map_err(|message| self.error(*span, message))
                }
                Stmt::Return { value, span } => {
                    let actual = self.infer_expr_with_hir(hir, value, scope)?;
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
                    self.infer_expr_with_hir(hir, value, scope)?;
                    Ok(())
                }
                Stmt::ExprStmt { expr, .. } => {
                    self.infer_expr_with_hir(hir, expr, scope)?;
                    Ok(())
                }
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                    span,
                } => {
                    let cond_ty = self.infer_expr_with_hir(hir, condition, scope)?;
                    ensure_assignable(&Type::Bool, &cond_ty).map_err(|e| {
                        self.error(*span, format!("Condição de if inválida: {}", e))
                    })?;
                    self.check_stmts(hir, then_body, scope, expected_return, decl, resolved)?;
                    if let Some(stmts) = else_body {
                        self.check_stmts(hir, stmts, scope, expected_return, decl, resolved)?;
                    }
                    Ok(())
                }
                Stmt::While {
                    condition,
                    body,
                    span,
                } => {
                    let cond_ty = self.infer_expr_with_hir(hir, condition, scope)?;
                    ensure_assignable(&Type::Bool, &cond_ty).map_err(|e| {
                        self.error(*span, format!("Condição de while inválida: {}", e))
                    })?;
                    self.check_stmts(hir, body, scope, expected_return, decl, resolved)
                }
                Stmt::For {
                    var,
                    iterable,
                    body,
                    span,
                } => {
                    let iter_ty = self.infer_expr_with_hir(hir, iterable, scope)?;
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
                    let binding_scope = self.symbols.stmt_binding_scope(stmt).or(scope.hir_scope);
                    let symbol = self.resolve_binding_symbol_in_hir_scope(
                        binding_scope,
                        decl,
                        resolved,
                        HirSymbolKind::ForBinding,
                        var,
                        *span,
                    );
                    self.produce_symbol_metadata(symbol, &item_ty);
                    scope.define_with_symbol(var, item_ty, false, symbol);
                    self.check_stmts(hir, body, scope, expected_return, decl, resolved)
                }
            }
        })();

        scope.hir_scope = previous_hir_scope;
        result
    }

    fn check_binding(
        &self,
        hir: &HirProgram<'_>,
        name: &str,
        annotation: &Option<Type>,
        value: &Expr,
        is_const: bool,
        scope: &mut Scope,
        span: Span,
        decl: Option<HirDeclId>,
        hir_scope: Option<HirScopeId>,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        let inferred = self.infer_expr_with_hir(hir, value, scope)?;
        let final_ty = if let Some(expected) = annotation {
            self.ensure_known_type(expected, span)?;
            ensure_assignable(expected, &inferred)
                .map_err(|e| self.error(span, format!("Tipo inválido para '{}': {}", name, e)))?;
            expected.clone()
        } else {
            inferred
        };

        let symbol_kind = if is_const {
            HirSymbolKind::ConstBinding
        } else {
            HirSymbolKind::LetBinding
        };
        let symbol = self.resolve_binding_symbol_in_hir_scope(
            hir_scope,
            decl,
            resolved,
            symbol_kind,
            name,
            span,
        );
        self.produce_symbol_metadata(symbol, &final_ty);
        scope.define_with_symbol(name, final_ty, is_const, symbol);
        Ok(())
    }
}
