use crate::hir::HirSymbolId;

use super::Checker;

impl Checker {
    pub(super) fn model_symbol(&self, name: &str) -> Option<HirSymbolId> {
        self.symbols.models.get(name).copied()
    }

    pub(super) fn auth_symbol(&self, name: &str) -> Option<HirSymbolId> {
        self.symbols.auths.get(name).copied()
    }

    pub(super) fn workflow_symbol(&self, name: &str) -> Option<HirSymbolId> {
        self.symbols.workflows.get(name).copied()
    }

    pub(super) fn function_symbol(&self, name: &str) -> Option<HirSymbolId> {
        self.symbols.functions.get(name).copied()
    }

    pub(super) fn model_field_symbol(&self, model: &str, field: &str) -> Option<HirSymbolId> {
        self.symbols.model_field(model, field)
    }
}
