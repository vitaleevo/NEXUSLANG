use crate::ast::{Decl, Expr, HttpMethod, Program, QueryParam, RouteAuthGuard, Stmt};
use crate::auth_ops::{AuthStaticOperation, CheckedAuthOperationArgs, AUTH_STATIC_TYPE_NAME};
use crate::model_ops::{CheckedModelOperationArgs, ModelStaticOperation};

pub use crate::auth_ops::AuthStaticOperation as CheckedAuthOperation;

#[derive(Debug, Clone, Copy)]
pub struct CheckedRouteView<'a> {
    pub method: &'a HttpMethod,
    pub path: &'a str,
    pub params: &'a [String],
    pub query_params: &'a [QueryParam],
    pub auth: Option<&'a RouteAuthGuard>,
    pub return_expr: Option<CheckedRouteExpr<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub enum CheckedRouteExpr<'a> {
    ModelOperation(CheckedRouteModelOperation<'a>),
    AuthOperation(CheckedRouteAuthOperation<'a>),
    Expr(&'a Expr),
}

#[derive(Debug, Clone, Copy)]
pub struct CheckedRouteModelOperation<'a> {
    pub model: &'a str,
    pub operation: ModelStaticOperation,
    pub args: &'a [Expr],
    pub checked_args: Option<CheckedModelOperationArgs<'a>>,
}

#[derive(Debug, Clone, Copy)]
pub struct CheckedRouteAuthOperation<'a> {
    pub operation: AuthStaticOperation,
    pub args: &'a [Expr],
    pub checked_args: Option<CheckedAuthOperationArgs<'a>>,
}

impl<'a> CheckedRouteExpr<'a> {
    pub fn from_expr(expr: &'a Expr) -> Self {
        if let Expr::StaticCall {
            ty, method, args, ..
        } = expr
        {
            if ty == AUTH_STATIC_TYPE_NAME {
                if let Some(operation) = AuthStaticOperation::from_method(method) {
                    return CheckedRouteExpr::AuthOperation(CheckedRouteAuthOperation {
                        operation,
                        args,
                        checked_args: operation.checked_args(args),
                    });
                }
            } else if let Some(operation) = ModelStaticOperation::from_method(method) {
                return CheckedRouteExpr::ModelOperation(CheckedRouteModelOperation {
                    model: ty,
                    operation,
                    args,
                    checked_args: operation.checked_args(args),
                });
            }
        }

        CheckedRouteExpr::Expr(expr)
    }
}

pub fn checked_routes(program: &Program) -> Vec<CheckedRouteView<'_>> {
    program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Route {
                method,
                path,
                params,
                query_params,
                auth,
                body,
                ..
            } => Some(CheckedRouteView {
                method,
                path,
                params,
                query_params,
                auth: auth.as_ref(),
                return_expr: checked_route_return_expr(body),
            }),
            _ => None,
        })
        .collect()
}

fn checked_route_return_expr(body: &[Stmt]) -> Option<CheckedRouteExpr<'_>> {
    body.iter().find_map(|stmt| match stmt {
        Stmt::Return { value, .. } => Some(CheckedRouteExpr::from_expr(value)),
        _ => None,
    })
}
