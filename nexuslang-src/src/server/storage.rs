use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::ast::*;

use super::storage_backend::Storage;

#[derive(Debug, Clone)]
pub enum ServerValue {
    Json(String),
    Str(String),
    Number(f64),
    Bool(bool),
    Money(f64, String),
    Array(Vec<ServerValue>),
    Object(Vec<(String, ServerValue)>),
    Null,
}

impl ServerValue {
    pub fn display(&self) -> String {
        match self {
            ServerValue::Json(s) | ServerValue::Str(s) => s.clone(),
            ServerValue::Number(n) => format_number(*n),
            ServerValue::Bool(b) => b.to_string(),
            ServerValue::Money(amount, currency) => {
                format!("{} {}", format_number(*amount), currency)
            }
            ServerValue::Array(items) => {
                let parts = items
                    .iter()
                    .map(ServerValue::display)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", parts)
            }
            ServerValue::Object(fields) => {
                let parts = fields
                    .iter()
                    .map(|(name, value)| format!("{}: {}", name, value.display()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{}}}", parts)
            }
            ServerValue::Null => "null".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum JsonValue {
    Object(Vec<(String, JsonValue)>),
    Array(Vec<JsonValue>),
    String(String),
    Number(f64),
    Bool(bool),
    Null,
}

#[derive(Debug)]
pub struct RouteView<'a> {
    pub method: &'a HttpMethod,
    pub path: &'a str,
    pub params: &'a [String],
    pub query_params: &'a [QueryParam],
    pub body: &'a [Stmt],
}

#[derive(Debug, Clone, Copy)]
pub struct Pagination {
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone)]
pub struct ListOrdering {
    pub field: String,
    pub descending: bool,
}

#[derive(Debug, Clone)]
pub struct ModelFilter {
    pub field: String,
    pub expected: ServerValue,
}

#[derive(Debug, Clone, Copy)]
pub enum CompareOperator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, Clone, Copy)]
pub enum TextOperator {
    Contains,
    StartsWith,
    EndsWith,
    IContains,
    IStartsWith,
    IEndsWith,
}

#[derive(Debug, Clone)]
pub struct ListOptions {
    pub ordering: Option<ListOrdering>,
    pub pagination: Option<Pagination>,
}

pub(crate) trait OpenApiPath {
    fn replace_segments_for_openapi(self) -> String;
}

impl OpenApiPath for String {
    fn replace_segments_for_openapi(self) -> String {
        self.split('/')
            .map(|segment| {
                if let Some(param) = segment.strip_prefix('{') {
                    format!("{{{}}}", param)
                } else {
                    segment.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("/")
    }
}

pub(crate) fn routes(program: &Program) -> Vec<RouteView<'_>> {
    program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Route {
                method,
                path,
                params,
                query_params,
                body,
                ..
            } => Some(RouteView {
                method,
                path,
                params,
                query_params,
                body,
            }),
            _ => None,
        })
        .collect()
}

pub(crate) fn model_fields<'a>(program: &'a Program, model: &str) -> Option<&'a [Field]> {
    program.decls.iter().find_map(|decl| match decl {
        Decl::Model { name, fields, .. } if name == model => Some(fields.as_slice()),
        _ => None,
    })
}

pub(crate) fn type_is_optional(ty: &Type) -> bool {
    matches!(ty, Type::Optional(_))
}

pub(crate) fn non_optional_type(ty: &Type) -> &Type {
    match ty {
        Type::Optional(inner) => non_optional_type(inner),
        other => other,
    }
}

pub(crate) fn query_param_required(param: &QueryParam) -> bool {
    param.default.is_none() && !type_is_optional(&param.ty)
}

pub(crate) fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

pub(crate) fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

pub(crate) fn parse_json(source: &str) -> Result<JsonValue, String> {
    let mut parser = JsonParser::new(source);
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.is_done() {
        Ok(value)
    } else {
        Err("caracteres extras apos JSON".to_string())
    }
}

