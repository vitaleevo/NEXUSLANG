use std::collections::HashSet;

use crate::ast::{Expr, Field, Span, Type};
use crate::hir::HirSymbolKind;

use super::resolver::ResolvedProgram;
use super::{ensure_assignable, type_name, CheckResult, Checker, Scope};

impl Checker {
    pub(super) fn collect_model_declaration(
        &mut self,
        name: &str,
        fields: &[Field],
        span: Span,
        resolved: &ResolvedProgram<'_>,
    ) -> CheckResult<()> {
        if self.models.contains_key(name) {
            return Err(self.error(span, format!("Model '{}' declarado mais de uma vez", name)));
        }
        if reserved_openapi_component_name(name) {
            return Err(self.error(
                span,
                format!(
                    "Model '{}' usa nome reservado para componentes OpenAPI intenos: NexusError, NexusPage_* ou NexusList_*",
                    name
                ),
            ));
        }

        let mut seen_fields = HashSet::new();
        for field in fields {
            if !seen_fields.insert(field.name.as_str()) {
                return Err(self.error(
                    field.span,
                    format!("Campo '{}.{}' declarado mais de uma vez", name, field.name),
                ));
            }
        }

        self.models.insert(name.to_string(), fields.to_vec());
        self.symbols.set_top_level(
            HirSymbolKind::Model,
            name,
            resolved.top_level_symbol(HirSymbolKind::Model, name),
        );

        Ok(())
    }

    pub(super) fn check_model_declaration(&self, name: &str, fields: &[Field]) -> CheckResult<()> {
        let default_scope = Scope::default();

        for field in fields {
            self.ensure_known_type(&field.ty, field.span)
                .map_err(|diagnostic| {
                    self.error(
                        field.span,
                        format!("Campo '{}.{}': {}", name, field.name, diagnostic.message),
                    )
                })?;

            self.check_model_field_unique_constraint(name, field)?;
            self.check_model_field_index_constraint(name, field)?;
            self.produce_symbol_metadata(self.model_field_symbol(name, &field.name), &field.ty);

            if field.min.is_some() || field.max.is_some() {
                self.check_model_min_max_constraints(name, field, &default_scope)?;
            }

            if let Some(default) = &field.default {
                self.check_model_field_default(name, field, default, &default_scope)?;
            }
        }

        Ok(())
    }

    fn check_model_field_unique_constraint(
        &self,
        model_name: &str,
        field: &Field,
    ) -> CheckResult<()> {
        if field.unique && !unique_constraint_type_supported(&field.ty) {
            return Err(self.error(
                field.span,
                format!(
                    "Campo '{}.{}': unique so suporta string, int, float, bool, money, date ou opcionais desses tipos",
                    model_name, field.name
                ),
            ));
        }

        Ok(())
    }

    fn check_model_field_index_constraint(
        &self,
        model_name: &str,
        field: &Field,
    ) -> CheckResult<()> {
        if field.index && !index_constraint_type_supported(&field.ty) {
            return Err(self.error(
                field.span,
                format!(
                    "Campo '{}.{}': index so suporta string, int, float, bool, money, date ou opcionais desses tipos",
                    model_name, field.name
                ),
            ));
        }

        Ok(())
    }

    fn check_model_field_default(
        &self,
        model_name: &str,
        field: &Field,
        default: &Expr,
        scope: &Scope,
    ) -> CheckResult<()> {
        self.ensure_static_model_default(default)
            .map_err(|diagnostic| {
                let span = Span::new(
                    diagnostic.line.unwrap_or(field.span.line),
                    diagnostic.column.unwrap_or(field.span.column),
                );
                self.error(
                    span,
                    format!(
                        "Campo '{}.{}': {}",
                        model_name, field.name, diagnostic.message
                    ),
                )
            })?;

        let actual = self.infer_expr(default, scope)?;
        ensure_assignable(&field.ty, &actual).map_err(|e| {
            let span = if default.span().is_known() {
                default.span()
            } else {
                field.span
            };
            self.error(
                span,
                format!(
                    "Campo '{}.{}' default invalido: {}",
                    model_name, field.name, e
                ),
            )
        })?;

        validate_default_against_min_max(&field.ty, default, &field.min, &field.max).map_err(
            |message| {
                let span = if default.span().is_known() {
                    default.span()
                } else {
                    field.span
                };
                self.error(
                    span,
                    format!("Campo '{}.{}': {}", model_name, field.name, message),
                )
            },
        )
    }

    fn ensure_static_model_default(&self, expr: &Expr) -> CheckResult<()> {
        self.ensure_static_default_expr(expr, "default de model field")
    }

