use crate::ast::{Stmt, Type};
use crate::hir::{HirDeclId, HirProgram};

use super::resolver::ResolvedProgram;
use super::{CheckResult, Checker, Scope};

impl Checker {
    pub(super) fn check_top_level_statement(
        &self,
        hir: &HirProgram<'_>,
        stmt: &Stmt,
        top_scope: &mut Scope,
        decl: Option<HirDeclId>,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        self.check_stmt(hir, stmt, top_scope, &Type::Unknown, decl, resolved)
    }
}
