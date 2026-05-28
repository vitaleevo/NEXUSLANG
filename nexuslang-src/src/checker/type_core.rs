use crate::ast::Type;

pub(super) fn ensure_assignable(expected: &Type, actual: &Type) -> Result<(), String> {
    if *expected == Type::Unknown || *actual == Type::Unknown || expected == actual {
        return Ok(());
    }

    match (expected, actual) {
        (Type::Optional(_), Type::Nil) => Ok(()),
        (Type::Optional(expected_inner), Type::Optional(actual_inner)) => {
            ensure_assignable(expected_inner, actual_inner)
        }
        (Type::Optional(inner), actual) => ensure_assignable(inner, actual),
        (Type::Float, Type::Int) => Ok(()),
        (Type::Array(a), Type::Array(b)) => ensure_assignable(a, b),
        _ => Err(format!(
            "esperado {}, encontrado {}",
            type_name(expected),
            type_name(actual)
        )),
    }
}

pub(super) fn type_name(ty: &Type) -> String {
    match ty {
        Type::String => "string".to_string(),
        Type::Int => "int".to_string(),
        Type::Float => "float".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Money => "money".to_string(),
        Type::Date => "date".to_string(),
        Type::Array(inner) => format!("[{}]", type_name(inner)),
        Type::Optional(inner) => format!("{}?", type_name(inner)),
        Type::Model(name) => name.clone(),
        Type::Nil => "nil".to_string(),
        Type::Void => "void".to_string(),
        Type::Unknown => "unknown".to_string(),
    }
}
