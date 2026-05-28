use crate::ast::{Span, Stmt, Type};
use crate::hir::{HirDeclId, HirProgram, HirScopeId, HirSymbolKind};

use super::resolver::ResolvedProgram;
use super::{type_name, CheckResult, Checker, FunctionSig, Scope};

impl Checker {
    pub(super) fn collect_function_declaration(
        &mut self,
        name: &str,
        params: &[(String, Type)],
        return_type: &Type,
        span: Span,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        if self.functions.contains_key(name) {
            return Err(self.error(span, format!("Função '{}' declarada mais de uma vez", name)));
        }

        for (_, ty) in params {
            self.ensure_known_type(ty, span)?;
        }
        self.ensure_known_type(return_type, span)?;

        self.functions.insert(
            name.to_string(),
            FunctionSig {
                params: params.to_vec(),
                return_type: return_type.clone(),
            },
        );
        self.symbols.set_top_level(
            HirSymbolKind::Function,
            name,
            resolved.top_level_symbol(HirSymbolKind::Function, name),
        );

        Ok(())
    }

    pub(super) fn check_function_declaration(
        &self,
        hir: &HirProgram<'_>,
        name: &str,
        params: &[(String, Type)],
        return_type: &Type,
        body: &[Stmt],
        span: Span,
        top_scope: &Scope,
        decl: Option<HirDeclId>,
        hir_scope: Option<HirScopeId>,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        let mut scope = top_scope.clone().with_hir_scope(hir_scope);
        for (name, ty) in params {
            let symbol = self.resolve_binding_symbol(
                &scope,
                decl,
                resolved,
                HirSymbolKind::Parameter,
                name,
                span,
            );
            self.produce_symbol_metadata(symbol, ty);
            scope.define_with_symbol(name, ty.clone(), false, symbol);
        }

        self.check_stmts(hir, body, &mut scope, return_type, decl, resolved)?;
        if *return_type != Type::Void && !block_guarantees_return(body) {
            return Err(self.error(
                span,
                format!(
                    "Funcao '{}' deve retornar {} em todos os caminhos",
                    name,
                    type_name(return_type)
                ),
            ));
        }

        Ok(())
    }
}

fn block_guarantees_return(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_guarantees_return)
}

fn stmt_guarantees_return(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return { .. } => true,
        Stmt::If {
            then_body,
            else_body: Some(else_body),
            ..
        } => block_guarantees_return(then_body) && block_guarantees_return(else_body),
        _ => false,
    }
}
