use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::hir::HirSymbolId;

use super::type_rules::{infer_std_builtin_call_type, infer_test_assert_call_type};
use super::{ensure_assignable, type_name, CheckResult, Checker, Scope};

impl Checker {
    pub(super) fn infer_expr(&self, expr: &Expr, scope: &Scope) -> CheckResult<Type> {
        if let Some(context) = self.typed_hir_expr_context(expr) {
            if let Some((_ty_id, ty)) = context.ty {
                let can_use_typed_hir = match expr {
                    Expr::Ident { .. } | Expr::FieldAccess { .. } => context.symbol.is_some(),
                    Expr::Object { .. }
                    | Expr::BinOp { .. }
                    | Expr::Call { .. }
                    | Expr::StaticCall { .. } => true,
                    _ => false,
                };

                if can_use_typed_hir {
                    self.record_hir_metadata_cache_hit();
                    self.record_typed_hir_expr_context_hit();
                    return Ok(ty);
                }
            }
        }

        let (inferred, symbol) = match expr {
            Expr::Integer { .. } => Ok((Type::Int, None)),
            Expr::Float { .. } => Ok((Type::Float, None)),
            Expr::StringLit { .. } => Ok((Type::String, None)),
            Expr::Bool { .. } => Ok((Type::Bool, None)),
            Expr::Money { .. } => Ok((Type::Money, None)),
            Expr::Nil { .. } => Ok((Type::Nil, None)),
            Expr::Array { items, span } => {
                let mut item_type = Type::Unknown;
                for item in items {
                    let ty = self.infer_expr(item, scope)?;
                    if item_type == Type::Unknown {
                        item_type = ty;
                    } else {
                        ensure_assignable(&item_type, &ty).map_err(|e| {
                            let error_span = if item.span().is_known() {
                                item.span()
                            } else {
                                *span
                            };
                            self.error(error_span, format!("Array com tipos incompatíveis: {}", e))
                        })?;
                    }
                }
                Ok((Type::Array(Box::new(item_type)), None))
            }
            Expr::Object {
                model,
                fields,
                span,
            } => {
                self.check_object_fields(model, fields, *span, scope)?;
                let resolved_model = self.resolved_model_name(model);
                Ok((
                    Type::Model(resolved_model.to_string()),
                    self.model_symbol(resolved_model),
                ))
            }
            Expr::Ident { name, span } => {
                let Some((ty, symbol)) = self.resolve_scope_binding(scope, name) else {
                    return Err(self.error(*span, format!("Variável '{}' não definida", name)));
                };
                Ok((ty, symbol))
            }
            Expr::FieldAccess {
                object,
                field,
                span,
            } => {
                let (ty, symbol) = self.infer_field_access(object, field, scope, *span)?;
                Ok((ty, symbol))
            }
            Expr::UnaryOp { op, expr, span } => {
                let ty = self.infer_expr(expr, scope)?;
                match op {
                    UnaryOp::Neg => match ty {
                        Type::Int | Type::Float | Type::Money => Ok((ty, None)),
                        _ => Err(self
                            .error(*span, format!("Operador '-' não aceita {}", type_name(&ty)))),
                    },
                    UnaryOp::Not => {
                        ensure_assignable(&Type::Bool, &ty).map_err(|e| {
                            self.error(*span, format!("Operador '!' inválido: {}", e))
                        })?;
                        Ok((Type::Bool, None))
                    }
                }
            }
            Expr::BinOp {
                left,
                op,
                right,
                span,
            } => self
                .infer_binop(left, op, right, scope, *span)
                .map(|ty| (ty, None)),
            Expr::Call { name, args, span } => {
                let ty = self.infer_call(name, args, scope, *span)?;
                Ok((ty, self.function_symbol(name)))
            }
            Expr::StaticCall {
                ty,
                method,
                args,
                span,
            } => {
                if self.model_symbol(ty).is_none() || !self.models.contains_key(ty) {
                    return Err(self.error(*span, format!("Model '{}' não encontrado", ty)));
                }
                if method != "all" {
                    return Err(self.error(
                        *span,
                        format!("Método estático '{}::{}' não existe", ty, method),
                    ));
                }
                if !args.is_empty() {
                    return Err(self.error(
                        *span,
                        format!("{}::all() fora de route nao recebe argumentos", ty),
                    ));
                }
                Ok((
                    Type::Array(Box::new(Type::Model(ty.clone()))),
                    self.model_symbol(ty),
                ))
            }
        }?;

        self.produce_expr_metadata(expr, &inferred, symbol);
        Ok(inferred)
    }

    pub(super) fn infer_expr_from_typed_hir_or_ast(
        &self,
        expr: &Expr,
        scope: &Scope,
    ) -> CheckResult<Type> {
        if let Some(ty) = self.typed_hir_expr_type(expr) {
            return Ok(ty);
        }

        self.infer_expr(expr, scope)
    }

