use crate::ast::{AuthConfig, Expr, Span, Type};
use crate::auth_ops::{AuthOperationReturnKind, AuthStaticOperation, CheckedAuthOperationArgs};
use crate::hir::HirProgram;

use super::{
    hir_args::{CheckedHirOperationArg, HirOperationArgs, HirOperationContext},
    CheckResult, Checker, Scope,
};

impl Checker {
    pub(super) fn infer_auth_return_expr(
        &self,
        operation: AuthStaticOperation,
        args: &[Expr],
        span: Span,
    ) -> CheckResult<Type> {
        let checked_args = self.check_auth_static_operation(operation, args, span)?;
        self.auth_return_type_from_checked_args(operation, checked_args, span)
    }

    pub(super) fn infer_hir_auth_return_expr<'a>(
        &self,
        hir: &HirProgram<'_>,
        operation: AuthStaticOperation,
        args: HirOperationArgs<'a>,
        scope: &Scope,
        span: Span,
    ) -> CheckResult<Type> {
        let checked_args =
            self.check_hir_auth_static_operation(hir, operation, args, scope, span)?;
        self.auth_return_type_from_checked_args(operation, checked_args, span)
    }

    fn auth_return_type_from_checked_args(
        &self,
        operation: AuthStaticOperation,
        checked_args: CheckedAuthOperationArgs<'_>,
        span: Span,
    ) -> CheckResult<Type> {
        match operation.return_kind() {
            AuthOperationReturnKind::AuthSuccess => {
                let config = self.auth_config_from_checked_args(operation, checked_args, span)?;
                Ok(Type::Model(config.model.clone()))
            }
            AuthOperationReturnKind::CurrentUser => Ok(Type::String),
            AuthOperationReturnKind::Bool => Ok(Type::Bool),
        }
    }

    pub(super) fn check_auth_static_operation<'a>(
        &self,
        operation: AuthStaticOperation,
        args: &'a [Expr],
        span: Span,
    ) -> CheckResult<CheckedAuthOperationArgs<'a>> {
        let Some(checked_args) = operation.checked_args(args) else {
            let error_span = if args.len() == 1 {
                args[0].span()
            } else {
                span
            };
            return Err(self.error(error_span, operation.argument_error(args)));
        };

        if let Some(name) = checked_args.auth_config_name() {
            let auth_symbol = self.auth_symbol(name);
            if auth_symbol.is_none() || !self.auths.contains_key(name) {
                return Err(self.error(
                    checked_args
                        .auth_config_expr()
                        .map(Expr::span)
                        .unwrap_or(span),
                    format!("Auth '{}' nao declarado", name),
                ));
            }
            if let (Some(expr), Some(symbol)) = (checked_args.auth_config_expr(), auth_symbol) {
                self.link_expr_symbol(expr, symbol);
            }
        }

        Ok(checked_args)
    }

    pub(super) fn auth_config_from_checked_args<'a>(
        &'a self,
        operation: AuthStaticOperation,
        checked_args: CheckedAuthOperationArgs<'_>,
        span: Span,
    ) -> CheckResult<&'a AuthConfig> {
        let Some(name) = checked_args.auth_config_name() else {
            return Err(self.error(span, operation.argument_error(checked_args.raw)));
        };
        let config = if self.auth_symbol(name).is_some() {
            self.auths.get(name)
        } else {
            None
        };
        config.ok_or_else(|| {
            self.error(
                checked_args
                    .auth_config_expr()
                    .map(Expr::span)
                    .unwrap_or(span),
                format!("Auth '{}' nao declarado", name),
            )
        })
    }

    pub(super) fn check_hir_auth_static_operation<'a>(
        &self,
        hir: &HirProgram<'_>,
        operation: AuthStaticOperation,
        args: HirOperationArgs<'a>,
        scope: &Scope,
        span: Span,
    ) -> CheckResult<CheckedAuthOperationArgs<'a>> {
        let checked_args = self.check_auth_static_operation(operation, args.raw, span)?;
        let checked_hir =
            HirOperationContext::with_hir(hir, args, scope).checked_hir_auth_args(checked_args);
        if let Some(auth_config) = checked_hir.auth_config() {
            if let Some(name) = auth_config.ident_name() {
                if let Some(symbol) = self.auth_symbol(name) {
                    if let Some(expr_id) = auth_config.hir_id() {
                        self.link_hir_expr_symbol(expr_id, symbol);
                    }
                }
            }
            self.record_checked_hir_operation_arg(auth_config);
        }
        Ok(checked_args)
    }

    fn record_checked_hir_operation_arg(&self, arg: CheckedHirOperationArg<'_>) {
        if arg.hir_id().is_some() {
            self.record_typed_hir_operation_arg_hit();
        }
    }
}
