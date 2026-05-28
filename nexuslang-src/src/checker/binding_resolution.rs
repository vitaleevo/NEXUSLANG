use crate::ast::{Span, Type};
use crate::hir::{HirDeclId, HirScopeId, HirSymbolId, HirSymbolKind};

use super::{ensure_assignable, Checker, ResolvedProgram, Scope};

impl Checker {
    pub(super) fn resolve_scope_binding(
        &self,
        scope: &Scope,
        name: &str,
    ) -> Option<(Type, Option<HirSymbolId>)> {
        let (legacy_ty, symbol) = scope.resolve(name)?;
        let ty = symbol
            .and_then(|symbol| self.checked_symbol_type(symbol))
            .unwrap_or_else(|| legacy_ty.clone());
        Some((ty, symbol))
    }

    pub(super) fn assign_in_scope(
        &self,
        scope: &Scope,
        name: &str,
        ty: &Type,
    ) -> Result<(), String> {
        if let Some((_legacy_ty, symbol)) = scope.resolve(name) {
            if scope.consts.contains(name) {
                return Err(format!("Constante '{}' não pode ser reatribuída", name));
            }
            if let Some(symbol) = symbol {
                if let Some(existing) = self.checked_symbol_type(symbol) {
                    return ensure_assignable(&existing, ty)
                        .map_err(|e| format!("Tipo inválido ao atribuir '{}': {}", name, e));
                }
            }
        }

        scope.assign(name, ty)
    }

    pub(super) fn resolve_binding_symbol(
        &self,
        scope: &Scope,
        decl: Option<HirDeclId>,
        resolved: &ResolvedProgram<'_>,
        kind: HirSymbolKind,
        name: &str,
        span: Span,
    ) -> Option<HirSymbolId> {
        self.resolve_binding_symbol_in_hir_scope(scope.hir_scope, decl, resolved, kind, name, span)
    }

    pub(super) fn resolve_binding_symbol_in_hir_scope(
        &self,
        hir_scope: Option<HirScopeId>,
        decl: Option<HirDeclId>,
        resolved: &ResolvedProgram<'_>,
        kind: HirSymbolKind,
        name: &str,
        span: Span,
    ) -> Option<HirSymbolId> {
        if let Some(symbol) =
            hir_scope.and_then(|scope| resolved.binding_symbol_in_scope(scope, kind, name, span))
        {
            #[cfg(test)]
            {
                *self.scoped_hir_binding_hits.borrow_mut() += 1;
            }
            return Some(symbol);
        }

        if let Some(symbol) = decl.and_then(|decl| resolved.binding_symbol(decl, kind, name, span))
        {
            return Some(symbol);
        }

        hir_scope.and_then(|scope| resolved.visible_binding_symbol(scope, &[kind], name, span))
    }
}
