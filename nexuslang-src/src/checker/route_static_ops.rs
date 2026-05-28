use crate::ast::{Expr, HttpMethod, Span, Type};
use crate::auth_ops::{AuthStaticOperation, AUTH_STATIC_TYPE_NAME};
use crate::hir::{self, HirExprId, HirProgram};
use crate::model_ops::{CheckedModelOperationArgs, ModelStaticOperation};

use super::{
    hir_args::{HirOperationArgs, ModelOperationContext},
    CheckResult, Checker, Scope,
};

impl Checker {
    pub(super) fn infer_route_static_call_return_expr(
        &self,
        expr: &Expr,
        ty: &str,
        method: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<Option<Type>> {
        if ty == AUTH_STATIC_TYPE_NAME {
            let Some(operation) = AuthStaticOperation::from_method(method) else {
                return Err(self.error(
                    span,
                    format!("Metodo estatico 'Auth::{}' nao existe", method),
                ));
            };
            let inferred = self.infer_auth_return_expr(operation, args, span)?;
            self.produce_expr_metadata(expr, &inferred, None);
            return Ok(Some(inferred));
        }

        if let Some(operation) = ModelStaticOperation::from_method(method) {
            self.check_model_static_operation(ty, operation, args, scope, span)?;
            let inferred = operation.return_type(ty);
            self.produce_expr_metadata(expr, &inferred, self.model_symbol(ty));
            return Ok(Some(inferred));
        }

        Ok(None)
    }

    pub(super) fn ensure_route_static_call_expr(
        &self,
        expr: &Expr,
        ty: &str,
        method: &str,
        args: &[Expr],
        scope: &Scope,
        route_method: &HttpMethod,
        span: Span,
    ) -> CheckResult<bool> {
        if ty == AUTH_STATIC_TYPE_NAME {
            let Some(operation) = AuthStaticOperation::from_method(method) else {
                return Err(self.error(
                    span,
                    format!("Metodo estatico 'Auth::{}' nao existe", method),
                ));
            };
            let required = operation.required_route_method();
            if !required.matches(route_method) {
                return Err(self.error(span, operation.route_method_error()));
            }
            let inferred = self.infer_auth_return_expr(operation, args, span)?;
            self.produce_expr_metadata(expr, &inferred, None);
            return Ok(true);
        }

        if let Some(operation) = ModelStaticOperation::from_method(method) {
            self.ensure_model_route_operation_expr(ty, operation, args, scope, route_method, span)?;
            let inferred = operation.return_type(ty);
            self.produce_expr_metadata(expr, &inferred, self.model_symbol(ty));
            return Ok(true);
        }

        Ok(false)
    }

    pub(super) fn infer_hir_route_static_call_return(
        &self,
        hir: &HirProgram<'_>,
        expr: &hir::HirExpr<'_>,
        ty: &str,
        method: &str,
        arg_ids: &[HirExprId],
        scope: &Scope,
    ) -> CheckResult<Type> {
        let Some(args) = HirOperationArgs::from_static_call(expr.source, arg_ids) else {
            return self.infer_route_return_expr(expr.source, scope);
        };

        if ty == AUTH_STATIC_TYPE_NAME {
            let Some(operation) = AuthStaticOperation::from_method(method) else {
                return Err(self.error(
                    expr.span,
                    format!("Metodo estatico 'Auth::{}' nao existe", method),
                ));
            };
            let inferred =
                self.infer_hir_auth_return_expr(hir, operation, args, scope, expr.span)?;
            self.produce_hir_expr_metadata(expr.id, &inferred, None);
            return Ok(inferred);
        }

        if let Some(operation) = ModelStaticOperation::from_method(method) {
            self.check_hir_model_static_operation(hir, ty, operation, args, scope, expr.span)?;
            let inferred = operation.return_type(ty);
            self.produce_hir_expr_metadata(expr.id, &inferred, self.model_symbol(ty));
            return Ok(inferred);
        }

        self.infer_route_return_expr(expr.source, scope)
    }

    pub(super) fn ensure_hir_route_static_call(
        &self,
        hir: &HirProgram<'_>,
        expr: &hir::HirExpr<'_>,
        ty: &str,
        method: &str,
        arg_ids: &[HirExprId],
        scope: &Scope,
        route_method: &HttpMethod,
    ) -> CheckResult<()> {
        let Some(args) = HirOperationArgs::from_static_call(expr.source, arg_ids) else {
            return self.ensure_route_expr(expr.source, scope, route_method);
        };

        if ty == AUTH_STATIC_TYPE_NAME {
            let Some(operation) = AuthStaticOperation::from_method(method) else {
                return Err(self.error(
                    expr.span,
                    format!("Metodo estatico 'Auth::{}' nao existe", method),
                ));
            };
            let required = operation.required_route_method();
            if !required.matches(route_method) {
                return Err(self.error(expr.span, operation.route_method_error()));
            }
            let inferred =
                self.infer_hir_auth_return_expr(hir, operation, args, scope, expr.span)?;
            self.produce_hir_expr_metadata(expr.id, &inferred, None);
            return Ok(());
        }

        if let Some(operation) = ModelStaticOperation::from_method(method) {
            if let Some(required) = operation.required_route_method(args.raw.len()) {
                if !required.matches(route_method) {
                    let message = operation
                        .route_method_error(ty, args.raw.len())
                        .unwrap_or_else(|| {
                            format!(
                                "{}::{}() so pode ser usado em route {}",
                                ty,
                                operation.method_name(),
                                required.name()
                            )
                        });
                    return Err(self.error(expr.span, message));
                }
            }

            let checked =
                self.check_hir_model_static_operation(hir, ty, operation, args, scope, expr.span)?;
            for arg in checked.raw {
                self.ensure_hir_operation_arg(hir, args, arg, scope, route_method)?;
            }
            let inferred = operation.return_type(ty);
            self.produce_hir_expr_metadata(expr.id, &inferred, self.model_symbol(ty));
            return Ok(());
        }

        self.infer_expr(expr.source, scope)?;
        Ok(())
    }

    fn ensure_model_route_operation_expr(
        &self,
        model: &str,
        operation: ModelStaticOperation,
        args: &[Expr],
        scope: &Scope,
        route_method: &HttpMethod,
        span: Span,
    ) -> CheckResult<()> {
        if let Some(required) = operation.required_route_method(args.len()) {
            if !required.matches(route_method) {
                let message = operation
                    .route_method_error(model, args.len())
                    .unwrap_or_else(|| {
                        format!(
                            "{}::{}() so pode ser usado em route {}",
                            model,
                            operation.method_name(),
                            required.name()
                        )
                    });
                return Err(self.error(span, message));
            }
        }

        let checked_args =
            self.check_model_static_operation(model, operation, args, scope, span)?;
        for arg in checked_args.raw {
            self.ensure_route_expr(arg, scope, route_method)?;
        }
        Ok(())
    }

    fn check_hir_model_static_operation<'a>(
        &self,
        hir: &HirProgram<'_>,
        model: &str,
        operation: ModelStaticOperation,
        args: HirOperationArgs<'a>,
        scope: &Scope,
        span: Span,
    ) -> CheckResult<CheckedModelOperationArgs<'a>> {
        self.check_model_static_operation_with_context(
            model,
            operation,
            args.raw,
            ModelOperationContext::with_hir(hir, args, scope),
            span,
        )
    }

    fn ensure_hir_operation_arg(
        &self,
        hir: &HirProgram<'_>,
        args: HirOperationArgs<'_>,
        arg: &Expr,
        scope: &Scope,
        route_method: &HttpMethod,
    ) -> CheckResult<()> {
        let Some(expr_id) = args.id_for(arg) else {
            return self.ensure_route_expr(arg, scope, route_method);
        };
        self.record_typed_hir_operation_arg_hit();
        self.ensure_hir_route_expr(hir, expr_id, scope, route_method)
    }
}