    fn check_model_min_max_constraints(
        &self,
        model_name: &str,
        field: &Field,
        scope: &Scope,
    ) -> CheckResult<()> {
        if !min_max_constraint_type_supported(&field.ty) {
            return Err(self.error(
                field.span,
                format!(
                    "Campo '{}.{}': min/max so suporta string, int, float, money, date ou opcionais desses tipos",
                    model_name, field.name
                ),
            ));
        }

        if let Some(min) = &field.min {
            self.check_model_min_max_bound(model_name, field, "min", min, scope)?;
        }
        if let Some(max) = &field.max {
            self.check_model_min_max_bound(model_name, field, "max", max, scope)?;
        }
        if let (Some(min), Some(max)) = (&field.min, &field.max) {
            ensure_min_max_bounds_ordered(&field.ty, min, max).map_err(|message| {
                self.error(
                    field.span,
                    format!("Campo '{}.{}': {}", model_name, field.name, message),
                )
            })?;
        }

        Ok(())
    }

    fn check_model_min_max_bound(
        &self,
        model_name: &str,
        field: &Field,
        constraint: &str,
        expr: &Expr,
        scope: &Scope,
    ) -> CheckResult<()> {
        self.ensure_static_min_max_expr(expr, constraint)?;
        let actual = self.infer_expr(expr, scope)?;
        ensure_min_max_bound_assignable(&field.ty, constraint, &actual).map_err(|message| {
            let span = if expr.span().is_known() {
                expr.span()
            } else {
                field.span
            };
            self.error(
                span,
                format!("Campo '{}.{}': {}", model_name, field.name, message),
            )
        })?;
        validate_min_max_bound_literal(&field.ty, constraint, expr).map_err(|message| {
            let span = if expr.span().is_known() {
                expr.span()
            } else {
                field.span
            };
            self.error(
                span,
                format!("Campo '{}.{}': {}", model_name, field.name, message),
            )
        })
    }

    fn ensure_static_min_max_expr(&self, expr: &Expr, constraint: &str) -> CheckResult<()> {
        match expr {
            Expr::Integer { .. }
            | Expr::Float { .. }
            | Expr::StringLit { .. }
            | Expr::Money { .. } => Ok(()),
            Expr::Array { span, .. }
            | Expr::Object { span, .. }
            | Expr::Bool { span, .. }
            | Expr::Nil { span }
            | Expr::Ident { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::BinOp { span, .. }
            | Expr::UnaryOp { span, .. }
            | Expr::Call { span, .. }
            | Expr::StaticCall { span, .. } => Err(self.error(
                *span,
                format!("constraint '{}' de model field nesta fase deve ser literal numerico, string ou money", constraint),
            )),
        }
    }
}

fn reserved_openapi_component_name(name: &str) -> bool {
    name == "NexusError" || name.starts_with("NexusPage_") || name.starts_with("NexusList_")
}

fn unique_constraint_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => unique_constraint_type_supported(inner),
        Type::String | Type::Int | Type::Float | Type::Bool | Type::Money | Type::Date => true,
        _ => false,
    }
}

fn index_constraint_type_supported(ty: &Type) -> bool {
    unique_constraint_type_supported(ty)
}

fn min_max_constraint_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => min_max_constraint_type_supported(inner),
        Type::String | Type::Int | Type::Float | Type::Money | Type::Date => true,
        _ => false,
    }
}

fn ensure_min_max_bound_assignable(
    field_ty: &Type,
    constraint: &str,
    actual: &Type,
) -> Result<(), String> {
    match field_ty {
        Type::Optional(inner) => ensure_min_max_bound_assignable(inner, constraint, actual),
        Type::String => {
            if matches!(actual, Type::Int) {
                Ok(())
            } else {
                Err(format!(
                    "{} em string espera int para tamanho, encontrado {}",
                    constraint,
                    type_name(actual)
                ))
            }
        }
        Type::Date => {
            if matches!(actual, Type::String) {
                Ok(())
            } else {
                Err(format!(
                    "{} em date espera string ISO, encontrado {}",
                    constraint,
                    type_name(actual)
                ))
            }
        }
        Type::Int => ensure_assignable(&Type::Int, actual)
            .map_err(|e| format!("{} invalido: {}", constraint, e)),
        Type::Float => ensure_assignable(&Type::Float, actual)
            .map_err(|e| format!("{} invalido: {}", constraint, e)),
        Type::Money => ensure_assignable(&Type::Money, actual)
            .map_err(|e| format!("{} invalido: {}", constraint, e)),
        _ => Err(format!(
            "{} nao suportado para {}",
            constraint,
            type_name(field_ty)
        )),
    }
}

fn validate_min_max_bound_literal(
    field_ty: &Type,
    constraint: &str,
    expr: &Expr,
) -> Result<(), String> {
    match field_ty {
        Type::Optional(inner) => validate_min_max_bound_literal(inner, constraint, expr),
        Type::String => {
            let Some(value) = integer_literal_value(expr) else {
                return Ok(());
            };
            if value < 0 {
                Err(format!("{} em string nao pode ser negativo", constraint))
            } else {
                Ok(())
            }
        }
        Type::Int | Type::Float | Type::Money | Type::Date => Ok(()),
        _ => Ok(()),
    }
}

fn validate_default_against_min_max(
    field_ty: &Type,
    default: &Expr,
    min: &Option<Expr>,
    max: &Option<Expr>,
) -> Result<(), String> {
    if matches!(default, Expr::Nil { .. }) {
        return Ok(());
    }

    if let Some(min) = min {
        validate_default_min_max_bound(field_ty, default, "min", min)?;
    }
    if let Some(max) = max {
        validate_default_min_max_bound(field_ty, default, "max", max)?;
    }

    Ok(())
}

