use std::collections::{HashMap, HashSet};

use crate::ast::Type;
use crate::hir::{HirScopeId, HirSymbolId};

use super::ensure_assignable;

#[derive(Debug, Default, Clone)]
pub(super) struct Scope {
    pub(super) vars: HashMap<String, Type>,
    pub(super) consts: HashSet<String>,
    pub(super) symbols: HashMap<String, HirSymbolId>,
    pub(super) hir_scope: Option<HirScopeId>,
}

impl Scope {
    pub(super) fn with_hir_scope(mut self, hir_scope: Option<HirScopeId>) -> Self {
        self.hir_scope = hir_scope;
        self
    }

    pub(super) fn define_with_symbol(
        &mut self,
        name: &str,
        ty: Type,
        is_const: bool,
        symbol: Option<HirSymbolId>,
    ) {
        self.vars.insert(name.to_string(), ty);
        if is_const {
            self.consts.insert(name.to_string());
        }
        if let Some(symbol) = symbol {
            self.symbols.insert(name.to_string(), symbol);
        }
    }

    pub(super) fn assign(&self, name: &str, ty: &Type) -> Result<(), String> {
        if self.consts.contains(name) {
            return Err(format!("Constante '{}' não pode ser reatribuída", name));
        }

        let Some(existing) = self.vars.get(name) else {
            return Err(format!("Variável '{}' não definida", name));
        };

        ensure_assignable(existing, ty)
            .map_err(|e| format!("Tipo inválido ao atribuir '{}': {}", name, e))
    }

    pub(super) fn resolve(&self, name: &str) -> Option<(&Type, Option<HirSymbolId>)> {
        self.vars
            .get(name)
            .map(|ty| (ty, self.symbols.get(name).copied()))
    }
}
