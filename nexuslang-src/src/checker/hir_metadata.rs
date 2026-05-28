use crate::ast::{Expr, Type};
use crate::hir::{self, HirExprId, HirRefId, HirSymbolId, HirTypeId};

use super::Checker;

#[derive(Debug, Clone)]
pub(super) struct TypedHirExprContext {
    pub(super) expr: HirExprId,
    pub(super) ty: Option<(HirTypeId, Type)>,
    pub(super) symbol: Option<HirSymbolId>,
}

impl Checker {
    pub fn checked_hir_metadata(&self) -> hir::HirCheckedMetadata {
        self.hir_metadata.snapshot()
    }

    #[cfg(test)]
    pub(super) fn hir_metadata_cache_hits(&self) -> usize {
        *self.hir_metadata_cache_hits.borrow()
    }

    #[cfg(test)]
    pub(super) fn scoped_hir_binding_hits(&self) -> usize {
        *self.scoped_hir_binding_hits.borrow()
    }

    #[cfg(test)]
    pub(super) fn typed_hir_binding_hits(&self) -> usize {
        *self.typed_hir_binding_hits.borrow()
    }

    #[cfg(test)]
    pub(super) fn typed_hir_expr_context_hits(&self) -> usize {
        *self.typed_hir_expr_context_hits.borrow()
    }

    #[cfg(test)]
    pub(super) fn typed_hir_expr_symbol_hits(&self) -> usize {
        *self.typed_hir_expr_symbol_hits.borrow()
    }

    #[cfg(test)]
    pub(super) fn typed_hir_expression_checker_hits(&self) -> usize {
        *self.typed_hir_expression_checker_hits.borrow()
    }

    #[cfg(test)]
    pub(super) fn typed_hir_operation_arg_hits(&self) -> usize {
        *self.typed_hir_operation_arg_hits.borrow()
    }

    #[cfg(test)]
    pub(super) fn typed_hir_model_op_validator_hits(&self) -> usize {
        *self.typed_hir_model_op_validator_hits.borrow()
    }

    #[cfg(test)]
    pub(super) fn typed_hir_reference_hits(&self) -> usize {
        *self.typed_hir_reference_hits.borrow()
    }

    pub(super) fn typed_hir_expr_context(&self, expr: &Expr) -> Option<TypedHirExprContext> {
        let expr_id = self.symbols.expr_id(expr)?;
        self.typed_hir_expr_context_by_id(expr_id)
    }

    pub(super) fn typed_hir_expr_context_by_id(
        &self,
        expr_id: HirExprId,
    ) -> Option<TypedHirExprContext> {
        self.hir_metadata.read(|metadata| {
            let ty = metadata
                .expr_type(expr_id)
                .and_then(|ty_id| metadata.ty(ty_id).cloned().map(|ty| (ty_id, ty)));
            let symbol = metadata.expr_symbol(expr_id);
            Some(TypedHirExprContext {
                expr: expr_id,
                ty,
                symbol,
            })
        })
    }

    pub(super) fn record_hir_metadata_cache_hit(&self) {
        #[cfg(test)]
        {
            *self.hir_metadata_cache_hits.borrow_mut() += 1;
        }
    }

    pub(super) fn record_typed_hir_expr_context_hit(&self) {
        #[cfg(test)]
        {
            *self.typed_hir_expr_context_hits.borrow_mut() += 1;
        }
    }

    pub(super) fn typed_hir_expr_type(&self, expr: &Expr) -> Option<Type> {
        let context = self.typed_hir_expr_context(expr)?;
        self.typed_hir_context_type(context)
    }

    pub(super) fn typed_hir_expr_type_by_id(&self, expr: HirExprId) -> Option<Type> {
        let context = self.typed_hir_expr_context_by_id(expr)?;
        self.typed_hir_context_type(context)
    }

    fn typed_hir_context_type(&self, context: TypedHirExprContext) -> Option<Type> {
        let (_ty_id, ty) = context.ty?;
        let _expr_id = context.expr;
        self.record_hir_metadata_cache_hit();
        self.record_typed_hir_expr_context_hit();
        Some(ty)
    }

    pub(super) fn typed_hir_expr_symbol(&self, expr: &Expr) -> Option<HirSymbolId> {
        let expr_id = self.symbols.expr_id(expr)?;
        self.typed_hir_expr_symbol_by_id(expr_id)
    }

    pub(super) fn typed_hir_expr_symbol_by_id(&self, expr: HirExprId) -> Option<HirSymbolId> {
        let symbol = self
            .typed_hir_expr_context_by_id(expr)
            .and_then(|context| context.symbol);
        if symbol.is_some() {
            self.record_hir_metadata_cache_hit();
            #[cfg(test)]
            {
                *self.typed_hir_expr_symbol_hits.borrow_mut() += 1;
            }
        }
        symbol
    }

    pub(super) fn typed_hir_reference_symbol(&self, reference: HirRefId) -> Option<HirSymbolId> {
        let symbol = self
            .hir_metadata
            .read(|metadata| metadata.reference_symbol(reference));
        if symbol.is_some() {
            self.record_hir_metadata_cache_hit();
            #[cfg(test)]
            {
                *self.typed_hir_reference_hits.borrow_mut() += 1;
            }
        }
        symbol
    }

    pub(super) fn checked_symbol_type(&self, symbol: HirSymbolId) -> Option<Type> {
        let ty = self.hir_metadata.read(|metadata| {
            let ty = metadata.symbol_type(symbol)?;
            metadata.ty(ty).cloned()
        });
        if ty.is_some() {
            #[cfg(test)]
            {
                *self.typed_hir_binding_hits.borrow_mut() += 1;
            }
        }
        ty
    }
}