    fn check_object_fields(
        &self,
        model: &str,
        fields: &[ObjectField],
        span: Span,
        scope: &Scope,
    ) -> CheckResult<()> {
        let resolved_model = self.resolved_model_name(model);
        let model_fields = self
            .models
            .get(resolved_model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;

        let expected = model_fields
            .iter()
            .map(|field| (field.name.as_str(), &field.ty))
            .collect::<HashMap<_, _>>();
        let mut seen = HashSet::new();

        for field in fields {
            if !seen.insert(field.name.as_str()) {
                return Err(self.error(
                    field.span,
                    format!("Campo '{}.{}' declarado mais de uma vez", model, field.name),
                ));
            }

            let Some(expected_ty) = expected.get(field.name.as_str()) else {
                return Err(self.error(
                    field.span,
                    format!("Campo '{}.{}' nao existe", model, field.name),
                ));
            };

            let actual = self.infer_expr_from_typed_hir_or_ast(&field.value, scope)?;
            ensure_assignable(expected_ty, &actual).map_err(|e| {
                self.error(
                    field.span,
                    format!("Campo '{}.{}': {}", model, field.name, e),
                )
            })?;
        }

        for field in model_fields {
            if !seen.contains(field.name.as_str())
                && field.default.is_none()
                && !is_optional_type(&field.ty)
            {
                return Err(self.error(
                    span,
                    format!("Campo '{}.{}' obrigatorio ausente", model, field.name),
                ));
            }
        }

        Ok(())
    }

    pub(super) fn resolved_model_name<'a>(&'a self, model: &'a str) -> &'a str {
        self.import_aliases
            .get(model)
            .map(String::as_str)
            .unwrap_or(model)
    }

    fn infer_field_access(
        &self,
        object: &Expr,
        field: &str,
        scope: &Scope,
        span: Span,
    ) -> CheckResult<(Type, Option<HirSymbolId>)> {
        let object_ty = self.infer_expr_from_typed_hir_or_ast(object, scope)?;
        let Type::Model(model) = object_ty else {
            return Err(self.error(
                span,
                format!(
                    "Acesso a campo '{}' espera model instance, encontrado {}",
                    field,
                    type_name(&object_ty)
                ),
            ));
        };

        let model_fields = self
            .models
            .get(&model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;

        model_fields
            .iter()
            .find(|candidate| candidate.name == field)
            .map(|candidate| (candidate.ty.clone(), self.model_field_symbol(&model, field)))
            .ok_or_else(|| self.error(span, format!("Campo '{}.{}' nao existe", model, field)))
    }

    fn infer_binop(
        &self,
        left: &Expr,
        op: &BinOp,
        right: &Expr,
        scope: &Scope,
        span: Span,
    ) -> CheckResult<Type> {
        let left_ty = self.infer_expr_from_typed_hir_or_ast(left, scope)?;
        let right_ty = self.infer_expr_from_typed_hir_or_ast(right, scope)?;

        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                numeric_result(op, &left_ty, &right_ty).map_err(|message| self.error(span, message))
            }
            BinOp::Eq | BinOp::NotEq => {
                ensure_assignable(&left_ty, &right_ty)
                    .map_err(|message| self.error(span, message))?;
                Ok(Type::Bool)
            }
            BinOp::Lt | BinOp::LtEq | BinOp::Gt | BinOp::GtEq => {
                ensure_comparable(&left_ty, &right_ty)
                    .map_err(|message| self.error(span, message))?;
                Ok(Type::Bool)
            }
            BinOp::And | BinOp::Or => {
                ensure_assignable(&Type::Bool, &left_ty)
                    .map_err(|message| self.error(span, message))?;
                ensure_assignable(&Type::Bool, &right_ty)
                    .map_err(|message| self.error(span, message))?;
                Ok(Type::Bool)
            }
        }
    }

