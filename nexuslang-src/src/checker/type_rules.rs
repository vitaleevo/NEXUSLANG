use crate::ast::{BinOp, Type};
use crate::hir;

use super::{ensure_assignable, type_name};

pub(super) fn hir_binary_op_to_ast(op: hir::HirBinaryOp) -> BinOp {
    match op {
        hir::HirBinaryOp::Add => BinOp::Add,
        hir::HirBinaryOp::Sub => BinOp::Sub,
        hir::HirBinaryOp::Mul => BinOp::Mul,
        hir::HirBinaryOp::Div => BinOp::Div,
        hir::HirBinaryOp::Mod => BinOp::Mod,
        hir::HirBinaryOp::Eq => BinOp::Eq,
        hir::HirBinaryOp::NotEq => BinOp::NotEq,
        hir::HirBinaryOp::Lt => BinOp::Lt,
        hir::HirBinaryOp::LtEq => BinOp::LtEq,
        hir::HirBinaryOp::Gt => BinOp::Gt,
        hir::HirBinaryOp::GtEq => BinOp::GtEq,
        hir::HirBinaryOp::And => BinOp::And,
        hir::HirBinaryOp::Or => BinOp::Or,
    }
}

pub(super) fn ordering_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => ordering_type_supported(inner),
        Type::String | Type::Int | Type::Float | Type::Bool | Type::Money | Type::Date => true,
        _ => false,
    }
}

pub(super) fn comparison_operator_supported(operator: &str) -> bool {
    matches!(operator, "==" | "!=" | ">" | ">=" | "<" | "<=")
}

pub(super) fn ensure_comparison_operator_allowed(
    operator: &str,
    field_ty: &Type,
    actual_ty: &Type,
) -> Result<(), String> {
    if matches!(operator, "==" | "!=") {
        if comparison_equality_type_supported(field_ty) {
            return Ok(());
        }
    } else {
        if matches!(actual_ty, Type::Nil) {
            return Err(format!("operador '{}' nao aceita nil", operator));
        }
        if comparison_order_type_supported(field_ty) {
            return Ok(());
        }
    }

    Err(format!(
        "campo do tipo {} nao suporta operador '{}'",
        type_name(field_ty),
        operator
    ))
}

pub(super) fn comparison_equality_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => comparison_equality_type_supported(inner),
        Type::String | Type::Int | Type::Float | Type::Bool | Type::Money | Type::Date => true,
        _ => false,
    }
}

pub(super) fn comparison_order_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => comparison_order_type_supported(inner),
        Type::String | Type::Int | Type::Float | Type::Money | Type::Date => true,
        _ => false,
    }
}

pub(super) fn text_operator_supported(operator: &str) -> bool {
    matches!(
        operator,
        "contains" | "starts_with" | "ends_with" | "icontains" | "istarts_with" | "iends_with"
    )
}

pub(super) fn text_filter_type_supported(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => text_filter_type_supported(inner),
        Type::String => true,
        _ => false,
    }
}

#[allow(dead_code)]
pub(super) fn infer_std_builtin_call_type(
    name: &str,
    arg_types: &[Type],
) -> Option<Result<Type, String>> {
    let result = match name {
        "__std_string_contains" | "__std_string_starts_with" | "__std_string_ends_with" => {
            expect_builtin_args(name, arg_types, &[Type::String, Type::String], Type::Bool)
        }
        "__std_string_to_upper" | "__std_string_to_lower" | "__std_string_trim" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::String)
        }

        "__std_array_contains_int" => expect_builtin_args(
            name,
            arg_types,
            &[Type::Array(Box::new(Type::Int)), Type::Int],
            Type::Bool,
        ),
        "__std_array_contains_string" => expect_builtin_args(
            name,
            arg_types,
            &[Type::Array(Box::new(Type::String)), Type::String],
            Type::Bool,
        ),
        "__std_array_first_int" | "__std_array_last_int" => expect_builtin_args(
            name,
            arg_types,
            &[Type::Array(Box::new(Type::Int))],
            Type::Int,
        ),
        "__std_array_first_string" | "__std_array_last_string" => expect_builtin_args(
            name,
            arg_types,
            &[Type::Array(Box::new(Type::String))],
            Type::String,
        ),
        "__std_array_reverse_int" => expect_builtin_args(
            name,
            arg_types,
            &[Type::Array(Box::new(Type::Int))],
            Type::Array(Box::new(Type::Int)),
        ),
        "__std_array_reverse_string" => expect_builtin_args(
            name,
            arg_types,
            &[Type::Array(Box::new(Type::String))],
            Type::Array(Box::new(Type::String)),
        ),

        "__std_validation_is_blank" | "__std_validation_is_email" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::Bool)
        }

        "__std_json_escape" | "__std_json_string" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::String)
        }
        "__std_json_is_object" | "__std_json_is_array" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::Bool)
        }

        "__std_csv_needs_quotes" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::Bool)
        }
        "__std_csv_escape_cell" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::String)
        }

        "__std_http_status_text" => {
            expect_builtin_args(name, arg_types, &[Type::Int], Type::String)
        }
        "__std_http_method_allows_body" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::Bool)
        }
        "__std_http_url_encode" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::String)
        }

        "__std_crypto_sha256_hex" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::String)
        }
        "__std_crypto_constant_time_eq" => {
            expect_builtin_args(name, arg_types, &[Type::String, Type::String], Type::Bool)
        }
        "__std_crypto_is_sha256_hex" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::Bool)
        }

        "__std_time_runtime_clock_available" => {
            expect_builtin_args(name, arg_types, &[], Type::Bool)
        }
        "__std_time_unix_seconds" | "__std_time_unix_millis" => {
            expect_builtin_args(name, arg_types, &[], Type::Int)
        }

        "__std_env_runtime_available" => expect_builtin_args(name, arg_types, &[], Type::Bool),
        "__std_env_get" => expect_builtin_args(name, arg_types, &[Type::String], Type::String),
        "__std_env_has" => expect_builtin_args(name, arg_types, &[Type::String], Type::Bool),

        "__std_path_join" => {
            expect_builtin_args(name, arg_types, &[Type::String, Type::String], Type::String)
        }
        "__std_path_basename"
        | "__std_path_dirname"
        | "__std_path_extension"
        | "__std_path_stem"
        | "__std_path_normalize" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::String)
        }
        "__std_path_is_absolute" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::Bool)
        }

        "__std_date_is_iso_date" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::Bool)
        }
        "__std_date_year" | "__std_date_month" | "__std_date_day" => {
            expect_builtin_args(name, arg_types, &[Type::String], Type::Int)
        }

        "__std_money_is_positive" | "__std_money_is_zero" => {
            expect_builtin_args(name, arg_types, &[Type::Money], Type::Bool)
        }
        "__std_money_same_currency" => {
            expect_builtin_args(name, arg_types, &[Type::Money, Type::Money], Type::Bool)
        }

        _ => return None,
    };

    Some(result)
}

