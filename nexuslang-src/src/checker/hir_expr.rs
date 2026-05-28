use std::collections::{HashMap, HashSet};

use crate::ast::*;
#[cfg(test)]
use crate::auth_ops::{AuthStaticOperation, AUTH_STATIC_TYPE_NAME};
use crate::hir::{self, HirExprId, HirProgram, HirSymbolId};
#[cfg(test)]
use crate::model_ops::ModelStaticOperation;

use super::expr::{ensure_comparable, is_optional_type, numeric_result};
use super::type_rules::{
    hir_binary_op_to_ast, infer_std_builtin_call_type, infer_test_assert_call_type,
};
use super::{ensure_assignable, type_name, CheckResult, Checker, Scope};

use super::hir_args::{CheckedHirOperationArg, ModelOperationContext};
#[cfg(test)]
use super::hir_args::{HirOperationArgs, HirOperationContext};

impl Checker {
    pub(super) fn infer_expr_with_hir(
        &self,
        hir: &HirProgram<'_>,
        expr: &Expr,
        scope: &Scope,
    ) -> CheckResult<Type> {
        if let Some(expr_id) = self.symbols.expr_id(expr) {
            return self.infer_hir_expr(hir, expr_id, scope);
        }

        self.infer_expr(expr, scope)
    }

    fn record_typed_hir_expression_checker_hit(&self) {
        #[cfg(test)]
        {
            *self.typed_hir_expression_checker_hits.borrow_mut() += 1;
        }
    }

    pub(super) fn record_typed_hir_operation_arg_hit(&self) {
        #[cfg(test)]
        {
            *self.typed_hir_operation_arg_hits.borrow_mut() += 1;
        }
    }

    fn record_typed_hir_model_op_validator_hit(&self) {
        #[cfg(test)]
        {
            *self.typed_hir_model_op_validator_hits.borrow_mut() += 1;
        }
    }

    pub(super) fn infer_model_operation_arg(
        &self,
        context: ModelOperationContext<'_, '_, '_, '_>,
        expr: &Expr,
    ) -> CheckResult<Type> {
        if let Some((hir, expr_id)) = context.hir_expr(expr) {
            self.record_typed_hir_model_op_validator_hit();
            return self.infer_hir_expr(hir, expr_id, context.scope());
        }

        self.infer_expr(expr, context.scope())
    }

    pub(super) fn infer_model_operation_arg_id(
        &self,
        context: ModelOperationContext<'_, '_, '_, '_>,
        expr_id: Option<HirExprId>,
        fallback: &Expr,
    ) -> CheckResult<Type> {
        if let Some(expr_id) = expr_id {
            if let Some((hir, expr_id)) = context.hir_arg(expr_id) {
                self.record_typed_hir_model_op_validator_hit();
                return self.infer_hir_expr(hir, expr_id, context.scope());
            }
        }

        self.infer_model_operation_arg(context, fallback)
    }

    pub(super) fn infer_checked_model_operation_arg(
        &self,
        context: ModelOperationContext<'_, '_, '_, '_>,
        arg: CheckedHirOperationArg<'_>,
    ) -> CheckResult<Type> {
        self.infer_model_operation_arg_id(context, arg.hir_id(), arg.source())
    }