fn validate_default_min_max_bound(
    field_ty: &Type,
    default: &Expr,
    constraint: &str,
    bound: &Expr,
) -> Result<(), String> {
    let op = if constraint == "min" { ">=" } else { "<=" };
    match field_ty {
        Type::Optional(inner) => validate_default_min_max_bound(inner, default, constraint, bound),
        Type::String => {
            let Some(value) = string_literal_value(default) else {
                return Ok(());
            };
            let Some(limit) = integer_literal_value(bound) else {
                return Ok(());
            };
            let length = value.chars().count() as i64;
            if (constraint == "min" && length >= limit) || (constraint == "max" && length <= limit)
            {
                Ok(())
            } else {
                Err(format!(
                    "default viola {}: tamanho deve ser {} {}",
                    constraint, op, limit
                ))
            }
        }
        Type::Int | Type::Float => {
            let Some(value) = numeric_literal_value(default) else {
                return Ok(());
            };
            let Some(limit) = numeric_literal_value(bound) else {
                return Ok(());
            };
            if (constraint == "min" && value >= limit) || (constraint == "max" && value <= limit) {
                Ok(())
            } else {
                Err(format!(
                    "default viola {}: valor deve ser {} {}",
                    constraint,
                    op,
                    format_number_for_check(limit)
                ))
            }
        }
        Type::Money => {
            let Some((value_amount, value_currency)) = money_literal_value(default) else {
                return Ok(());
            };
            let Some((limit_amount, limit_currency)) = money_literal_value(bound) else {
                return Ok(());
            };
            if value_currency != limit_currency {
                return Err(format!(
                    "default usa moeda {}, mas {} usa {}",
                    value_currency, constraint, limit_currency
                ));
            }
            if (constraint == "min" && value_amount >= limit_amount)
                || (constraint == "max" && value_amount <= limit_amount)
            {
                Ok(())
            } else {
                Err(format!(
                    "default viola {}: valor deve ser {} {} {}",
                    constraint,
                    op,
                    format_number_for_check(limit_amount),
                    limit_currency
                ))
            }
        }
        Type::Date => {
            let Some(value) = string_literal_value(default) else {
                return Ok(());
            };
            let Some(limit) = string_literal_value(bound) else {
                return Ok(());
            };
            if (constraint == "min" && value >= limit) || (constraint == "max" && value <= limit) {
                Ok(())
            } else {
                Err(format!(
                    "default viola {}: valor deve ser {} {}",
                    constraint, op, limit
                ))
            }
        }
        _ => Ok(()),
    }
}

fn ensure_min_max_bounds_ordered(field_ty: &Type, min: &Expr, max: &Expr) -> Result<(), String> {
    match field_ty {
        Type::Optional(inner) => ensure_min_max_bounds_ordered(inner, min, max),
        Type::String => {
            let Some(min) = integer_literal_value(min) else {
                return Ok(());
            };
            let Some(max) = integer_literal_value(max) else {
                return Ok(());
            };
            if min <= max {
                Ok(())
            } else {
                Err("min nao pode ser maior que max".to_string())
            }
        }
        Type::Int | Type::Float => {
            let Some(min) = numeric_literal_value(min) else {
                return Ok(());
            };
            let Some(max) = numeric_literal_value(max) else {
                return Ok(());
            };
            if min <= max {
                Ok(())
            } else {
                Err("min nao pode ser maior que max".to_string())
            }
        }
        Type::Money => {
            let Some((min_amount, min_currency)) = money_literal_value(min) else {
                return Ok(());
            };
            let Some((max_amount, max_currency)) = money_literal_value(max) else {
                return Ok(());
            };
            if min_currency != max_currency {
                return Err("min/max money devem usar a mesma moeda".to_string());
            }
            if min_amount <= max_amount {
                Ok(())
            } else {
                Err("min nao pode ser maior que max".to_string())
            }
        }
        Type::Date => {
            let Some(min) = string_literal_value(min) else {
                return Ok(());
            };
            let Some(max) = string_literal_value(max) else {
                return Ok(());
            };
            if min <= max {
                Ok(())
            } else {
                Err("min nao pode ser maior que max".to_string())
            }
        }
        _ => Ok(()),
    }
}

fn integer_literal_value(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Integer { value, .. } => Some(*value),
        _ => None,
    }
}

fn numeric_literal_value(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Integer { value, .. } => Some(*value as f64),
        Expr::Float { value, .. } => Some(*value),
        _ => None,
    }
}

fn money_literal_value(expr: &Expr) -> Option<(f64, &str)> {
    match expr {
        Expr::Money {
            value, currency, ..
        } => Some((*value, currency.as_str())),
        _ => None,
    }
}

fn string_literal_value(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::StringLit { value, .. } => Some(value.as_str()),
        _ => None,
    }
}

fn format_number_for_check(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        value.to_string()
    }
}