pub(super) fn infer_test_assert_call_type(
    name: &str,
    arg_types: &[Type],
) -> Option<Result<Type, String>> {
    let result = match name {
        "assert_true" => infer_assert_true_call_type(name, arg_types),
        "assert_eq" => infer_assert_eq_call_type(name, arg_types),
        "assert_ne" => infer_assert_ne_call_type(name, arg_types),
        "assert_contains" => infer_assert_contains_call_type(name, arg_types),
        _ => return None,
    };

    Some(result)
}

fn infer_assert_true_call_type(name: &str, arg_types: &[Type]) -> Result<Type, String> {
    if !(1..=2).contains(&arg_types.len()) {
        return Err(format!(
            "{} espera 1 ou 2 argumento(s), recebeu {}",
            name,
            arg_types.len()
        ));
    }

    ensure_assignable(&Type::Bool, &arg_types[0]).map_err(|message| {
        format!(
            "argumento 1 invalido em {}: esperado bool, encontrado {} ({})",
            name,
            type_name(&arg_types[0]),
            message
        )
    })?;

    ensure_optional_assert_message(name, arg_types, 1)?;

    Ok(Type::Void)
}

fn infer_assert_eq_call_type(name: &str, arg_types: &[Type]) -> Result<Type, String> {
    infer_assert_pair_call_type(name, arg_types)
}

fn infer_assert_ne_call_type(name: &str, arg_types: &[Type]) -> Result<Type, String> {
    infer_assert_pair_call_type(name, arg_types)
}

fn infer_assert_pair_call_type(name: &str, arg_types: &[Type]) -> Result<Type, String> {
    if !(2..=3).contains(&arg_types.len()) {
        return Err(format!(
            "{} espera 2 ou 3 argumento(s), recebeu {}",
            name,
            arg_types.len()
        ));
    }

    ensure_assignable(&arg_types[1], &arg_types[0]).map_err(|message| {
        format!(
            "argumentos incompatíveis em {}: esperado {}, encontrado {} ({})",
            name,
            type_name(&arg_types[1]),
            type_name(&arg_types[0]),
            message
        )
    })?;

    ensure_optional_assert_message(name, arg_types, 2)?;

    Ok(Type::Void)
}

fn infer_assert_contains_call_type(name: &str, arg_types: &[Type]) -> Result<Type, String> {
    if !(2..=3).contains(&arg_types.len()) {
        return Err(format!(
            "{} espera 2 ou 3 argumento(s), recebeu {}",
            name,
            arg_types.len()
        ));
    }

    match &arg_types[0] {
        Type::String => ensure_assignable(&Type::String, &arg_types[1]).map_err(|message| {
            format!(
                "valor procurado invalido em {}: esperado string, encontrado {} ({})",
                name,
                type_name(&arg_types[1]),
                message
            )
        })?,
        Type::Array(inner) => ensure_assignable(inner, &arg_types[1]).map_err(|message| {
            format!(
                "valor procurado invalido em {}: esperado {}, encontrado {} ({})",
                name,
                type_name(inner),
                type_name(&arg_types[1]),
                message
            )
        })?,
        Type::Unknown => {}
        other => {
            return Err(format!(
                "{} espera string ou array no argumento 1, encontrado {}",
                name,
                type_name(other)
            ));
        }
    }

    ensure_optional_assert_message(name, arg_types, 2)?;

    Ok(Type::Void)
}

fn ensure_optional_assert_message(
    name: &str,
    arg_types: &[Type],
    index: usize,
) -> Result<(), String> {
    let Some(message_type) = arg_types.get(index) else {
        return Ok(());
    };

    ensure_assignable(&Type::String, message_type).map_err(|message| {
        format!(
            "mensagem invalida em {}: esperado string, encontrado {} ({})",
            name,
            type_name(message_type),
            message
        )
    })
}

fn expect_builtin_args(
    name: &str,
    actual: &[Type],
    expected: &[Type],
    return_type: Type,
) -> Result<Type, String> {
    if actual.len() != expected.len() {
        return Err(format!(
            "{} espera {} argumento(s), recebeu {}",
            name,
            expected.len(),
            actual.len()
        ));
    }

    for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
        ensure_assignable(expected, actual).map_err(|message| {
            format!("argumento {} invalido em {}: {}", index + 1, name, message)
        })?;
    }

    Ok(return_type)
}