pub(crate) fn json_value_json(value: &JsonValue) -> String {
    match value {
        JsonValue::Object(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|(name, value)| format!(
                    r#""{}":{}"#,
                    escape_json(name),
                    json_value_json(value)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        JsonValue::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(json_value_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        JsonValue::String(value) => format!(r#""{}""#, escape_json(value)),
        JsonValue::Number(value) => format_number(*value),
        JsonValue::Bool(value) => value.to_string(),
        JsonValue::Null => "null".to_string(),
    }
}

pub(crate) fn json_type_label(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Object(_) => "object",
        JsonValue::Array(_) => "array",
        JsonValue::String(_) => "string",
        JsonValue::Number(_) => "number",
        JsonValue::Bool(_) => "bool",
        JsonValue::Null => "null",
    }
}

pub(crate) fn model_record_from_json(
    storage: &Storage,
    program: &Program,
    fields: &[Field],
    mut input: Vec<(String, JsonValue)>,
    context: &str,
) -> Result<ServerValue, String> {
    let declared = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();

    for (name, _) in &input {
        if !seen.insert(name.as_str()) {
            return Err(format!("{}: campo '{}' duplicado", context, name));
        }
        if !declared.contains(name.as_str()) {
            return Err(format!("{}: campo '{}' nao existe", context, name));
        }
    }

    let mut out = Vec::new();
    for field in fields {
        if let Some(pos) = input.iter().position(|(name, _)| name == &field.name) {
            let (_, value) = input.remove(pos);
            out.push((
                field.name.clone(),
                json_value_to_server_value(storage, program, &field.ty, value).map_err(
                    |message| format!("{}: campo '{}': {}", context, field.name, message),
                )?,
            ));
        } else if let Some(default) = &field.default {
            out.push((
                field.name.clone(),
                eval_default_value(storage, program, default)?,
            ));
        } else if type_is_optional(&field.ty) {
            out.push((field.name.clone(), ServerValue::Null));
        } else {
            return Err(format!(
                "{}: campo '{}' obrigatorio ausente",
                context, field.name
            ));
        }
    }

    Ok(ServerValue::Object(out))
}

pub(crate) fn eval_default_value(
    storage: &Storage,
    program: &Program,
    expr: &Expr,
) -> Result<ServerValue, String> {
    let params = HashMap::new();
    crate::server::router::eval_expr_value(expr, &params, storage, program, "", &HttpMethod::Get)
}

pub(crate) fn json_value_to_server_value(
    storage: &Storage,
    program: &Program,
    ty: &Type,
    value: JsonValue,
) -> Result<ServerValue, String> {
    match (ty, value) {
        (Type::Optional(_), JsonValue::Null) => Ok(ServerValue::Null),
        (Type::Optional(inner), value) => {
            json_value_to_server_value(storage, program, inner, value)
        }
        (Type::String, JsonValue::String(value)) | (Type::Date, JsonValue::String(value)) => {
            Ok(ServerValue::Str(value))
        }
        (Type::Int, JsonValue::Number(value)) if value.fract() == 0.0 => {
            Ok(ServerValue::Number(value))
        }
        (Type::Float, JsonValue::Number(value)) => Ok(ServerValue::Number(value)),
        (Type::Bool, JsonValue::Bool(value)) => Ok(ServerValue::Bool(value)),
        (Type::Money, JsonValue::Object(fields)) => money_from_json(fields),
        (Type::Array(inner), JsonValue::Array(items)) => {
            let mut out = Vec::new();
            for item in items {
                out.push(json_value_to_server_value(storage, program, inner, item)?);
            }
            Ok(ServerValue::Array(out))
        }
        (Type::Model(model), JsonValue::Object(fields)) => {
            let model_fields = model_fields(program, model)
                .ok_or_else(|| format!("Model '{}' nao encontrado", model))?;
            let context = format!("Objeto '{}'", model);
            model_record_from_json(storage, program, model_fields, fields, &context)
        }
        (_, JsonValue::Null) => Err(format!("esperado {}, encontrado null", type_label(ty))),
        (expected, actual) => Err(format!(
            "esperado {}, encontrado {}",
            type_label(expected),
            json_type_label(&actual)
        )),
    }
}

pub(crate) fn money_from_json(mut fields: Vec<(String, JsonValue)>) -> Result<ServerValue, String> {
    let amount = take_json_field(&mut fields, "amount")
        .ok_or_else(|| "money requer campo 'amount'".to_string())?;
    let currency = take_json_field(&mut fields, "currency")
        .ok_or_else(|| "money requer campo 'currency'".to_string())?;
    if let Some((name, _)) = fields.first() {
        return Err(format!("money nao aceita campo '{}'", name));
    }
    let JsonValue::Number(amount) = amount else {
        return Err("money.amount espera number".to_string());
    };
    let JsonValue::String(currency) = currency else {
        return Err("money.currency espera string".to_string());
    };
    Ok(ServerValue::Money(amount, currency))
}

pub(crate) fn take_json_field(
    fields: &mut Vec<(String, JsonValue)>,
    name: &str,
) -> Option<JsonValue> {
    let pos = fields.iter().position(|(field, _)| field == name)?;
    Some(fields.remove(pos).1)
}

pub(crate) fn server_value_json(value: ServerValue) -> String {
    match value {
        ServerValue::Json(json) => json,
        ServerValue::Str(s) => format!(r#""{}""#, escape_json(&s)),
        ServerValue::Number(n) => format_number(n),
        ServerValue::Bool(b) => b.to_string(),
        ServerValue::Money(amount, currency) => format!(
            r#"{{"amount":{},"currency":"{}"}}"#,
            format_number(amount),
            escape_json(&currency)
        ),
        ServerValue::Array(items) => format!(
            "[{}]",
            items
                .into_iter()
                .map(server_value_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        ServerValue::Object(fields) => format!(
            "{{{}}}",
            fields
                .into_iter()
                .map(|(name, value)| format!(
                    r#""{}":{}"#,
                    escape_json(&name),
                    server_value_json(value)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        ServerValue::Null => "null".to_string(),
    }
}

pub(crate) fn server_values_equal(left: &ServerValue, right: &ServerValue) -> bool {
    match (left, right) {
        (ServerValue::Str(a), ServerValue::Str(b)) => a == b,
        (ServerValue::Number(a), ServerValue::Number(b)) => (*a - *b).abs() < f64::EPSILON,
        (ServerValue::Bool(a), ServerValue::Bool(b)) => a == b,
        (ServerValue::Money(a_amount, a_currency), ServerValue::Money(b_amount, b_currency)) => {
            (*a_amount - *b_amount).abs() < f64::EPSILON && a_currency == b_currency
        }
        (ServerValue::Null, ServerValue::Null) => true,
        _ => false,
    }
}

pub(crate) fn server_values_compare(
    value: &ServerValue,
    operator: CompareOperator,
    expected: &ServerValue,
) -> bool {
    let ordering = match (value, expected) {
        (ServerValue::Number(a), ServerValue::Number(b)) => a.partial_cmp(b),
        (ServerValue::Str(a), ServerValue::Str(b)) => Some(a.cmp(b)),
        (ServerValue::Money(a_amount, a_currency), ServerValue::Money(b_amount, b_currency)) => {
            if a_currency != b_currency {
                return false;
            }
            Some(a_amount.partial_cmp(b_amount).unwrap_or(Ordering::Equal))
        }
        _ => None,
    };
    let Some(ordering) = ordering else {
        return false;
    };
    match operator {
        CompareOperator::Eq => ordering == Ordering::Equal,
        CompareOperator::Ne => ordering != Ordering::Equal,
        CompareOperator::Gt => ordering == Ordering::Greater,
        CompareOperator::Gte => ordering != Ordering::Less,
        CompareOperator::Lt => ordering == Ordering::Less,
        CompareOperator::Lte => ordering != Ordering::Greater,
    }
}

pub(crate) fn server_values_text_match(
    value: &ServerValue,
    operator: TextOperator,
    pattern: &ServerValue,
) -> bool {
    let ServerValue::Str(text) = value else {
        return false;
    };
    let ServerValue::Str(pattern) = pattern else {
        return false;
    };
    match operator {
        TextOperator::Contains => text.contains(pattern),
        TextOperator::StartsWith => text.starts_with(pattern),
        TextOperator::EndsWith => text.ends_with(pattern),
        TextOperator::IContains => text.to_lowercase().contains(&pattern.to_lowercase()),
        TextOperator::IStartsWith => text.to_lowercase().starts_with(&pattern.to_lowercase()),
        TextOperator::IEndsWith => text.to_lowercase().ends_with(&pattern.to_lowercase()),
    }
}

pub(crate) fn server_values_between(
    value: &ServerValue,
    min: &ServerValue,
    max: &ServerValue,
) -> bool {
    match (value, min, max) {
        (ServerValue::Number(v), ServerValue::Number(min), ServerValue::Number(max)) => {
            v >= min && v <= max
        }
        (ServerValue::Str(v), ServerValue::Str(min), ServerValue::Str(max)) => v >= min && v <= max,
        (
            ServerValue::Money(v_amount, v_currency),
            ServerValue::Money(min_amount, min_currency),
            ServerValue::Money(max_amount, max_currency),
        ) => {
            v_currency == min_currency
                && v_currency == max_currency
                && v_amount >= min_amount
                && v_amount <= max_amount
        }
        _ => false,
    }
}

pub(crate) fn type_label(ty: &Type) -> String {
    match ty {
        Type::String => "string".to_string(),
        Type::Int => "int".to_string(),
        Type::Float => "float".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Money => "money".to_string(),
        Type::Date => "date".to_string(),
        Type::Array(inner) => format!("[{}]", type_label(inner)),
        Type::Optional(inner) => format!("{}?", type_label(inner)),
        Type::Model(name) => name.clone(),
        Type::Nil => "nil".to_string(),
        Type::Void => "void".to_string(),
        Type::Unknown => "unknown".to_string(),
    }
}

pub(crate) fn server_object_field<'a>(
    value: &'a ServerValue,
    field_name: &str,
) -> Option<&'a ServerValue> {
    let ServerValue::Object(fields) = value else {
        return None;
    };
    fields
        .iter()
        .find(|(candidate, _)| candidate == field_name)
        .map(|(_, value)| value)
}

pub(crate) fn has_unique_fields(fields: &[Field]) -> bool {
    fields.iter().any(|field| field.unique)
}

pub(crate) fn ensure_unique_constraints(
    storage: &Storage,
    program: &Program,
    model: &str,
    fields: &[Field],
    record: &ServerValue,
    records: &[JsonValue],
    skip_index: Option<usize>,
) -> Result<(), String> {
    for field in fields.iter().filter(|field| field.unique) {
        let Some(candidate) = server_object_field(record, &field.name) else {
            continue;
        };
        for (index, stored_record) in records.iter().enumerate() {
            if skip_index == Some(index) {
                continue;
            }
            let JsonValue::Object(record_fields) = stored_record.clone() else {
                return Err(format!("Storage JSON de '{}' deve conter objetos", model));
            };
            let context = format!("Storage JSON de '{}'", model);
            let existing =
                model_record_from_json(storage, program, fields, record_fields, &context)?;
            let Some(existing_value) = server_object_field(&existing, &field.name) else {
                continue;
            };
            if server_values_equal(candidate, existing_value) {
                return Err(format!(
                    "Conflito: campo '{}.{}' unique ja existe com valor {}",
                    model,
                    field.name,
                    candidate.display()
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn ensure_min_max_constraints(
    storage: &Storage,
    program: &Program,
    model: &str,
    fields: &[Field],
    record: &ServerValue,
    context: &str,
) -> Result<(), String> {
    for field in fields
        .iter()
        .filter(|field| field.min.is_some() || field.max.is_some())
    {
        let Some(candidate) = server_object_field(record, &field.name) else {
            continue;
        };
        if matches!(candidate, ServerValue::Null) {
            continue;
        }

        if let Some(min) = &field.min {
            let bound = eval_default_value(storage, program, min).map_err(|message| {
                format!("{}: campo '{}.{}': {}", context, model, field.name, message)
            })?;
            ensure_min_max_bound(model, field, candidate, "min", &bound, context)?;
        }
        if let Some(max) = &field.max {
            let bound = eval_default_value(storage, program, max).map_err(|message| {
                format!("{}: campo '{}.{}': {}", context, model, field.name, message)
            })?;
            ensure_min_max_bound(model, field, candidate, "max", &bound, context)?;
        }
    }

    Ok(())
}

fn ensure_min_max_bound(
    model: &str,
    field: &Field,
    value: &ServerValue,
    constraint: &str,
    bound: &ServerValue,
    context: &str,
) -> Result<(), String> {
    let op = if constraint == "min" { ">=" } else { "<=" };
    match non_optional_type(&field.ty) {
        Type::String => {
            let (ServerValue::Str(text), ServerValue::Number(limit)) = (value, bound) else {
                return Ok(());
            };
            let length = text.chars().count() as f64;
            if (constraint == "min" && length >= *limit)
                || (constraint == "max" && length <= *limit)
            {
                Ok(())
            } else {
                Err(format!(
                    "{}: campo '{}.{}' deve ter tamanho {} {}",
                    context,
                    model,
                    field.name,
                    op,
                    format_number(*limit)
                ))
            }
        }
        Type::Int | Type::Float => {
            let (ServerValue::Number(value), ServerValue::Number(limit)) = (value, bound) else {
                return Ok(());
            };
            if (constraint == "min" && value >= limit) || (constraint == "max" && value <= limit) {
                Ok(())
            } else {
                Err(format!(
                    "{}: campo '{}.{}' deve ser {} {}",
                    context,
                    model,
                    field.name,
                    op,
                    format_number(*limit)
                ))
            }
        }
        Type::Money => {
            let (
                ServerValue::Money(value_amount, value_currency),
                ServerValue::Money(bound_amount, bound_currency),
            ) = (value, bound)
            else {
                return Ok(());
            };
            if value_currency != bound_currency {
                return Err(format!(
                    "{}: campo '{}.{}' deve usar moeda {} para {}",
                    context, model, field.name, bound_currency, constraint
                ));
            }
            if (constraint == "min" && value_amount >= bound_amount)
                || (constraint == "max" && value_amount <= bound_amount)
            {
                Ok(())
            } else {
                Err(format!(
                    "{}: campo '{}.{}' deve ser {} {} {}",
                    context,
                    model,
                    field.name,
                    op,
                    format_number(*bound_amount),
                    bound_currency
                ))
            }
        }
        Type::Date => {
            let (ServerValue::Str(value), ServerValue::Str(limit)) = (value, bound) else {
                return Ok(());
            };
            if (constraint == "min" && value >= limit) || (constraint == "max" && value <= limit) {
                Ok(())
            } else {
                Err(format!(
                    "{}: campo '{}.{}' deve ser {} {}",
                    context, model, field.name, op, limit
                ))
            }
        }
        _ => Ok(()),
    }
}

pub(crate) fn apply_ordering(
    mut items: Vec<ServerValue>,
    ordering: Option<&ListOrdering>,
) -> Result<Vec<ServerValue>, String> {
    let Some(ordering) = ordering else {
        return Ok(items);
    };
    let descending = ordering.descending;
    items.sort_by(|a, b| -> Ordering {
        let a_val = server_object_field(a, &ordering.field);
        let b_val = server_object_field(b, &ordering.field);
        match (a_val, b_val) {
            (Some(ServerValue::Str(a)), Some(ServerValue::Str(b))) => {
                if descending {
                    b.cmp(a)
                } else {
                    a.cmp(b)
                }
            }
            (Some(ServerValue::Number(a)), Some(ServerValue::Number(b))) => {
                if descending {
                    b.partial_cmp(a).unwrap_or(Ordering::Equal)
                } else {
                    a.partial_cmp(b).unwrap_or(Ordering::Equal)
                }
            }
            _ => Ordering::Equal,
        }
    });
    Ok(items)
}

pub(crate) fn apply_pagination(
    items: Vec<ServerValue>,
    pagination: Option<Pagination>,
) -> Vec<ServerValue> {
    match pagination {
        Some(Pagination { limit, offset }) => items.into_iter().skip(offset).take(limit).collect(),
        None => items,
    }
}

pub(crate) fn paginated_list_response(
    items: Vec<ServerValue>,
    ordering: Option<ListOrdering>,
    pagination: Option<Pagination>,
) -> Result<ServerValue, String> {
    let total = items.len();
    let items = apply_ordering(items, ordering.as_ref())?;
    let items = apply_pagination(items, pagination);
    Ok(ServerValue::Object(vec![
        ("total".to_string(), ServerValue::Number(total as f64)),
        ("items".to_string(), ServerValue::Array(items)),
    ]))
}

pub(crate) fn record_matches_filter(
    storage: &Storage,
    program: &Program,
    model: &str,
    fields: &[Field],
    record_fields: &[(String, JsonValue)],
    filter: &ModelFilter,
) -> Result<bool, String> {
    let field = fields
        .iter()
        .find(|candidate| candidate.name == filter.field)
        .ok_or_else(|| format!("Campo '{}.{}' nao existe", model, filter.field))?;
    let Some((_, stored_value)) = record_fields
        .iter()
        .find(|(candidate, _)| candidate == &filter.field)
    else {
        return Ok(false);
    };
    let stored = json_value_to_server_value(storage, program, &field.ty, stored_value.clone())
        .map_err(|message| {
            format!(
                "Storage JSON de '{}': campo '{}': {}",
                model, filter.field, message
            )
        })?;
    Ok(server_values_equal(&stored, &filter.expected))
}

pub(crate) fn eval_all_list_options(
    model: &str,
    args: &[Expr],
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<ListOptions, String> {
    match args.len() {
        2 if starts_ordering_args(args) => Ok(ListOptions {
            ordering: Some(parse_list_ordering(model, "all", &args[0], &args[1])?),
            pagination: None,
        }),
        2 => Ok(ListOptions {
            ordering: None,
            pagination: Some(eval_pagination(
                &args[0],
                &args[1],
                params,
                storage,
                program,
                request_body,
                route_method,
            )?),
        }),
        4 => Ok(ListOptions {
            ordering: Some(parse_list_ordering(model, "all", &args[0], &args[1])?),
            pagination: Some(eval_pagination(
                &args[2],
                &args[3],
                params,
                storage,
                program,
                request_body,
                route_method,
            )?),
        }),
        _ => Err(format!(
            "Requisicao invalida: {}::all() argumentos invalidos",
            model
        )),
    }
}

pub(crate) fn eval_page_list_options(
    model: &str,
    args: &[Expr],
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<ListOptions, String> {
    match args.len() {
        2 => Ok(ListOptions {
            ordering: None,
            pagination: Some(eval_pagination(
                &args[0],
                &args[1],
                params,
                storage,
                program,
                request_body,
                route_method,
            )?),
        }),
        4 => Ok(ListOptions {
            ordering: Some(parse_list_ordering(model, "page", &args[0], &args[1])?),
            pagination: Some(eval_pagination(
                &args[2],
                &args[3],
                params,
                storage,
                program,
                request_body,
                route_method,
            )?),
        }),
        _ => Err(format!(
            "Requisicao invalida: {}::page() argumentos invalidos",
            model
        )),
    }
}

pub(crate) fn eval_where_list_options(
    model: &str,
    method: &str,
    args: &[Expr],
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<ListOptions, String> {
    match args.len() {
        2 => Ok(ListOptions {
            ordering: None,
            pagination: None,
        }),
        4 if starts_ordering_args(&args[2..]) => Ok(ListOptions {
            ordering: Some(parse_list_ordering(model, method, &args[2], &args[3])?),
            pagination: None,
        }),
        4 => Ok(ListOptions {
            ordering: None,
            pagination: Some(eval_pagination(
                &args[2],
                &args[3],
                params,
                storage,
                program,
                request_body,
                route_method,
            )?),
        }),
        6 => Ok(ListOptions {
            ordering: Some(parse_list_ordering(model, method, &args[2], &args[3])?),
            pagination: Some(eval_pagination(
                &args[4],
                &args[5],
                params,
                storage,
                program,
                request_body,
                route_method,
            )?),
        }),
        _ => Err(format!(
            "Requisicao invalida: {}::{}() argumentos invalidos",
            model, method
        )),
    }
}

pub(crate) fn eval_where_page_list_options(
    model: &str,
    method: &str,
    args: &[Expr],
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<ListOptions, String> {
    match args.len() {
        4 => Ok(ListOptions {
            ordering: None,
            pagination: Some(eval_pagination(
                &args[2],
                &args[3],
                params,
                storage,
                program,
                request_body,
                route_method,
            )?),
        }),
        6 => Ok(ListOptions {
            ordering: Some(parse_list_ordering(model, method, &args[2], &args[3])?),
            pagination: Some(eval_pagination(
                &args[4],
                &args[5],
                params,
                storage,
                program,
                request_body,
                route_method,
            )?),
        }),
        _ => Err(format!(
            "Requisicao invalida: {}::{}() argumentos invalidos",
            model, method
        )),
    }
}

pub(crate) fn eval_where_compare_list_options(
    model: &str,
    method: &str,
    args: &[Expr],
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<ListOptions, String> {
    match args.len() {
        3 => Ok(ListOptions {
            ordering: None,
            pagination: None,
        }),
        5 if starts_ordering_args(&args[3..]) => Ok(ListOptions {
            ordering: Some(parse_list_ordering(model, method, &args[3], &args[4])?),
            pagination: None,
        }),
        5 => Ok(ListOptions {
            ordering: None,
            pagination: Some(eval_pagination(
                &args[3],
                &args[4],
                params,
                storage,
                program,
                request_body,
                route_method,
            )?),
        }),
        7 => Ok(ListOptions {
            ordering: Some(parse_list_ordering(model, method, &args[3], &args[4])?),
            pagination: Some(eval_pagination(
                &args[5],
                &args[6],
                params,
                storage,
                program,
                request_body,
                route_method,
            )?),
        }),
        _ => Err(format!(
            "Requisicao invalida: {}::{}() argumentos invalidos",
            model, method
        )),
    }
}

pub(crate) fn eval_where_all_filters_and_options(
    model: &str,
    args: &[Expr],
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<(Vec<ModelFilter>, ListOptions), String> {
    let filter_arg_count = where_all_filter_arg_count(args).ok_or_else(|| {
        format!(
            "Requisicao invalida: {}::where_all() argumentos invalidos",
            model
        )
    })?;
    let mut filters = Vec::new();
    for pair in args[..filter_arg_count].chunks(2) {
        let field = match &pair[0] {
            Expr::StringLit { value, .. } => value.clone(),
            _ => {
                return Err(format!(
                    "Requisicao invalida: {}::where_all() espera campo string literal",
                    model
                ));
            }
        };
        let expected = crate::server::router::eval_expr_value(
            &pair[1],
            params,
            storage,
            program,
            request_body,
            route_method,
        )?;
        filters.push(ModelFilter { field, expected });
    }

    let ordering = if where_all_args_have_ordering(args) {
        let order_index = args.len() - 4;
        Some(parse_list_ordering(
            model,
            "where_all",
            &args[order_index],
            &args[order_index + 1],
        )?)
    } else {
        None
    };
    let pagination = if where_all_args_have_pagination(args) {
        let limit_index = args.len() - 2;
        Some(eval_pagination(
            &args[limit_index],
            &args[limit_index + 1],
            params,
            storage,
            program,
            request_body,
            route_method,
        )?)
    } else {
        None
    };

    Ok((
        filters,
        ListOptions {
            ordering,
            pagination,
        },
    ))
}

pub(crate) fn eval_where_all_page_filters_and_options(
    model: &str,
    args: &[Expr],
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<(Vec<ModelFilter>, ListOptions), String> {
    let filter_arg_count = where_all_page_filter_arg_count(args).ok_or_else(|| {
        format!(
            "Requisicao invalida: {}::where_all_page() argumentos invalidos",
            model
        )
    })?;
    let mut filters = Vec::new();
    for pair in args[..filter_arg_count].chunks(2) {
        let field = match &pair[0] {
            Expr::StringLit { value, .. } => value.clone(),
            _ => {
                return Err(format!(
                    "Requisicao invalida: {}::where_all_page() espera campo string literal",
                    model
                ));
            }
        };
        let expected = crate::server::router::eval_expr_value(
            &pair[1],
            params,
            storage,
            program,
            request_body,
            route_method,
        )?;
        filters.push(ModelFilter { field, expected });
    }

    let ordering = if where_all_args_have_ordering(args) {
        let order_index = args.len() - 4;
        Some(parse_list_ordering(
            model,
            "where_all_page",
            &args[order_index],
            &args[order_index + 1],
        )?)
    } else {
        None
    };
    let limit_index = args.len() - 2;
    let pagination = Some(eval_pagination(
        &args[limit_index],
        &args[limit_index + 1],
        params,
        storage,
        program,
        request_body,
        route_method,
    )?);

    Ok((
        filters,
        ListOptions {
            ordering,
            pagination,
        },
    ))
}

pub(crate) fn eval_where_any_filters_and_options(
    model: &str,
    args: &[Expr],
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<(Vec<ModelFilter>, ListOptions), String> {
    let filter_arg_count = where_all_filter_arg_count(args).ok_or_else(|| {
        format!(
            "Requisicao invalida: {}::where_any() argumentos invalidos",
            model
        )
    })?;
    let mut filters = Vec::new();
    for pair in args[..filter_arg_count].chunks(2) {
        let field = match &pair[0] {
            Expr::StringLit { value, .. } => value.clone(),
            _ => {
                return Err(format!(
                    "Requisicao invalida: {}::where_any() espera campo string literal",
                    model
                ));
            }
        };
        let expected = crate::server::router::eval_expr_value(
            &pair[1],
            params,
            storage,
            program,
            request_body,
            route_method,
        )?;
        filters.push(ModelFilter { field, expected });
    }

    let ordering = if where_all_args_have_ordering(args) {
        let order_index = args.len() - 4;
        Some(parse_list_ordering(
            model,
            "where_any",
            &args[order_index],
            &args[order_index + 1],
        )?)
    } else {
        None
    };
    let pagination = if where_all_args_have_pagination(args) {
        let limit_index = args.len() - 2;
        Some(eval_pagination(
            &args[limit_index],
            &args[limit_index + 1],
            params,
            storage,
            program,
            request_body,
            route_method,
        )?)
    } else {
        None
    };

    Ok((
        filters,
        ListOptions {
            ordering,
            pagination,
        },
    ))
}

pub(crate) fn eval_where_any_page_filters_and_options(
    model: &str,
    args: &[Expr],
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<(Vec<ModelFilter>, ListOptions), String> {
    let filter_arg_count = where_all_page_filter_arg_count(args).ok_or_else(|| {
        format!(
            "Requisicao invalida: {}::where_any_page() argumentos invalidos",
            model
        )
    })?;
    let mut filters = Vec::new();
    for pair in args[..filter_arg_count].chunks(2) {
        let field = match &pair[0] {
            Expr::StringLit { value, .. } => value.clone(),
            _ => {
                return Err(format!(
                    "Requisicao invalida: {}::where_any_page() espera campo string literal",
                    model
                ));
            }
        };
        let expected = crate::server::router::eval_expr_value(
            &pair[1],
            params,
            storage,
            program,
            request_body,
            route_method,
        )?;
        filters.push(ModelFilter { field, expected });
    }

    let ordering = if where_all_args_have_ordering(args) {
        let order_index = args.len() - 4;
        Some(parse_list_ordering(
            model,
            "where_any_page",
            &args[order_index],
            &args[order_index + 1],
        )?)
    } else {
        None
    };
    let limit_index = args.len() - 2;
    let pagination = Some(eval_pagination(
        &args[limit_index],
        &args[limit_index + 1],
        params,
        storage,
        program,
        request_body,
        route_method,
    )?);

    Ok((
        filters,
        ListOptions {
            ordering,
            pagination,
        },
    ))
}

pub(crate) fn where_all_filter_arg_count(args: &[Expr]) -> Option<usize> {
    let mut count = 0;
    let mut pos = 0;
    while pos + 1 < args.len() {
        match &args[pos] {
            Expr::StringLit { .. } => {
                if pos + 3 < args.len() {
                    if let Expr::StringLit { value, .. } = &args[pos + 1] {
                        if (value == "asc" || value == "desc")
                            && !matches!(&args[pos + 2], Expr::StringLit { .. })
                        {
                            break;
                        }
                    }
                }
                count += 1;
                pos += 2;
            }
            _ => break,
        }
    }
    if count == 0 {
        None
    } else {
        Some(count * 2)
    }
}

pub(crate) fn where_all_page_filter_arg_count(args: &[Expr]) -> Option<usize> {
    if args.len() < 6 || !where_all_args_have_pagination(args) {
        return None;
    }
    let filter_arg_count = if where_all_args_have_ordering(args) {
        args.len() - 4
    } else {
        args.len() - 2
    };
    if filter_arg_count >= 4 && filter_arg_count % 2 == 0 {
        Some(filter_arg_count)
    } else {
        None
    }
}

pub(crate) fn starts_ordering_args(args: &[Expr]) -> bool {
    matches!(&args[0], Expr::StringLit { .. })
}

pub(crate) fn where_all_args_have_ordering(args: &[Expr]) -> bool {
    args.len() >= 8
        && matches!(&args[args.len() - 4], Expr::StringLit { .. })
        && matches!(&args[args.len() - 3], Expr::StringLit { value, .. } if value == "asc" || value == "desc")
        && !matches!(&args[args.len() - 2], Expr::StringLit { .. })
}

pub(crate) fn where_all_args_have_pagination(args: &[Expr]) -> bool {
    args.len() >= 6 && !matches!(&args[args.len() - 2], Expr::StringLit { .. })
}

pub(crate) fn parse_list_ordering(
    model: &str,
    method: &str,
    field_expr: &Expr,
    direction_expr: &Expr,
) -> Result<ListOrdering, String> {
    let field = match field_expr {
        Expr::StringLit { value, .. } => value.clone(),
        _ => {
            return Err(format!(
                "Requisicao invalida: {}::{}() espera campo string literal",
                model, method
            ));
        }
    };
    let direction = match direction_expr {
        Expr::StringLit { value, .. } => value.as_str(),
        _ => {
            return Err(format!(
                "Requisicao invalida: {}::{}() espera direcao string literal",
                model, method
            ));
        }
    };
    let descending = direction == "desc";
    if direction != "asc" && direction != "desc" {
        return Err(format!(
            "Requisicao invalida: {}::{}() direcao deve ser 'asc' ou 'desc'",
            model, method
        ));
    }
    Ok(ListOrdering { field, descending })
}

pub(crate) fn eval_pagination(
    limit_expr: &Expr,
    offset_expr: &Expr,
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<Pagination, String> {
    let limit = crate::server::router::eval_expr_value(
        limit_expr,
        params,
        storage,
        program,
        request_body,
        route_method,
    )?;
    let offset = crate::server::router::eval_expr_value(
        offset_expr,
        params,
        storage,
        program,
        request_body,
        route_method,
    )?;
    let ServerValue::Number(limit) = limit else {
        return Err("Paginacao: limit espera numero".to_string());
    };
    let ServerValue::Number(offset) = offset else {
        return Err("Paginacao: offset espera numero".to_string());
    };
    Ok(Pagination {
        limit: limit as usize,
        offset: offset as usize,
    })
}

pub(crate) fn parse_compare_operator(
    model: &str,
    operator: &str,
) -> Result<CompareOperator, String> {
    match operator {
        "eq" | "==" => Ok(CompareOperator::Eq),
        "ne" | "!=" => Ok(CompareOperator::Ne),
        "gt" | ">" => Ok(CompareOperator::Gt),
        "gte" | ">=" => Ok(CompareOperator::Gte),
        "lt" | "<" => Ok(CompareOperator::Lt),
        "lte" | "<=" => Ok(CompareOperator::Lte),
        _ => Err(format!(
            "Requisicao invalida: {}::where_compare() operador '{}' invalido",
            model, operator
        )),
    }
}

pub(crate) fn parse_text_operator(model: &str, operator: &str) -> Result<TextOperator, String> {
    match operator {
        "contains" => Ok(TextOperator::Contains),
        "starts_with" => Ok(TextOperator::StartsWith),
        "ends_with" => Ok(TextOperator::EndsWith),
        "icontains" => Ok(TextOperator::IContains),
        "istarts_with" => Ok(TextOperator::IStartsWith),
        "iends_with" => Ok(TextOperator::IEndsWith),
        _ => Err(format!(
            "Requisicao invalida: {}::where_text() operador '{}' invalido",
            model, operator
        )),
    }
}

pub(crate) fn eval_binop(
    left: ServerValue,
    op: &BinOp,
    right: ServerValue,
) -> Result<ServerValue, String> {
    match op {
        BinOp::Add => match (left, right) {
            (ServerValue::Str(a), b) => Ok(ServerValue::Str(format!("{}{}", a, b.display()))),
            (a, ServerValue::Str(b)) => Ok(ServerValue::Str(format!("{}{}", a.display(), b))),
            (ServerValue::Number(a), ServerValue::Number(b)) => Ok(ServerValue::Number(a + b)),
            _ => Err("Operação + inválida em route HTTP".to_string()),
        },
        _ => Err("Apenas + é suportado em expressões HTTP nesta fase".to_string()),
    }
}

pub(crate) fn parse_query_string(query: &str) -> Result<HashMap<String, String>, String> {
    let mut values = HashMap::new();
    if query.is_empty() {
        return Ok(values);
    }

    for part in query.split('&') {
        if part.is_empty() {
            continue;
        }
        let (name, value) = part.split_once('=').unwrap_or((part, ""));
        values.insert(
            decode_query_component(name)?,
            decode_query_component(value)?,
        );
    }

    Ok(values)
}

fn decode_query_component(value: &str) -> Result<String, String> {
    decode_percent_component(value, true, "query")
}

fn decode_path_component(value: &str) -> Result<String, String> {
    decode_percent_component(value, false, "path")
}

pub(crate) fn validate_path_encoding(path: &str) -> Result<(), String> {
    for part in split_path(path) {
        decode_path_component(part)?;
    }
    Ok(())
}

fn decode_percent_component(
    value: &str,
    plus_as_space: bool,
    context: &str,
) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut pos = 0;
    while pos < bytes.len() {
        match bytes[pos] {
            b'+' if plus_as_space => {
                out.push(b' ');
                pos += 1;
            }
            b'%' => {
                if pos + 2 >= bytes.len() {
                    return Err(format!(
                        "Requisicao invalida: escape de {} incompleto",
                        context
                    ));
                }
                let hi = hex_value(bytes[pos + 1]).ok_or_else(|| {
                    format!("Requisicao invalida: escape de {} invalido", context)
                })?;
                let lo = hex_value(bytes[pos + 2]).ok_or_else(|| {
                    format!("Requisicao invalida: escape de {} invalido", context)
                })?;
                out.push((hi << 4) | lo);
                pos += 3;
            }
            byte => {
                out.push(byte);
                pos += 1;
            }
        }
    }

    String::from_utf8(out)
        .map_err(|_| format!("Requisicao invalida: escape de {} invalido", context))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(crate) fn match_route(
    pattern: &str,
    path: &str,
) -> Result<Option<HashMap<String, ServerValue>>, String> {
    let pattern_parts = split_path(pattern);
    let path_parts = split_path(path);
    if pattern_parts.len() != path_parts.len() {
        return Ok(None);
    }

    let mut params = HashMap::new();
    for (pattern, value) in pattern_parts.iter().zip(path_parts.iter()) {
        let decoded_value = decode_path_component(value)?;
        if let Some(name) = pattern.strip_prefix(':') {
            params.insert(name.to_string(), ServerValue::Str(decoded_value));
        } else if *pattern != decoded_value {
            return Ok(None);
        }
    }

    Ok(Some(params))
}

fn split_path(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect()
}

struct JsonParser {
    chars: Vec<char>,
    pos: usize,
}

impl JsonParser {
    fn new(source: &str) -> Self {
        JsonParser {
            chars: source.chars().collect(),
            pos: 0,
        }
    }

    fn is_done(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        Some(ch)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(' ' | '\n' | '\r' | '\t')) {
            self.pos += 1;
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => self.parse_string().map(JsonValue::String),
            Some('t') => {
                self.consume_literal("true")?;
                Ok(JsonValue::Bool(true))
            }
            Some('f') => {
                self.consume_literal("false")?;
                Ok(JsonValue::Bool(false))
            }
            Some('n') => {
                self.consume_literal("null")?;
                Ok(JsonValue::Null)
            }
            Some('-' | '0'..='9') => self.parse_number().map(JsonValue::Number),
            Some(ch) => Err(format!("valor JSON inesperado '{}'", ch)),
            None => Err("corpo JSON vazio".to_string()),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.expect('{')?;
        let mut fields = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some('}') {
            self.advance();
            return Ok(JsonValue::Object(fields));
        }

        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(':')?;
            let value = self.parse_value()?;
            fields.push((key, value));
            self.skip_whitespace();
            match self.advance() {
                Some(',') => {}
                Some('}') => return Ok(JsonValue::Object(fields)),
                Some(ch) => return Err(format!("esperado ',' ou '}}', encontrado '{}'", ch)),
                None => return Err("objeto JSON nao terminado".to_string()),
            }
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.expect('[')?;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(']') {
            self.advance();
            return Ok(JsonValue::Array(items));
        }

        loop {
            items.push(self.parse_value()?);
            self.skip_whitespace();
            match self.advance() {
                Some(',') => {}
                Some(']') => return Ok(JsonValue::Array(items)),
                Some(ch) => return Err(format!("esperado ',' ou ']', encontrado '{}'", ch)),
                None => return Err("array JSON nao terminado".to_string()),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            let Some(ch) = self.advance() else {
                return Err("string JSON nao terminada".to_string());
            };
            match ch {
                '"' => return Ok(out),
                '\\' => out.push(self.parse_escape()?),
                ch => out.push(ch),
            }
        }
    }

    fn parse_escape(&mut self) -> Result<char, String> {
        match self.advance() {
            Some('"') => Ok('"'),
            Some('\\') => Ok('\\'),
            Some('/') => Ok('/'),
            Some('b') => Ok('\u{08}'),
            Some('f') => Ok('\u{0c}'),
            Some('n') => Ok('\n'),
            Some('r') => Ok('\r'),
            Some('t') => Ok('\t'),
            Some('u') => {
                let mut value = 0_u32;
                for _ in 0..4 {
                    let Some(ch) = self.advance() else {
                        return Err("escape unicode incompleto".to_string());
                    };
                    value = value * 16
                        + ch.to_digit(16)
                            .ok_or_else(|| "escape unicode invalido".to_string())?;
                }
                char::from_u32(value).ok_or_else(|| "escape unicode invalido".to_string())
            }
            Some(ch) => Err(format!("escape JSON invalido '\\{}'", ch)),
            None => Err("escape JSON incompleto".to_string()),
        }
    }

    fn parse_number(&mut self) -> Result<f64, String> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.advance();
        }
        self.consume_digits();
        if self.peek() == Some('.') {
            self.advance();
            self.consume_digits();
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            self.advance();
            if matches!(self.peek(), Some('+' | '-')) {
                self.advance();
            }
            self.consume_digits();
        }
        self.chars[start..self.pos]
            .iter()
            .collect::<String>()
            .parse::<f64>()
            .map_err(|_| "numero JSON invalido".to_string())
    }

    fn consume_digits(&mut self) {
        while matches!(self.peek(), Some('0'..='9')) {
            self.advance();
        }
    }

    fn consume_literal(&mut self, literal: &str) -> Result<(), String> {
        for expected in literal.chars() {
            match self.advance() {
                Some(ch) if ch == expected => {}
                _ => return Err(format!("literal JSON '{}' invalido", literal)),
            }
        }
        Ok(())
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        match self.advance() {
            Some(ch) if ch == expected => Ok(()),
            Some(ch) => Err(format!("esperado '{}', encontrado '{}'", expected, ch)),
            None => Err(format!("esperado '{}'", expected)),
        }
    }
}
