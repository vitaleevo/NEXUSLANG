use crate::ast::{BinOp, Expr, HttpMethod, Span, Type};
use crate::hir::{self, HirExprId, HirProgram};

use super::{type_name, CheckResult, Checker, Scope};

impl Checker {
    pub(super) fn infer_route_return_expr(&self, expr: &Expr, scope: &Scope) -> CheckResult<Type> {
        if let Some(cached) = self.typed_hir_expr_type(expr) {
            return Ok(cached);
        }

        if let Expr::StaticCall {
            ty,
            method,
            args,
            span,
        } = expr
        {
            if let Some(inferred) =
                self.infer_route_static_call_return_expr(expr, ty, method, args, scope, *span)?
            {
                return Ok(inferred);
            }
        }

        self.infer_expr(expr, scope)
    }

    pub(super) fn ensure_route_expr(
        &self,
        expr: &Expr,
        scope: &Scope,
        route_method: &HttpMethod,
    ) -> CheckResult<()> {
        match expr {
            Expr::Integer { .. }
            | Expr::Float { .. }
            | Expr::StringLit { .. }
            | Expr::Bool { .. }
            | Expr::Money { .. }
            | Expr::Nil { .. } => Ok(()),
            Expr::Ident { name, span } => {
                if let Some((ty, symbol)) = self.resolve_scope_binding(scope, name) {
                    self.ensure_expr_metadata(expr, &ty, symbol);
                    Ok(())
                } else {
                    Err(self.error(*span, format!("Parametro '{}' nao definido na route", name)))
                }
            }
            Expr::Array { items, .. } => {
                for item in items {
                    self.ensure_route_expr(item, scope, route_method)?;
                }
                Ok(())
            }
            Expr::Object { fields, .. } => {
                for field in fields {
                    self.ensure_route_expr(&field.value, scope, route_method)?;
                }
                self.infer_expr(expr, scope)?;
                Ok(())
            }
            Expr::FieldAccess { object, .. } => {
                self.ensure_route_expr(object, scope, route_method)?;
                if self.typed_hir_expr_type(expr).is_some()
                    && self.typed_hir_expr_symbol(expr).is_some()
                {
                    return Ok(());
                }
                self.infer_expr(expr, scope)?;
                Ok(())
            }
            Expr::BinOp {
                left,
                op,
                right,
                span,
            } => {
                if !matches!(op, BinOp::Add) {
                    return Err(self.error(
                        *span,
                        "Route HTTP nesta fase so suporta operador '+' no return",
                    ));
                }
                self.ensure_route_expr(left, scope, route_method)?;
                self.ensure_route_expr(right, scope, route_method)?;
                let inferred = self.infer_expr(expr, scope)?;
                self.produce_expr_metadata(expr, &inferred, None);
                Ok(())
            }
            Expr::UnaryOp { span, .. } => Err(self.error(
                *span,
                "Route HTTP nesta fase nao suporta operador unario no return",
            )),
            Expr::Call { name, args, span } if name == "str" => {
                if args.len() != 1 {
                    return Err(self.error(*span, "str() recebe exatamente 1 argumento"));
                }
                self.ensure_route_expr(&args[0], scope, route_method)?;
                let inferred = self.infer_expr(expr, scope)?;
                self.produce_expr_metadata(expr, &inferred, None);
                Ok(())
            }
            Expr::Call { name, span, .. } => Err(self.error(
                *span,
                format!("Route HTTP nesta fase nao suporta chamada '{}()'", name),
            )),
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } => {
                if self.ensure_route_static_call_expr(
                    expr,
                    ty,
                    method,
                    args,
                    scope,
                    route_method,
                    *span,
                )? {
                    return Ok(());
                }
                self.infer_expr(expr, scope)?;
                Ok(())
            }
        }
    }

    pub(super) fn ensure_route_return_type(
        &self,
        path: &str,
        ty: &Type,
        span: Span,
    ) -> CheckResult<()> {
        match ty {
            Type::String
            | Type::Int
            | Type::Float
            | Type::Bool
            | Type::Money
            | Type::Array(_)
            | Type::Model(_) => Ok(()),
            Type::Optional(inner) => self.ensure_route_return_type(path, inner, span),
            Type::Void | Type::Unknown | Type::Nil => Err(self.error(
                span,
                format!(
                    "Route '{}' deve retornar valor HTTP concreto, encontrado {}",
                    path,
                    type_name(ty)
                ),
            )),
            Type::Date => Err(self.error(
                span,
                format!(
                    "Route '{}' nao pode retornar {} diretamente nesta fase",
                    path,
                    type_name(ty)
                ),
            )),
        }
    }

    pub(super) fn infer_route_return_expr_with_hir(
        &self,
        hir: &HirProgram<'_>,
        expr: &Expr,
        scope: &Scope,
    ) -> CheckResult<Type> {
        let Some(expr_id) = self.symbols.expr_id(expr) else {
            return self.infer_route_return_expr(expr, scope);
        };

        if let Some(cached) = self.typed_hir_expr_type_by_id(expr_id) {
            return Ok(cached);
        }

        let Some(hir_expr) = hir.expr(expr_id) else {
            return self.infer_route_return_expr(expr, scope);
        };

        if let hir::HirExprKind::StaticCall { ty, method, args } = &hir_expr.kind {
            return self.infer_hir_route_static_call_return(hir, hir_expr, ty, method, args, scope);
        }

        self.infer_hir_expr(hir, expr_id, scope)
    }

    pub(super) fn ensure_route_expr_with_hir(
        &self,
        hir: &HirProgram<'_>,
        expr: &Expr,
        scope: &Scope,
        route_method: &HttpMethod,
    ) -> CheckResult<()> {
        let Some(expr_id) = self.symbols.expr_id(expr) else {
            return self.ensure_route_expr(expr, scope, route_method);
        };

        self.ensure_hir_route_expr(hir, expr_id, scope, route_method)
    }

    pub(super) fn ensure_hir_route_expr(
        &self,
        hir: &HirProgram<'_>,
        expr_id: HirExprId,
        scope: &Scope,
        route_method: &HttpMethod,
    ) -> CheckResult<()> {
        let Some(expr) = hir.expr(expr_id) else {
            return Ok(());
        };

        match &expr.kind {
            hir::HirExprKind::Integer(_)
            | hir::HirExprKind::Float(_)
            | hir::HirExprKind::String(_)
            | hir::HirExprKind::Bool(_)
            | hir::HirExprKind::Money { .. }
            | hir::HirExprKind::Nil => Ok(()),
            hir::HirExprKind::Ident { name } => {
                if let Some((ty, symbol)) = self.resolve_scope_binding(scope, name) {
                    self.ensure_hir_expr_metadata(expr_id, &ty, symbol);
                    Ok(())
                } else {
                    Err(self.error(
                        expr.span,
                        format!("Parametro '{}' nao definido na route", name),
                    ))
                }
            }
            hir::HirExprKind::Array { items } => {
                for item in items {
                    self.ensure_hir_route_expr(hir, *item, scope, route_method)?;
                }
                Ok(())
            }
            hir::HirExprKind::Object { fields, .. } => {
                for field in fields {
                    self.ensure_hir_route_expr(hir, field.value, scope, route_method)?;
                }
                self.infer_hir_expr(hir, expr_id, scope)?;
                Ok(())
            }
            hir::HirExprKind::FieldAccess { object, .. } => {
                self.ensure_hir_route_expr(hir, *object, scope, route_method)?;
                if self.typed_hir_expr_type_by_id(expr_id).is_some()
                    && self.typed_hir_expr_symbol_by_id(expr_id).is_some()
                {
                    return Ok(());
                }
                self.infer_hir_expr(hir, expr_id, scope)?;
                Ok(())
            }
            hir::HirExprKind::Binary { left, op, right } => {
                if !matches!(op, hir::HirBinaryOp::Add) {
                    return Err(self.error(
                        expr.span,
                        "Route HTTP nesta fase so suporta operador '+' no return",
                    ));
                }
                self.ensure_hir_route_expr(hir, *left, scope, route_method)?;
                self.ensure_hir_route_expr(hir, *right, scope, route_method)?;
                let inferred = self.infer_hir_expr(hir, expr_id, scope)?;
                self.produce_hir_expr_metadata(expr_id, &inferred, None);
                Ok(())
            }
            hir::HirExprKind::Unary { .. } => Err(self.error(
                expr.span,
                "Route HTTP nesta fase nao suporta operador unario no return",
            )),
            hir::HirExprKind::Call { name, args } if *name == "str" => {
                if args.len() != 1 {
                    return Err(self.error(expr.span, "str() recebe exatamente 1 argumento"));
                }
                self.ensure_hir_route_expr(hir, args[0], scope, route_method)?;
                let inferred = self.infer_hir_expr(hir, expr_id, scope)?;
                self.produce_hir_expr_metadata(expr_id, &inferred, None);
                Ok(())
            }
            hir::HirExprKind::Call { name, .. } => Err(self.error(
                expr.span,
                format!("Route HTTP nesta fase nao suporta chamada '{}()'", name),
            )),
            hir::HirExprKind::StaticCall { ty, method, args } => {
                self.ensure_hir_route_static_call(hir, expr, ty, method, args, scope, route_method)
            }
        }
    }
}
