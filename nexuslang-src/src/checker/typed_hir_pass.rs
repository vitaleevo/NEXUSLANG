use std::cell::RefCell;

use crate::ast::{Expr, Type};
use crate::hir::{self, HirExprId, HirProgram, HirRefId, HirSymbolId};

use super::Checker;

#[derive(Debug)]
pub(super) struct TypedHirMetadataStore {
    metadata: RefCell<hir::HirCheckedMetadata>,
}

impl Default for TypedHirMetadataStore {
    fn default() -> Self {
        Self {
            metadata: RefCell::new(hir::HirCheckedMetadata::default()),
        }
    }
}

impl TypedHirMetadataStore {
    pub(super) fn snapshot(&self) -> hir::HirCheckedMetadata {
        self.metadata.borrow().clone()
    }

    pub(super) fn read<T>(&self, f: impl FnOnce(&hir::HirCheckedMetadata) -> T) -> T {
        let metadata = self.metadata.borrow();
        f(&metadata)
    }

    fn replace(&self, metadata: hir::HirCheckedMetadata) {
        *self.metadata.borrow_mut() = metadata;
    }

    fn write<T>(&self, f: impl FnOnce(&mut hir::HirCheckedMetadata) -> T) -> T {
        let mut metadata = self.metadata.borrow_mut();
        f(&mut metadata)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct TypedHirMetadataOwner;

impl TypedHirMetadataOwner {
    pub(super) fn initialize(program: &HirProgram<'_>) -> hir::HirCheckedMetadata {
        hir::HirCheckedMetadata::with_reference_counts(
            program.exprs.len(),
            program.symbols.len(),
            program.references.len(),
        )
    }

    fn link_expr_symbol(
        metadata: &mut hir::HirCheckedMetadata,
        expr: HirExprId,
        symbol: HirSymbolId,
    ) {
        metadata.set_expr_symbol(expr, symbol);
    }

    fn link_reference_symbol(
        metadata: &mut hir::HirCheckedMetadata,
        reference: HirRefId,
        symbol: HirSymbolId,
    ) {
        metadata.set_reference_symbol(reference, symbol);
    }

    fn produce_symbol_metadata(
        metadata: &mut hir::HirCheckedMetadata,
        symbol: HirSymbolId,
        ty: &Type,
    ) {
        metadata.set_symbol_type(symbol, ty);
    }

    fn produce_expr_metadata(
        metadata: &mut hir::HirCheckedMetadata,
        expr: HirExprId,
        ty: &Type,
        symbol: Option<HirSymbolId>,
    ) {
        if let Some(symbol) = symbol {
            metadata.set_expr_symbol(expr, symbol);
        }
        metadata.set_expr_type(expr, ty);
    }

    fn ensure_expr_metadata(
        metadata: &mut hir::HirCheckedMetadata,
        expr: HirExprId,
        ty: &Type,
        symbol: Option<HirSymbolId>,
    ) {
        if metadata.expr_type(expr).is_none() {
            metadata.set_expr_type(expr, ty);
        }
        if let Some(symbol) = symbol {
            if metadata.expr_symbol(expr).is_none() {
                metadata.set_expr_symbol(expr, symbol);
            }
        }
    }
}

impl Checker {
    pub(super) fn begin_typed_hir_metadata_pass(&self, program: &HirProgram<'_>) {
        self.hir_metadata
            .replace(TypedHirMetadataOwner::initialize(program));
        #[cfg(test)]
        {
            *self.hir_metadata_cache_hits.borrow_mut() = 0;
            *self.scoped_hir_binding_hits.borrow_mut() = 0;
            *self.typed_hir_binding_hits.borrow_mut() = 0;
            *self.typed_hir_expr_context_hits.borrow_mut() = 0;
            *self.typed_hir_expr_symbol_hits.borrow_mut() = 0;
            *self.typed_hir_expression_checker_hits.borrow_mut() = 0;
            *self.typed_hir_operation_arg_hits.borrow_mut() = 0;
            *self.typed_hir_model_op_validator_hits.borrow_mut() = 0;
            *self.typed_hir_reference_hits.borrow_mut() = 0;
        }
    }

    pub(super) fn link_expr_symbol(&self, expr: &Expr, symbol: HirSymbolId) {
        if let Some(expr_id) = self.symbols.expr_id(expr) {
            self.link_hir_expr_symbol(expr_id, symbol);
        }
    }

    pub(super) fn link_hir_expr_symbol(&self, expr: HirExprId, symbol: HirSymbolId) {
        self.hir_metadata
            .write(|metadata| TypedHirMetadataOwner::link_expr_symbol(metadata, expr, symbol));
    }

    pub(super) fn link_hir_reference_symbol(&self, reference: HirRefId, symbol: HirSymbolId) {
        self.hir_metadata.write(|metadata| {
            TypedHirMetadataOwner::link_reference_symbol(metadata, reference, symbol)
        });
    }

    pub(super) fn produce_symbol_metadata(&self, symbol: Option<HirSymbolId>, ty: &Type) {
        if let Some(symbol) = symbol {
            self.hir_metadata.write(|metadata| {
                TypedHirMetadataOwner::produce_symbol_metadata(metadata, symbol, ty)
            });
        }
    }

    pub(super) fn produce_expr_metadata(
        &self,
        expr: &Expr,
        ty: &Type,
        symbol: Option<HirSymbolId>,
    ) {
        if let Some(expr_id) = self.symbols.expr_id(expr) {
            self.produce_hir_expr_metadata(expr_id, ty, symbol);
        }
    }

    pub(super) fn produce_hir_expr_metadata(
        &self,
        expr: HirExprId,
        ty: &Type,
        symbol: Option<HirSymbolId>,
    ) {
        self.hir_metadata.write(|metadata| {
            TypedHirMetadataOwner::produce_expr_metadata(metadata, expr, ty, symbol)
        });
    }

    pub(super) fn ensure_expr_metadata(&self, expr: &Expr, ty: &Type, symbol: Option<HirSymbolId>) {
        if let Some(expr_id) = self.symbols.expr_id(expr) {
            self.ensure_hir_expr_metadata(expr_id, ty, symbol);
        }
    }

    pub(super) fn ensure_hir_expr_metadata(
        &self,
        expr: HirExprId,
        ty: &Type,
        symbol: Option<HirSymbolId>,
    ) {
        self.hir_metadata.write(|metadata| {
            TypedHirMetadataOwner::ensure_expr_metadata(metadata, expr, ty, symbol)
        });
    }
}