    pub(super) fn infer_hir_expr(
        &self,
        hir: &HirProgram<'_>,
        expr_id: HirExprId,
        scope: &Scope,
    ) -> CheckResult<Type> {
        let Some(expr) = hir.expr(expr_id) else {
            return Ok(Type::Unknown);
        };

        if let Some(context) = self.typed_hir_expr_context_by_id(expr_id) {
            if let Some((_ty_id, ty)) = context.ty {
                let can_use_typed_hir = match &expr.kind {
                    hir::HirExprKind::Ident { .. } | hir::HirExprKind::FieldAccess { .. } => {
                        context.symbol.is_some()
                    }
                    hir::HirExprKind::Object { .. }
                    | hir::HirExprKind::Binary { .. }
                    | hir::HirExprKind::Call { .. }
                    | hir::HirExprKind::StaticCall { .. } => true,
                    _ => false,
                };

                if can_use_typed_hir {
                    self.record_hir_metadata_cache_hit();
                    self.record_typed_hir_expr_context_hit();
                    self.record_typed_hir_expression_checker_hit();
                    return Ok(ty);
                }
            }
        }

        self.record_typed_hir_expression_checker_hit();
        let (inferred, symbol) = match &expr.kind {
            hir::HirExprKind::Integer(_) => Ok((Type::Int, None)),
            hir::HirExprKind::Float(_) => Ok((Type::Float, None)),
            hir::HirExprKind::String(_) => Ok((Type::String, None)),
            hir::HirExprKind::Bool(_) => Ok((Type::Bool, None)),
            hir::HirExprKind::Money { .. } => Ok((Type::Money, None)),
            hir::HirExprKind::Nil => Ok((Type::Nil, None)),
            hir::HirExprKind::Array { items } => {
                let mut item_type = Type::Unknown;
                for item in items {
                    let ty = self.infer_hir_expr(hir, *item, scope)?;
                    if item_type == Type::Unknown {
                        item_type = ty;
                    } else {
                        ensure_assignable(&item_type, &ty).map_err(|e| {
                            let item_span =
                                hir.expr(*item).map(|expr| expr.span).unwrap_or(expr.span);
                            let error_span = if item_span.is_known() {
                                item_span
                            } else {
                                expr.span
                            };
                            self.error(error_span, format!("Array com tipos incompatíveis: {}", e))
                        })?;
                    }
                }
                Ok((Type::Array(Box::new(item_type)), None))
            }
            hir::HirExprKind::Object { model, fields } => {
                self.check_hir_object_fields(hir, model, fields, expr.span, scope)?;
                let resolved_model = self.resolved_model_name(model);
                Ok((
                    Type::Model(resolved_model.to_string()),
                    self.model_symbol(resolved_model),
                ))
            }
            hir::HirExprKind::Ident { name } => {
                let Some((ty, symbol)) = self.resolve_scope_binding(scope, name) else {
                    return Err(self.error(expr.span, format!("Variável '{}' não definida", name)));
                };
                Ok((ty, symbol))
            }
            hir::HirExprKind::FieldAccess { object, field } => {
                let (ty, symbol) =
                    self.infer_hir_field_access(hir, *object, field, scope, expr.span)?;
                Ok((ty, symbol))
            }
            hir::HirExprKind::Unary {
                op,
                expr: inner_expr,
            } => {
                let ty = self.infer_hir_expr(hir, *inner_expr, scope)?;
                match op {
                    hir::HirUnaryOp::Neg => match ty {
                        Type::Int | Type::Float | Type::Money => Ok((ty, None)),
                        _ => Err(self.error(
                            expr.span,
                            format!("Operador '-' não aceita {}", type_name(&ty)),
                        )),
                    },
                    hir::HirUnaryOp::Not => {
                        ensure_assignable(&Type::Bool, &ty).map_err(|e| {
                            self.error(expr.span, format!("Operador '!' inválido: {}", e))
                        })?;
                        Ok((Type::Bool, None))
                    }
                }
            }
            hir::HirExprKind::Binary { left, op, right } => self
                .infer_hir_binop(hir, *left, *op, *right, scope, expr.span)
                .map(|ty| (ty, None)),
            hir::HirExprKind::Call { name, args } => {
                let ty = self.infer_hir_call(hir, name, args, scope, expr.span)?;
                Ok((ty, self.function_symbol(name)))
            }
            hir::HirExprKind::StaticCall { ty, method, args } => {
                if self.model_symbol(ty).is_none() || !self.models.contains_key(*ty) {
                    return Err(self.error(expr.span, format!("Model '{}' não encontrado", ty)));
                }
                if *method != "all" {
                    return Err(self.error(
                        expr.span,
                        format!("Método estático '{}::{}' não existe", ty, method),
                    ));
                }
                if !args.is_empty() {
                    return Err(self.error(
                        expr.span,
                        format!("{}::all() fora de route nao recebe argumentos", ty),
                    ));
                }
                Ok((
                    Type::Array(Box::new(Type::Model((*ty).to_string()))),
                    self.model_symbol(ty),
                ))
            }
        }?;

        self.produce_hir_expr_metadata(expr_id, &inferred, symbol);
        Ok(inferred)
    }