    fn infer_call(
        &self,
        name: &str,
        args: &[Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<Type> {
        match name {
            "print" => {
                for arg in args {
                    self.infer_expr_from_typed_hir_or_ast(arg, scope)?;
                }
                return Ok(Type::Void);
            }
            "len" => {
                if args.len() != 1 {
                    return Err(self.error(span, "len() recebe exatamente 1 argumento"));
                }
                let ty = self.infer_expr_from_typed_hir_or_ast(&args[0], scope)?;
                if !matches!(ty, Type::Array(_) | Type::String | Type::Unknown) {
                    return Err(self.error(span, format!("len() não aceita {}", type_name(&ty))));
                }
                return Ok(Type::Int);
            }
            "str" => {
                if args.len() != 1 {
                    return Err(self.error(span, "str() recebe exatamente 1 argumento"));
                }
                self.infer_expr_from_typed_hir_or_ast(&args[0], scope)?;
                return Ok(Type::String);
            }
            "run_workflow" => {
                self.ensure_run_workflow_arg_count(args.len(), span)?;
                let ty = self.infer_expr_from_typed_hir_or_ast(&args[0], scope)?;
                let static_name = self.ast_static_workflow_name(&args[0]);
                let checked = self.check_run_workflow_target(&ty, static_name, span)?;
                if let Some(name) = static_name {
                    self.link_run_workflow_target_expr(&args[0], name);
                }
                return Ok(checked);
            }
            _ => {}
        }

        let arg_types = args
            .iter()
            .map(|arg| self.infer_expr_from_typed_hir_or_ast(arg, scope))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(result) = infer_std_builtin_call_type(name, &arg_types) {
            return result.map_err(|message| self.error(span, message));
        }
        if let Some(result) = infer_test_assert_call_type(name, &arg_types) {
            return result.map_err(|message| self.error(span, message));
        }

        let Some(_function_symbol) = self.function_symbol(name) else {
            return Err(self.error(span, format!("Função '{}' não definida", name)));
        };
        let sig = self
            .functions
            .get(name)
            .ok_or_else(|| self.error(span, format!("Função '{}' não definida", name)))?;

        if args.len() != sig.params.len() {
            return Err(self.error(
                span,
                format!(
                    "Função '{}' espera {} argumento(s), recebeu {}",
                    name,
                    sig.params.len(),
                    args.len()
                ),
            ));
        }

        for (arg, (_, expected)) in args.iter().zip(sig.params.iter()) {
            let actual = self.infer_expr_from_typed_hir_or_ast(arg, scope)?;
            ensure_assignable(expected, &actual).map_err(|e| {
                let error_span = if arg.span().is_known() {
                    arg.span()
                } else {
                    span
                };
                self.error(
                    error_span,
                    format!("Argumento inválido em '{}': {}", name, e),
                )
            })?;
        }

        Ok(sig.return_type.clone())
    }
}

pub(super) fn is_optional_type(ty: &Type) -> bool {
    matches!(ty, Type::Optional(_))
}

fn is_optional_or_nil_type(ty: &Type) -> bool {
    matches!(ty, Type::Optional(_) | Type::Nil)
}

pub(super) fn ensure_comparable(left: &Type, right: &Type) -> Result<(), String> {
    ensure_assignable(left, right)?;
    match left {
        Type::Int | Type::Float | Type::Money | Type::String | Type::Unknown => Ok(()),
        _ => Err(format!("{} não é comparável", type_name(left))),
    }
}

pub(super) fn numeric_result(op: &BinOp, left: &Type, right: &Type) -> Result<Type, String> {
    if is_optional_or_nil_type(left) || is_optional_or_nil_type(right) {
        return Err(format!(
            "operacao com opcional invalida: {} e {}",
            type_name(left),
            type_name(right)
        ));
    }

    match op {
        BinOp::Add if *left == Type::String && *right == Type::String => Ok(Type::String),
        BinOp::Add | BinOp::Sub => match (left, right) {
            (Type::Money, Type::Money) => Ok(Type::Money),
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::Int, Type::Float) | (Type::Float, Type::Int) | (Type::Float, Type::Float) => {
                Ok(Type::Float)
            }
            _ => Err(format!(
                "operação numérica inválida: {} e {}",
                type_name(left),
                type_name(right)
            )),
        },
        BinOp::Mul => match (left, right) {
            (Type::Money, Type::Int)
            | (Type::Money, Type::Float)
            | (Type::Int, Type::Money)
            | (Type::Float, Type::Money) => Ok(Type::Money),
            (Type::Int, Type::Int) => Ok(Type::Int),
            (Type::Int, Type::Float) | (Type::Float, Type::Int) | (Type::Float, Type::Float) => {
                Ok(Type::Float)
            }
            _ => Err(format!(
                "operação numérica inválida: {} e {}",
                type_name(left),
                type_name(right)
            )),
        },
        BinOp::Div => match (left, right) {
            (Type::Money, Type::Int) | (Type::Money, Type::Float) => Ok(Type::Money),
            (Type::Int, Type::Int)
            | (Type::Int, Type::Float)
            | (Type::Float, Type::Int)
            | (Type::Float, Type::Float) => Ok(Type::Float),
            _ => Err(format!(
                "divisão inválida: {} por {}",
                type_name(left),
                type_name(right)
            )),
        },
        BinOp::Mod => match (left, right) {
            (Type::Int, Type::Int) => Ok(Type::Int),
            _ => Err("módulo apenas aceita int".to_string()),
        },
        _ => unreachable!(),
    }
}
