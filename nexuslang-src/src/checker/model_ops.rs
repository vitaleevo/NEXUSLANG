use crate::ast::*;
use crate::model_ops::{
    starts_ordering_args, CheckedModelOperationArgs, ModelOperationCheckerValidation,
    ModelStaticOperation,
};

use super::hir_args::{
    CheckedHirModelAdvancedFilterArgs, CheckedHirModelLookupArgs, CheckedHirModelOperationArgs,
    CheckedHirModelOrderingArgs, CheckedHirModelPaginationArgs, CheckedHirModelRangeFilterArgs,
    CheckedHirOperationArg, ModelOperationContext,
};
use super::type_rules::{
    comparison_operator_supported, comparison_order_type_supported,
    ensure_comparison_operator_allowed, ordering_type_supported, text_filter_type_supported,
    text_operator_supported,
};
use super::{ensure_assignable, type_name, CheckResult, Checker, Scope};

#[derive(Debug, Clone, Copy)]
enum ModelLookupValidation {
    Exact,
    Optional,
    Array,
    OptionalArray,
}

#[derive(Debug, Clone, Copy)]
enum ModelAdvancedFilterValidation {
    Comparison,
    Text,
    Range,
}

impl Checker {
    pub(super) fn check_model_static_operation<'a>(
        &self,
        model: &str,
        operation: ModelStaticOperation,
        args: &'a [Expr],
        scope: &Scope,
        span: Span,
    ) -> CheckResult<CheckedModelOperationArgs<'a>> {
        self.check_model_static_operation_with_context(
            model,
            operation,
            args,
            ModelOperationContext::ast(scope),
            span,
        )
    }

    pub(super) fn check_model_static_operation_with_context<'a>(
        &self,
        model: &str,
        operation: ModelStaticOperation,
        args: &'a [Expr],
        context: ModelOperationContext<'_, '_, 'a, '_>,
        span: Span,
    ) -> CheckResult<CheckedModelOperationArgs<'a>> {
        let method = operation.method_name();
        match operation.checker_validation() {
            ModelOperationCheckerValidation::All => {
                self.check_model_all_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::Page => {
                self.check_model_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::Create => {
                if !self.models.contains_key(model) {
                    return Err(self.error(span, format!("Model '{}' nao encontrado", model)));
                }
                if !args.is_empty() {
                    return Err(
                        self.error(span, format!("{}::create() nao recebe argumentos", model))
                    );
                }
                Ok(())
            }
            ModelOperationCheckerValidation::Lookup => {
                self.check_model_lookup_call(model, operation, args, context, span)
            }
            ModelOperationCheckerValidation::Where => {
                self.check_model_where_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WherePage => {
                self.check_model_where_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereNot => {
                self.check_model_where_not_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereNotPage => {
                self.check_model_where_not_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereOptional => {
                self.check_model_where_optional_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereOptionalPage => {
                self.check_model_where_optional_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereIn => {
                self.check_model_where_in_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereInPage => {
                self.check_model_where_in_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereNotIn => {
                self.check_model_where_not_in_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereNotInPage => {
                self.check_model_where_not_in_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereNotInOptional => {
                self.check_model_where_not_in_optional_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereNotInOptionalPage => {
                self.check_model_where_not_in_optional_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereInOptional => {
                self.check_model_where_in_optional_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereInOptionalPage => {
                self.check_model_where_in_optional_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereCompare => {
                self.check_model_where_compare_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereComparePage => {
                self.check_model_where_compare_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereText => {
                self.check_model_where_text_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereTextPage => {
                self.check_model_where_text_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereBetween => {
                self.check_model_where_between_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereBetweenPage => {
                self.check_model_where_between_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereAll => {
                self.check_model_where_all_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereAllPage => {
                self.check_model_where_all_page_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereAny => {
                self.check_model_where_any_call(model, args, context, span)
            }
            ModelOperationCheckerValidation::WhereAnyPage => {
                self.check_model_where_any_page_call(model, args, context, span)
            }
        }?;

        operation.checked_args(args).ok_or_else(|| {
            self.error(
                span,
                format!("{}::{}() argumentos invalidos", model, method),
            )
        })
    }

    fn check_model_all_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        let hir_checked = ModelStaticOperation::All
            .checked_args(args)
            .map(|checked| context.checked_hir_model_args(checked));
        if !self.models.contains_key(model) {
            return Err(self.error(span, format!("Model '{}' nao encontrado", model)));
        }
        match args.len() {
            0 => Ok(()),
            2 if starts_ordering_args(args) => {
                let Some(ordering) = hir_checked.as_ref().and_then(|checked| checked.ordering())
                else {
                    return Err(self.error(
                        span,
                        format!(
                            "{}::all() recebe zero argumentos, limit e offset, ordenacao ou ordenacao com paginacao",
                            model
                        ),
                    ));
                };
                self.check_ordering_args(model, "all", ordering)
            }
            2 => {
                let Some(pagination) = hir_checked.as_ref().and_then(|checked| checked.pagination())
                else {
                    return Err(self.error(
                        span,
                        format!(
                            "{}::all() recebe zero argumentos, limit e offset, ordenacao ou ordenacao com paginacao",
                            model
                        ),
                    ));
                };
                self.check_pagination_args(model, "all", pagination, context)
            }
            4 => {
                let Some(ordering) = hir_checked.as_ref().and_then(|checked| checked.ordering())
                else {
                    return Err(self.error(
                        span,
                        format!(
                            "{}::all() recebe zero argumentos, limit e offset, ordenacao ou ordenacao com paginacao",
                            model
                        ),
                    ));
                };
                self.check_ordering_args(model, "all", ordering)?;
                let Some(pagination) = hir_checked.as_ref().and_then(|checked| checked.pagination())
                else {
                    return Err(self.error(
                        span,
                        format!(
                            "{}::all() recebe zero argumentos, limit e offset, ordenacao ou ordenacao com paginacao",
                            model
                        ),
                    ));
                };
                self.check_pagination_args(model, "all", pagination, context)
            }
            _ => Err(self.error(
                span,
                format!(
                    "{}::all() recebe zero argumentos, limit e offset, ordenacao ou ordenacao com paginacao",
                    model
                ),
            )),
        }
    }

    fn check_model_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        let hir_checked = ModelStaticOperation::Page
            .checked_args(args)
            .map(|checked| context.checked_hir_model_args(checked));
        if !self.models.contains_key(model) {
            return Err(self.error(span, format!("Model '{}' nao encontrado", model)));
        }
        match args.len() {
            2 => {
                let Some(pagination) = hir_checked
                    .as_ref()
                    .and_then(|checked| checked.pagination())
                else {
                    return Err(self.error(
                        span,
                        format!(
                            "{}::page() recebe limit/offset ou ordenacao com limit/offset",
                            model
                        ),
                    ));
                };
                self.check_pagination_args(model, "page", pagination, context)
            }
            4 => {
                let Some(ordering) = hir_checked.as_ref().and_then(|checked| checked.ordering())
                else {
                    return Err(self.error(
                        span,
                        format!(
                            "{}::page() recebe limit/offset ou ordenacao com limit/offset",
                            model
                        ),
                    ));
                };
                self.check_ordering_args(model, "page", ordering)?;
                let Some(pagination) = hir_checked
                    .as_ref()
                    .and_then(|checked| checked.pagination())
                else {
                    return Err(self.error(
                        span,
                        format!(
                            "{}::page() recebe limit/offset ou ordenacao com limit/offset",
                            model
                        ),
                    ));
                };
                self.check_pagination_args(model, "page", pagination, context)
            }
            _ => Err(self.error(
                span,
                format!(
                    "{}::page() recebe limit/offset ou ordenacao com limit/offset",
                    model
                ),
            )),
        }
    }

    fn checked_model_args_or_error<'a>(
        &self,
        model: &str,
        operation: ModelStaticOperation,
        args: &'a [Expr],
        span: Span,
        message: String,
    ) -> CheckResult<CheckedModelOperationArgs<'a>> {
        operation
            .checked_args(args)
            .ok_or_else(|| self.error(span, message))
            .and_then(|checked| {
                if self.models.contains_key(model) {
                    Ok(checked)
                } else {
                    Err(self.error(span, format!("Model '{}' nao encontrado", model)))
                }
            })
    }

    fn check_model_lookup_filter_family_call(
        &self,
        model: &str,
        operation: ModelStaticOperation,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        message: String,
        validation: ModelLookupValidation,
    ) -> CheckResult<()> {
        let method = operation.method_name();
        let checked = self.checked_model_args_or_error(model, operation, args, span, message)?;
        if checked.lookup().is_none() {
            return Err(self.error(
                span,
                format!("{}::{}() argumentos invalidos", model, method),
            ));
        }
        let hir_checked = context.checked_hir_model_args(checked);
        let Some(lookup) = hir_checked.lookup() else {
            return Err(self.error(
                span,
                format!("{}::{}() argumentos invalidos", model, method),
            ));
        };
        self.check_model_lookup_expr_pair(model, lookup, context, span, method, validation)?;
        self.check_model_operation_options(model, method, &hir_checked, context)
    }

    fn check_model_advanced_filter_family_call(
        &self,
        model: &str,
        operation: ModelStaticOperation,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        message: String,
        validation: ModelAdvancedFilterValidation,
    ) -> CheckResult<()> {
        let method = operation.method_name();
        let checked = self.checked_model_args_or_error(model, operation, args, span, message)?;
        let hir_checked = context.checked_hir_model_args(checked);
        match validation {
            ModelAdvancedFilterValidation::Comparison => {
                if checked.advanced_filter().is_none() {
                    return Err(self.error(
                        span,
                        format!("{}::{}() argumentos invalidos", model, method),
                    ));
                }
                let Some(filter) = hir_checked.advanced_filter() else {
                    return Err(self.error(
                        span,
                        format!("{}::{}() argumentos invalidos", model, method),
                    ));
                };
                self.check_model_compare_exprs(model, filter, context, span, method)?;
            }
            ModelAdvancedFilterValidation::Text => {
                if checked.advanced_filter().is_none() {
                    return Err(self.error(
                        span,
                        format!("{}::{}() argumentos invalidos", model, method),
                    ));
                }
                let Some(filter) = hir_checked.advanced_filter() else {
                    return Err(self.error(
                        span,
                        format!("{}::{}() argumentos invalidos", model, method),
                    ));
                };
                self.check_model_text_exprs(model, filter, context, span, method)?;
            }
            ModelAdvancedFilterValidation::Range => {
                if checked.range_filter().is_none() {
                    return Err(self.error(
                        span,
                        format!("{}::{}() argumentos invalidos", model, method),
                    ));
                }
                let Some(filter) = hir_checked.range_filter() else {
                    return Err(self.error(
                        span,
                        format!("{}::{}() argumentos invalidos", model, method),
                    ));
                };
                self.check_model_range_exprs(model, filter, context, span, method)?;
            }
        }
        self.check_model_operation_options(model, method, &hir_checked, context)
    }

    fn check_model_composite_filter_family_call(
        &self,
        model: &str,
        operation: ModelStaticOperation,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        message: String,
    ) -> CheckResult<()> {
        let method = operation.method_name();
        let checked = self.checked_model_args_or_error(model, operation, args, span, message)?;
        let Some(_filters) = checked.composite_filter_args() else {
            return Err(self.error(
                span,
                format!("{}::{}() argumentos invalidos", model, method),
            ));
        };
        let hir_checked = context.checked_hir_model_args(checked);
        let Some(filters) = hir_checked.composite_filters() else {
            return Err(self.error(
                span,
                format!("{}::{}() argumentos invalidos", model, method),
            ));
        };
        for pair in filters {
            self.check_model_lookup_expr_pair(
                model,
                *pair,
                context,
                span,
                method,
                ModelLookupValidation::Exact,
            )?;
        }
        self.check_model_operation_options(model, method, &hir_checked, context)
    }

    fn check_model_operation_options(
        &self,
        model: &str,
        method: &str,
        hir_args: &CheckedHirModelOperationArgs<'_>,
        context: ModelOperationContext<'_, '_, '_, '_>,
    ) -> CheckResult<()> {
        if let Some(ordering) = hir_args.ordering() {
            self.check_ordering_args(model, method, ordering)?;
        }
        if let Some(pagination) = hir_args.pagination() {
            self.check_pagination_args(model, method, pagination, context)?;
        }
        Ok(())
    }

    fn check_model_where_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::Where,
            args,
            context,
            span,
            format!(
                "{}::where() recebe campo e valor, com ordenacao e limit/offset opcionais",
                model
            ),
            ModelLookupValidation::Exact,
        )
    }

    fn check_model_where_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WherePage,
            args,
            context,
            span,
            format!(
                "{}::where_page() recebe campo, valor e limit/offset, com ordenacao opcional",
                model
            ),
            ModelLookupValidation::Exact,
        )
    }

    fn check_model_where_not_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereNot,
            args,
            context,
            span,
            format!(
                "{}::where_not() recebe campo e valor, com ordenacao e limit/offset opcionais",
                model
            ),
            ModelLookupValidation::Exact,
        )
    }

    fn check_model_where_not_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereNotPage,
            args,
            context,
            span,
            format!(
                "{}::where_not_page() recebe campo, valor e limit/offset, com ordenacao opcional",
                model
            ),
            ModelLookupValidation::Exact,
        )
    }

    fn check_model_where_optional_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereOptional,
            args,
            context,
            span,
            format!("{}::where_optional() recebe campo e valor opcional, com ordenacao e limit/offset opcionais", model),
            ModelLookupValidation::Optional,
        )
    }

    fn check_model_where_optional_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereOptionalPage,
            args,
            context,
            span,
            format!("{}::where_optional_page() recebe campo, valor opcional e limit/offset, com ordenacao opcional", model),
            ModelLookupValidation::Optional,
        )
    }

    fn check_model_where_in_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereIn,
            args,
            context,
            span,
            format!("{}::where_in() recebe campo e array de valores, com ordenacao e limit/offset opcionais", model),
            ModelLookupValidation::Array,
        )
    }

    fn check_model_where_in_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereInPage,
            args,
            context,
            span,
            format!("{}::where_in_page() recebe campo, array de valores e limit/offset, com ordenacao opcional", model),
            ModelLookupValidation::Array,
        )
    }

    fn check_model_where_not_in_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereNotIn,
            args,
            context,
            span,
            format!("{}::where_not_in() recebe campo e array de valores, com ordenacao e limit/offset opcionais", model),
            ModelLookupValidation::Array,
        )
    }

    fn check_model_where_not_in_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereNotInPage,
            args,
            context,
            span,
            format!("{}::where_not_in_page() recebe campo, array de valores e limit/offset, com ordenacao opcional", model),
            ModelLookupValidation::Array,
        )
    }

    fn check_model_where_not_in_optional_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereNotInOptional,
            args,
            context,
            span,
            format!("{}::where_not_in_optional() recebe campo e array opcional de valores, com ordenacao e limit/offset opcionais", model),
            ModelLookupValidation::OptionalArray,
        )
    }

    fn check_model_where_not_in_optional_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereNotInOptionalPage,
            args,
            context,
            span,
            format!("{}::where_not_in_optional_page() recebe campo, array opcional de valores e limit/offset, com ordenacao opcional", model),
            ModelLookupValidation::OptionalArray,
        )
    }

    fn check_model_where_in_optional_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereInOptional,
            args,
            context,
            span,
            format!("{}::where_in_optional() recebe campo e array opcional de valores, com ordenacao e limit/offset opcionais", model),
            ModelLookupValidation::OptionalArray,
        )
    }

    fn check_model_where_in_optional_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_lookup_filter_family_call(
            model,
            ModelStaticOperation::WhereInOptionalPage,
            args,
            context,
            span,
            format!("{}::where_in_optional_page() recebe campo, array opcional de valores e limit/offset, com ordenacao opcional", model),
            ModelLookupValidation::OptionalArray,
        )
    }

    fn check_model_where_compare_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_advanced_filter_family_call(
            model,
            ModelStaticOperation::WhereCompare,
            args,
            context,
            span,
            format!("{}::where_compare() recebe campo, operador e valor, com ordenacao e limit/offset opcionais", model),
            ModelAdvancedFilterValidation::Comparison,
        )
    }

    fn check_model_where_compare_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_advanced_filter_family_call(
            model,
            ModelStaticOperation::WhereComparePage,
            args,
            context,
            span,
            format!("{}::where_compare_page() recebe campo, operador, valor e limit/offset, com ordenacao opcional", model),
            ModelAdvancedFilterValidation::Comparison,
        )
    }

    fn check_model_where_text_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_advanced_filter_family_call(
            model,
            ModelStaticOperation::WhereText,
            args,
            context,
            span,
            format!("{}::where_text() recebe campo, operador textual e valor, com ordenacao e limit/offset opcionais", model),
            ModelAdvancedFilterValidation::Text,
        )
    }

    fn check_model_where_text_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_advanced_filter_family_call(
            model,
            ModelStaticOperation::WhereTextPage,
            args,
            context,
            span,
            format!("{}::where_text_page() recebe campo, operador textual, valor e limit/offset, com ordenacao opcional", model),
            ModelAdvancedFilterValidation::Text,
        )
    }

    fn check_model_where_between_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_advanced_filter_family_call(
            model,
            ModelStaticOperation::WhereBetween,
            args,
            context,
            span,
            format!("{}::where_between() recebe campo, min e max, com ordenacao e limit/offset opcionais", model),
            ModelAdvancedFilterValidation::Range,
        )
    }

    fn check_model_where_between_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_advanced_filter_family_call(
            model,
            ModelStaticOperation::WhereBetweenPage,
            args,
            context,
            span,
            format!("{}::where_between_page() recebe campo, min, max e limit/offset, com ordenacao opcional", model),
            ModelAdvancedFilterValidation::Range,
        )
    }

    fn check_model_where_all_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_composite_filter_family_call(
            model,
            ModelStaticOperation::WhereAll,
            args,
            context,
            span,
            format!(
                "{}::where_all() recebe ao menos dois pares campo/valor",
                model
            ),
        )
    }

    fn check_model_where_all_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_composite_filter_family_call(
            model,
            ModelStaticOperation::WhereAllPage,
            args,
            context,
            span,
            format!(
                "{}::where_all_page() recebe ao menos dois pares campo/valor e limit/offset",
                model
            ),
        )
    }

    fn check_model_where_any_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_composite_filter_family_call(
            model,
            ModelStaticOperation::WhereAny,
            args,
            context,
            span,
            format!(
                "{}::where_any() recebe ao menos dois pares campo/valor",
                model
            ),
        )
    }

    fn check_model_where_any_page_call(
        &self,
        model: &str,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        self.check_model_composite_filter_family_call(
            model,
            ModelStaticOperation::WhereAnyPage,
            args,
            context,
            span,
            format!(
                "{}::where_any_page() recebe ao menos dois pares campo/valor e limit/offset",
                model
            ),
        )
    }

    fn check_model_lookup_call(
        &self,
        model: &str,
        operation: ModelStaticOperation,
        args: &[Expr],
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
    ) -> CheckResult<()> {
        let method = operation.method_name();
        if args.len() != 2 {
            return Err(self.error(
                span,
                format!("{}::{}() recebe campo e valor", model, method),
            ));
        }
        let hir_lookup = operation
            .checked_args(args)
            .map(|checked| context.checked_hir_model_args(checked))
            .and_then(|checked| checked.lookup());
        self.check_model_lookup_args(model, args, hir_lookup, context, span, method)
    }

    fn check_model_lookup_args(
        &self,
        model: &str,
        args: &[Expr],
        lookup: Option<CheckedHirModelLookupArgs<'_>>,
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let [_, _] = args else {
            return Err(self.error(
                span,
                format!("{}::{}() recebe campo e valor", model, method),
            ));
        };
        let Some(lookup) = lookup else {
            return Err(self.error(
                span,
                format!("{}::{}() recebe campo e valor", model, method),
            ));
        };
        self.check_model_lookup_expr_pair(
            model,
            lookup,
            context,
            span,
            method,
            ModelLookupValidation::Exact,
        )
    }

    fn check_model_lookup_expr_pair(
        &self,
        model: &str,
        lookup: CheckedHirModelLookupArgs<'_>,
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        method: &str,
        validation: ModelLookupValidation,
    ) -> CheckResult<()> {
        match validation {
            ModelLookupValidation::Exact => {
                self.check_model_exact_lookup_expr_pair(model, lookup, context, span, method)
            }
            ModelLookupValidation::Optional => {
                self.check_model_optional_lookup_expr_pair(model, lookup, context, span, method)
            }
            ModelLookupValidation::Array => {
                self.check_model_array_lookup_expr_pair(model, lookup, context, span, method)
            }
            ModelLookupValidation::OptionalArray => self
                .check_model_optional_array_lookup_expr_pair(model, lookup, context, span, method),
        }
    }

    fn check_model_exact_lookup_expr_pair(
        &self,
        model: &str,
        lookup: CheckedHirModelLookupArgs<'_>,
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let (field_ty, field) = self.model_field_for_filter(model, lookup.field, span, method)?;
        let actual = self.infer_checked_model_operation_arg(context, lookup.value)?;
        ensure_assignable(&field_ty, &actual).map_err(|e| {
            self.error(
                lookup.value.span(),
                format!(
                    "{}::{}() valor invalido para '{}': {}",
                    model, method, field, e
                ),
            )
        })
    }

    fn check_model_optional_lookup_expr_pair(
        &self,
        model: &str,
        lookup: CheckedHirModelLookupArgs<'_>,
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let (field_ty, field) = self.model_field_for_filter(model, lookup.field, span, method)?;
        let actual = self.infer_checked_model_operation_arg(context, lookup.value)?;
        let Type::Optional(inner) = &actual else {
            return Err(self.error(
                lookup.value.span(),
                format!(
                    "{}::{}() valor para '{}' deve ser opcional, encontrado {}",
                    model,
                    method,
                    field,
                    type_name(&actual)
                ),
            ));
        };
        ensure_assignable(&field_ty, inner).map_err(|e| {
            self.error(
                lookup.value.span(),
                format!(
                    "{}::{}() valor invalido para '{}': {}",
                    model, method, field, e
                ),
            )
        })
    }

    fn check_model_array_lookup_expr_pair(
        &self,
        model: &str,
        lookup: CheckedHirModelLookupArgs<'_>,
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let (field_ty, field) = self.model_field_for_filter(model, lookup.field, span, method)?;
        let actual = self.infer_checked_model_operation_arg(context, lookup.value)?;
        let Type::Array(item_ty) = &actual else {
            return Err(self.error(
                lookup.value.span(),
                format!(
                    "{}::{}() valores para '{}' devem ser array, encontrado {}",
                    model,
                    method,
                    field,
                    type_name(&actual)
                ),
            ));
        };
        self.check_model_array_item_type(model, method, field, lookup.value, &field_ty, item_ty)
    }

    fn check_model_optional_array_lookup_expr_pair(
        &self,
        model: &str,
        lookup: CheckedHirModelLookupArgs<'_>,
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let (field_ty, field) = self.model_field_for_filter(model, lookup.field, span, method)?;
        let actual = self.infer_checked_model_operation_arg(context, lookup.value)?;
        let Type::Optional(inner) = &actual else {
            return Err(self.error(
                lookup.value.span(),
                format!(
                    "{}::{}() valores para '{}' devem ser array opcional, encontrado {}",
                    model,
                    method,
                    field,
                    type_name(&actual)
                ),
            ));
        };
        let Type::Array(item_ty) = inner.as_ref() else {
            return Err(self.error(
                lookup.value.span(),
                format!(
                    "{}::{}() valores para '{}' devem ser array opcional, encontrado {}",
                    model,
                    method,
                    field,
                    type_name(&actual)
                ),
            ));
        };
        self.check_model_array_item_type(model, method, field, lookup.value, &field_ty, item_ty)
    }

    fn check_model_array_item_type(
        &self,
        model: &str,
        method: &str,
        field: &str,
        value_arg: CheckedHirOperationArg<'_>,
        field_ty: &Type,
        item_ty: &Type,
    ) -> CheckResult<()> {
        if matches!(item_ty, Type::Optional(_) | Type::Nil) {
            return Err(self.error(
                value_arg.span(),
                format!(
                    "{}::{}() itens para '{}' devem ser valores concretos",
                    model, method, field
                ),
            ));
        }
        ensure_assignable(field_ty, item_ty).map_err(|e| {
            self.error(
                value_arg.span(),
                format!(
                    "{}::{}() item invalido para '{}': {}",
                    model, method, field, e
                ),
            )
        })
    }

    fn model_field_for_filter<'b>(
        &self,
        model: &str,
        field_arg: CheckedHirOperationArg<'b>,
        span: Span,
        method: &str,
    ) -> CheckResult<(Type, &'b str)> {
        let fields = self
            .models
            .get(model)
            .ok_or_else(|| self.error(span, format!("Model '{}' nao encontrado", model)))?;
        let Some(field) = field_arg.string_literal() else {
            return Err(self.error(
                field_arg.span(),
                format!(
                    "{}::{}() espera nome de campo como string literal",
                    model, method
                ),
            ));
        };
        let Some(model_field) = fields.iter().find(|candidate| candidate.name == field) else {
            return Err(self.error(
                field_arg.span(),
                format!("Campo '{}.{}' nao existe", model, field),
            ));
        };
        let field_ty =
            self.linked_model_field_operation_arg_type(model, field, field_arg, &model_field.ty);

        Ok((field_ty, field))
    }

    fn check_model_compare_exprs(
        &self,
        model: &str,
        filter: CheckedHirModelAdvancedFilterArgs<'_>,
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let (field_ty, field) = self.model_field_for_filter(model, filter.field, span, method)?;
        let Some(operator) = filter.operator.string_literal() else {
            return Err(self.error(
                filter.operator.span(),
                format!(
                    "{}::{}() espera operador como string literal",
                    model, method
                ),
            ));
        };
        if !comparison_operator_supported(operator) {
            return Err(self.error(
                filter.operator.span(),
                format!(
                    "{}::{}() operador deve ser \"==\", \"!=\", \">\", \">=\", \"<\" ou \"<=\"",
                    model, method
                ),
            ));
        }

        let actual = self.infer_checked_model_operation_arg(context, filter.value)?;
        ensure_assignable(&field_ty, &actual).map_err(|e| {
            self.error(
                filter.value.span(),
                format!(
                    "{}::{}() valor invalido para '{}': {}",
                    model, method, field, e
                ),
            )
        })?;
        ensure_comparison_operator_allowed(operator, &field_ty, &actual).map_err(|e| {
            self.error(
                filter.operator.span(),
                format!("{}::{}() {}", model, method, e),
            )
        })
    }

    fn check_model_text_exprs(
        &self,
        model: &str,
        filter: CheckedHirModelAdvancedFilterArgs<'_>,
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let (field_ty, field) = self.model_field_for_filter(model, filter.field, span, method)?;
        if !text_filter_type_supported(&field_ty) {
            return Err(self.error(
                filter.field.span(),
                format!(
                    "{}::{}() campo '{}' deve ser string ou string?",
                    model, method, field
                ),
            ));
        }

        let Some(operator) = filter.operator.string_literal() else {
            return Err(self.error(
                filter.operator.span(),
                format!(
                    "{}::{}() espera operador textual como string literal",
                    model, method
                ),
            ));
        };
        if !text_operator_supported(operator) {
            return Err(self.error(
                filter.operator.span(),
                format!(
                    "{}::{}() operador textual deve ser \"contains\", \"starts_with\", \"ends_with\", \"icontains\", \"istarts_with\" ou \"iends_with\"",
                    model, method
                ),
            ));
        }

        let actual = self.infer_checked_model_operation_arg(context, filter.value)?;
        ensure_assignable(&field_ty, &actual).map_err(|e| {
            self.error(
                filter.value.span(),
                format!(
                    "{}::{}() valor invalido para '{}': {}",
                    model, method, field, e
                ),
            )
        })
    }

    fn check_model_range_exprs(
        &self,
        model: &str,
        filter: CheckedHirModelRangeFilterArgs<'_>,
        context: ModelOperationContext<'_, '_, '_, '_>,
        span: Span,
        method: &str,
    ) -> CheckResult<()> {
        let (field_ty, field) = self.model_field_for_filter(model, filter.field, span, method)?;
        if !comparison_order_type_supported(&field_ty) {
            return Err(self.error(
                filter.field.span(),
                format!(
                    "{}::{}() campo '{}' deve ser ordenavel para range",
                    model, method, field
                ),
            ));
        }

        for (label, arg) in [("min", filter.min), ("max", filter.max)] {
            let actual = self.infer_checked_model_operation_arg(context, arg)?;
            if matches!(actual, Type::Nil | Type::Optional(_)) {
                return Err(self.error(
                    arg.span(),
                    format!(
                        "{}::{}() {} deve ser valor concreto, encontrado {}",
                        model,
                        method,
                        label,
                        type_name(&actual)
                    ),
                ));
            }
            ensure_assignable(&field_ty, &actual).map_err(|e| {
                self.error(
                    arg.span(),
                    format!(
                        "{}::{}() {} invalido para '{}': {}",
                        model, method, label, field, e
                    ),
                )
            })?;
        }

        Ok(())
    }

    fn check_ordering_args(
        &self,
        model: &str,
        method: &str,
        ordering: CheckedHirModelOrderingArgs<'_>,
    ) -> CheckResult<()> {
        let fields = self.models.get(model).ok_or_else(|| {
            self.error(
                ordering.field.span(),
                format!("Model '{}' nao encontrado", model),
            )
        })?;
        let Some(field) = ordering.field.string_literal() else {
            return Err(self.error(
                ordering.field.span(),
                format!(
                    "{}::{}() espera campo de ordenacao como string literal",
                    model, method
                ),
            ));
        };
        let Some(model_field) = fields.iter().find(|candidate| candidate.name == field) else {
            return Err(self.error(
                ordering.field.span(),
                format!("Campo '{}.{}' nao existe", model, field),
            ));
        };
        let field_ty = self.linked_model_field_operation_arg_type(
            model,
            field,
            ordering.field,
            &model_field.ty,
        );
        if !ordering_type_supported(&field_ty) {
            return Err(self.error(
                ordering.field.span(),
                format!(
                    "Campo '{}.{}' nao pode ser usado para ordenacao",
                    model, field
                ),
            ));
        }

        let Some(direction) = ordering.direction.string_literal() else {
            return Err(self.error(
                ordering.direction.span(),
                format!(
                    "{}::{}() espera direcao de ordenacao como string literal",
                    model, method
                ),
            ));
        };
        if direction != "asc" && direction != "desc" {
            return Err(self.error(
                ordering.direction.span(),
                format!(
                    "{}::{}() direcao de ordenacao deve ser \"asc\" ou \"desc\"",
                    model, method
                ),
            ));
        }

        Ok(())
    }

    fn link_model_field_operation_arg(
        &self,
        model: &str,
        field: &str,
        arg: CheckedHirOperationArg<'_>,
    ) {
        if let (Some(expr), Some(symbol)) = (arg.hir_id(), self.model_field_symbol(model, field)) {
            self.produce_hir_expr_metadata(expr, &Type::String, Some(symbol));
        }
    }

    fn linked_model_field_operation_arg_type(
        &self,
        model: &str,
        field: &str,
        arg: CheckedHirOperationArg<'_>,
        fallback_ty: &Type,
    ) -> Type {
        self.link_model_field_operation_arg(model, field, arg);
        let Some(expr) = arg.hir_id() else {
            return fallback_ty.clone();
        };
        let Some(symbol) = self.typed_hir_expr_symbol_by_id(expr) else {
            return fallback_ty.clone();
        };
        self.checked_symbol_type(symbol)
            .unwrap_or_else(|| fallback_ty.clone())
    }

    fn check_pagination_args(
        &self,
        model: &str,
        method: &str,
        pagination: CheckedHirModelPaginationArgs<'_>,
        context: ModelOperationContext<'_, '_, '_, '_>,
    ) -> CheckResult<()> {
        let limit_ty = self.infer_checked_model_operation_arg(context, pagination.limit)?;
        ensure_assignable(&Type::Int, &limit_ty).map_err(|e| {
            self.error(
                pagination.limit.span(),
                format!("{}::{}() limit invalido: {}", model, method, e),
            )
        })?;
        if let Some((value, span)) = pagination.limit.integer_literal() {
            if value <= 0 {
                return Err(self.error(
                    span,
                    format!("{}::{}() limit deve ser maior que zero", model, method),
                ));
            }
        }

        let offset_ty = self.infer_checked_model_operation_arg(context, pagination.offset)?;
        ensure_assignable(&Type::Int, &offset_ty).map_err(|e| {
            self.error(
                pagination.offset.span(),
                format!("{}::{}() offset invalido: {}", model, method, e),
            )
        })?;
        if let Some((value, span)) = pagination.offset.integer_literal() {
            if value < 0 {
                return Err(self.error(
                    span,
                    format!("{}::{}() offset nao pode ser negativo", model, method),
                ));
            }
        }

        Ok(())
    }
}