    fn check_hir_object_fields(
        &self,
        hir: &HirProgram<'_>,
        model: &str,
        fields: &[hir::HirObjectField<'_>],
        span: Span,
        scope: &Scope,
    ) -> CheckResult<()> {
        let model_fields = self.hir_model_fields(hir, model, span)?;

        let expected = model_fields
            .iter()
            .map(|field| (field.name, field))
            .collect::<HashMap<_, _>>();
        let mut seen = HashSet::new();

        for field in fields {
            if !seen.insert(field.name) {
                return Err(self.error(
                    field.span,
                    format!("Campo '{}.{}' declarado mais de uma vez", model, field.name),
                ));
            }

            let Some(expected_field) = expected.get(field.name) else {
                return Err(self.error(
                    field.span,
                    format!("Campo '{}.{}' nao existe", model, field.name),
                ));
            };

            self.link_hir_reference_symbol(field.field_ref, expected_field.symbol);
            let linked_symbol = self
                .typed_hir_reference_symbol(field.field_ref)
                .unwrap_or(expected_field.symbol);
            let expected_ty = self
                .checked_symbol_type(linked_symbol)
                .unwrap_or_else(|| (*expected_field.ty).clone());

            let actual = self.infer_hir_expr(hir, field.value, scope)?;
            ensure_assignable(&expected_ty, &actual).map_err(|e| {
                self.error(
                    field.span,
                    format!("Campo '{}.{}': {}", model, field.name, e),
                )
            })?;
        }

        for field in model_fields {
            if !seen.contains(field.name) && field.default.is_none() && !is_optional_type(field.ty)
            {
                return Err(self.error(
                    span,
                    format!("Campo '{}.{}' obrigatorio ausente", model, field.name),
                ));
            }
        }

        Ok(())
    }

    fn hir_model_fields<'hir, 'src>(
        &self,
        hir: &'hir HirProgram<'src>,
        model: &str,
        span: Span,
    ) -> CheckResult<&'hir [hir::HirModelField<'src>]> {
        // Try direct lookup first, then fall back to import alias resolution
        let effective_model = self
            .import_aliases
            .get(model)
            .map(|s| s.as_str())
            .unwrap_or(model);
        hir.decls
            .iter()
            .find_map(|decl| match &decl.body {
                hir::HirDeclBody::Model { fields } if decl.name == Some(effective_model) => {
                    Some(fields.as_slice())
                }
                _ => None,
            })
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))
    }

    fn infer_hir_field_access(
        &self,
        hir: &HirProgram<'_>,
        object: HirExprId,
        field: &str,
        scope: &Scope,
        span: Span,
    ) -> CheckResult<(Type, Option<HirSymbolId>)> {
        let object_ty = self.infer_hir_expr(hir, object, scope)?;
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

        let resolved_model = self.resolved_model_name(&model);
        let model_fields = self
            .models
            .get(resolved_model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;

        model_fields
            .iter()
            .find(|candidate| candidate.name == field)
            .map(|candidate| {
                (
                    candidate.ty.clone(),
                    self.model_field_symbol(resolved_model, field)
                        .or_else(|| self.model_field_symbol(&model, field)),
                )
            })
            .ok_or_else(|| self.error(span, format!("Campo '{}.{}' nao existe", model, field)))
    }

    fn infer_hir_binop(
        &self,
        hir: &HirProgram<'_>,
        left: HirExprId,
        op: hir::HirBinaryOp,
        right: HirExprId,
        scope: &Scope,
        span: Span,
    ) -> CheckResult<Type> {
        let left_ty = self.infer_hir_expr(hir, left, scope)?;
        let right_ty = self.infer_hir_expr(hir, right, scope)?;

        match op {
            hir::HirBinaryOp::Add
            | hir::HirBinaryOp::Sub
            | hir::HirBinaryOp::Mul
            | hir::HirBinaryOp::Div
            | hir::HirBinaryOp::Mod => {
                numeric_result(&hir_binary_op_to_ast(op), &left_ty, &right_ty)
                    .map_err(|message| self.error(span, message))
            }
            hir::HirBinaryOp::Eq | hir::HirBinaryOp::NotEq => {
                ensure_assignable(&left_ty, &right_ty)
                    .map_err(|message| self.error(span, message))?;
                Ok(Type::Bool)
            }
            hir::HirBinaryOp::Lt
            | hir::HirBinaryOp::LtEq
            | hir::HirBinaryOp::Gt
            | hir::HirBinaryOp::GtEq => {
                ensure_comparable(&left_ty, &right_ty)
                    .map_err(|message| self.error(span, message))?;
                Ok(Type::Bool)
            }
            hir::HirBinaryOp::And | hir::HirBinaryOp::Or => {
                ensure_assignable(&Type::Bool, &left_ty)
                    .map_err(|message| self.error(span, message))?;
                ensure_assignable(&Type::Bool, &right_ty)
                    .map_err(|message| self.error(span, message))?;
                Ok(Type::Bool)
            }
        }
    }

    fn infer_hir_call(
        &self,
        hir: &HirProgram<'_>,
        name: &str,
        args: &[HirExprId],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<Type> {
        match name {
            "print" => {
                for arg in args {
                    self.infer_hir_expr(hir, *arg, scope)?;
                }
                return Ok(Type::Void);
            }
            "len" => {
                if args.len() != 1 {
                    return Err(self.error(span, "len() recebe exatamente 1 argumento"));
                }
                let ty = self.infer_hir_expr(hir, args[0], scope)?;
                if !matches!(ty, Type::Array(_) | Type::String | Type::Unknown) {
                    return Err(self.error(span, format!("len() não aceita {}", type_name(&ty))));
                }
                return Ok(Type::Int);
            }
            "str" => {
                if args.len() != 1 {
                    return Err(self.error(span, "str() recebe exatamente 1 argumento"));
                }
                self.infer_hir_expr(hir, args[0], scope)?;
                return Ok(Type::String);
            }
            "run_workflow" => {
                self.ensure_run_workflow_arg_count(args.len(), span)?;
                let ty = self.infer_hir_expr(hir, args[0], scope)?;
                let static_name = self.hir_static_workflow_name(hir, args[0]);
                let checked = self.check_run_workflow_target(&ty, static_name, span)?;
                if let Some(name) = static_name {
                    self.link_hir_run_workflow_target_expr(args[0], name);
                }
                return Ok(checked);
            }
            _ => {}
        }

        let arg_types = args
            .iter()
            .map(|arg| self.infer_hir_expr(hir, *arg, scope))
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
            let actual = self.infer_hir_expr(hir, *arg, scope)?;
            ensure_assignable(expected, &actual).map_err(|e| {
                let arg_span = hir.expr(*arg).map(|expr| expr.span).unwrap_or(span);
                let error_span = if arg_span.is_known() { arg_span } else { span };
                self.error(
                    error_span,
                    format!("Argumento inválido em '{}': {}", name, e),
                )
            })?;
        }

        Ok(sig.return_type.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use super::*;
    use crate::hir::{lower_program, HirExprKind, HirProgram};

    #[test]
    fn model_operation_context_maps_checked_lookup_and_pagination_to_hir_ids() {
        let source = r#"
model Customer {
    score: int
}

route GET /customers ?(score: int = 10, limit: int = 5, offset: int = 0) {
    return Customer::where("score", score, limit, offset)
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let static_call = hir
            .exprs
            .iter()
            .find(|expr| {
                matches!(
                    &expr.kind,
                    HirExprKind::StaticCall {
                        ty: "Customer",
                        method: "where",
                        ..
                    }
                )
            })
            .unwrap();
        let HirExprKind::StaticCall { args, .. } = &static_call.kind else {
            unreachable!();
        };

        let hir_args = HirOperationArgs::from_static_call(static_call.source, args).unwrap();
        let checked = ModelStaticOperation::Where
            .checked_args(hir_args.raw)
            .unwrap();
        let checked_lookup = checked.lookup().unwrap();
        let checked_pagination = checked.pagination.unwrap();
        let scope = Scope::default();
        let context = ModelOperationContext::with_hir(&hir, hir_args, &scope);
        let checked_hir = context.checked_hir_model_args(checked);
        let lookup = checked_hir.lookup().unwrap();
        let pagination = checked_hir.pagination().unwrap();

        assert!(ptr::eq(lookup.field.source(), checked_lookup.0));
        assert!(ptr::eq(lookup.value.source(), checked_lookup.1));
        assert_eq!(lookup.field.hir_id(), hir_args.id_for(checked_lookup.0));
        assert_eq!(lookup.value.hir_id(), hir_args.id_for(checked_lookup.1));
        assert_eq!(
            pagination.limit.hir_id(),
            hir_args.id_for(checked_pagination.limit)
        );
        assert_eq!(
            pagination.offset.hir_id(),
            hir_args.id_for(checked_pagination.offset)
        );
        assert_eq!(lookup.field.hir_id(), Some(args[0]));
        assert_eq!(lookup.value.hir_id(), Some(args[1]));
        assert_eq!(pagination.limit.hir_id(), Some(args[2]));
        assert_eq!(pagination.offset.hir_id(), Some(args[3]));
    }

    #[test]
    fn model_operation_context_maps_checked_advanced_and_composite_filters_to_hir_ids() {
        let source = r#"
model Customer {
    status: string
    score: int
}

route GET /customers/compare ?(min_score: int = 10, limit: int = 5, offset: int = 0) {
    return Customer::where_compare_page("score", ">=", min_score, limit, offset)
}

route GET /customers/range ?(min_score: int = 10, max_score: int = 99) {
    return Customer::where_between("score", min_score, max_score)
}

route GET /customers/composite ?(score: int = 10, status: string = "active", limit: int = 5, offset: int = 0) {
    return Customer::where_all_page("score", score, "status", status, limit, offset)
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let scope = Scope::default();

        let (compare_source, compare_ids) = static_call_source_and_args(&hir, "where_compare_page");
        let compare_args = HirOperationArgs::from_static_call(compare_source, compare_ids).unwrap();
        let compare_checked = ModelStaticOperation::WhereComparePage
            .checked_args(compare_args.raw)
            .unwrap();
        let compare_filter = compare_checked.advanced_filter().unwrap();
        let compare_pagination = compare_checked.pagination.unwrap();
        let compare_context = ModelOperationContext::with_hir(&hir, compare_args, &scope);
        let compare_hir = compare_context.checked_hir_model_args(compare_checked);
        let advanced = compare_hir.advanced_filter().unwrap();
        let compare_page = compare_hir.pagination().unwrap();
        assert!(ptr::eq(advanced.field.source(), compare_filter.0));
        assert!(ptr::eq(advanced.operator.source(), compare_filter.1));
        assert!(ptr::eq(advanced.value.source(), compare_filter.2));
        assert_eq!(
            advanced.field.hir_id(),
            compare_args.id_for(compare_filter.0)
        );
        assert_eq!(
            advanced.operator.hir_id(),
            compare_args.id_for(compare_filter.1)
        );
        assert_eq!(
            advanced.value.hir_id(),
            compare_args.id_for(compare_filter.2)
        );
        assert_eq!(advanced.field.hir_id(), Some(compare_ids[0]));
        assert_eq!(advanced.operator.hir_id(), Some(compare_ids[1]));
        assert_eq!(advanced.value.hir_id(), Some(compare_ids[2]));
        assert_eq!(
            compare_page.limit.hir_id(),
            compare_args.id_for(compare_pagination.limit)
        );
        assert_eq!(
            compare_page.offset.hir_id(),
            compare_args.id_for(compare_pagination.offset)
        );
        assert_eq!(compare_page.limit.hir_id(), Some(compare_ids[3]));
        assert_eq!(compare_page.offset.hir_id(), Some(compare_ids[4]));

        let ast_compare =
            ModelOperationContext::ast(&scope).checked_hir_model_args(compare_checked);
        let ast_advanced = ast_compare.advanced_filter().unwrap();
        let ast_page = ast_compare.pagination().unwrap();
        assert_eq!(ast_advanced.field.hir_id(), None);
        assert_eq!(ast_advanced.operator.hir_id(), None);
        assert_eq!(ast_advanced.value.hir_id(), None);
        assert_eq!(ast_advanced.field.string_literal(), Some("score"));
        assert_eq!(ast_advanced.operator.string_literal(), Some(">="));
        assert_eq!(ast_advanced.operator.span(), compare_filter.1.span());
        assert_eq!(ast_page.limit.hir_id(), None);
        assert_eq!(ast_page.limit.span(), compare_pagination.limit.span());

        let (range_source, range_ids) = static_call_source_and_args(&hir, "where_between");
        let range_args = HirOperationArgs::from_static_call(range_source, range_ids).unwrap();
        let range_checked = ModelStaticOperation::WhereBetween
            .checked_args(range_args.raw)
            .unwrap();
        let range_filter = range_checked.range_filter().unwrap();
        let range_context = ModelOperationContext::with_hir(&hir, range_args, &scope);
        let range_hir = range_context.checked_hir_model_args(range_checked);
        let range = range_hir.range_filter().unwrap();
        assert!(ptr::eq(range.field.source(), range_filter.0));
        assert!(ptr::eq(range.min.source(), range_filter.1));
        assert!(ptr::eq(range.max.source(), range_filter.2));
        assert_eq!(range.field.hir_id(), range_args.id_for(range_filter.0));
        assert_eq!(range.min.hir_id(), range_args.id_for(range_filter.1));
        assert_eq!(range.max.hir_id(), range_args.id_for(range_filter.2));
        assert_eq!(range.field.hir_id(), Some(range_ids[0]));
        assert_eq!(range.min.hir_id(), Some(range_ids[1]));
        assert_eq!(range.max.hir_id(), Some(range_ids[2]));

        let (composite_source, composite_ids) = static_call_source_and_args(&hir, "where_all_page");
        let composite_args =
            HirOperationArgs::from_static_call(composite_source, composite_ids).unwrap();
        let composite_checked = ModelStaticOperation::WhereAllPage
            .checked_args(composite_args.raw)
            .unwrap();
        let composite_filters = composite_checked.composite_filter_args().unwrap();
        let composite_pagination = composite_checked.pagination.unwrap();
        let composite_context = ModelOperationContext::with_hir(&hir, composite_args, &scope);
        let composite_hir = composite_context.checked_hir_model_args(composite_checked);
        let composite = composite_hir.composite_filters().unwrap();
        let composite_page = composite_hir.pagination().unwrap();
        assert_eq!(composite.len(), 2);
        assert!(ptr::eq(composite[0].field.source(), &composite_filters[0]));
        assert!(ptr::eq(composite[0].value.source(), &composite_filters[1]));
        assert!(ptr::eq(composite[1].field.source(), &composite_filters[2]));
        assert!(ptr::eq(composite[1].value.source(), &composite_filters[3]));
        assert_eq!(
            composite[0].field.hir_id(),
            composite_args.id_for(&composite_filters[0])
        );
        assert_eq!(
            composite[0].value.hir_id(),
            composite_args.id_for(&composite_filters[1])
        );
        assert_eq!(
            composite[1].field.hir_id(),
            composite_args.id_for(&composite_filters[2])
        );
        assert_eq!(
            composite[1].value.hir_id(),
            composite_args.id_for(&composite_filters[3])
        );
        assert_eq!(composite[0].field.hir_id(), Some(composite_ids[0]));
        assert_eq!(composite[0].value.hir_id(), Some(composite_ids[1]));
        assert_eq!(composite[1].field.hir_id(), Some(composite_ids[2]));
        assert_eq!(composite[1].value.hir_id(), Some(composite_ids[3]));
        assert_eq!(
            composite_page.limit.hir_id(),
            composite_args.id_for(composite_pagination.limit)
        );
        assert_eq!(
            composite_page.offset.hir_id(),
            composite_args.id_for(composite_pagination.offset)
        );
        assert_eq!(composite_page.limit.hir_id(), Some(composite_ids[4]));
        assert_eq!(composite_page.offset.hir_id(), Some(composite_ids[5]));
    }

    #[test]
    fn operation_context_maps_checked_auth_args_to_shared_hir_arg() {
        let source = r#"
model User {
    email: string unique
}

auth UserAuth {
    model: User
    identity: email
}

route POST /auth/login {
    return Auth::login(UserAuth)
}
"#;

        let program = crate::parse_source(source).unwrap();
        let hir = lower_program(&program);
        let scope = Scope::default();

        let (login_source, login_ids) =
            static_call_source_and_args_for_type(&hir, AUTH_STATIC_TYPE_NAME, "login");
        let login_args = HirOperationArgs::from_static_call(login_source, login_ids).unwrap();
        let checked = AuthStaticOperation::Login
            .checked_args(login_args.raw)
            .unwrap();
        let auth_config_expr = checked.auth_config_expr().unwrap();
        let context = HirOperationContext::with_hir(&hir, login_args, &scope);
        let checked_hir = context.checked_hir_auth_args(checked);
        let auth_config = checked_hir.auth_config().unwrap();

        assert!(ptr::eq(auth_config.source(), auth_config_expr));
        assert_eq!(auth_config.ident_name(), Some("UserAuth"));
        assert_eq!(auth_config.hir_id(), login_args.id_for(auth_config_expr));
        assert_eq!(auth_config.hir_id(), Some(login_ids[0]));

        let ast_checked_hir = HirOperationContext::ast(&scope).checked_hir_auth_args(checked);
        let ast_auth_config = ast_checked_hir.auth_config().unwrap();
        assert_eq!(ast_auth_config.hir_id(), None);
        assert!(ptr::eq(ast_auth_config.source(), auth_config_expr));
        assert_eq!(ast_auth_config.ident_name(), Some("UserAuth"));
        assert_eq!(ast_auth_config.span(), auth_config_expr.span());
    }

    fn static_call_source_and_args<'hir, 'source>(
        hir: &'hir HirProgram<'source>,
        method: &str,
    ) -> (&'source Expr, &'hir [HirExprId]) {
        static_call_source_and_args_for_type(hir, "Customer", method)
    }

    fn static_call_source_and_args_for_type<'hir, 'source>(
        hir: &'hir HirProgram<'source>,
        ty: &str,
        method: &str,
    ) -> (&'source Expr, &'hir [HirExprId]) {
        let expr = hir
            .exprs
            .iter()
            .find(|expr| {
                matches!(
                    &expr.kind,
                    HirExprKind::StaticCall { ty: candidate_ty, method: candidate, .. }
                        if *candidate_ty == ty && *candidate == method
                )
            })
            .unwrap();
        let HirExprKind::StaticCall { args, .. } = &expr.kind else {
            unreachable!();
        };
        (expr.source, args)
    }
}
