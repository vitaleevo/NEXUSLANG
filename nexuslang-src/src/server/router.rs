use std::collections::HashMap;

use crate::ast::*;

use super::auth::{self, AuthenticatedUser};
use super::http::{json_response, method_name, route_error_status, HttpResponse};
use super::storage::*;
use super::storage_backend::Storage;

pub(crate) fn handle_request(
    program: &Program,
    storage: &Storage,
    method: &str,
    path: &str,
    request_body: &str,
) -> HttpResponse {
    handle_request_with_headers(program, storage, method, path, &[], request_body)
}

pub(crate) fn handle_request_with_headers(
    program: &Program,
    storage: &Storage,
    method: &str,
    path: &str,
    headers: &[(String, String)],
    request_body: &str,
) -> HttpResponse {
    let (clean_path, query_string) = path.split_once('?').unwrap_or((path, ""));
    if let Err(e) = validate_path_encoding(clean_path) {
        return json_response(400, format!(r#"{{"error":"{}"}}"#, escape_json(&e)));
    }

    if method == "GET" && clean_path == "/openapi.json" {
        return json_response(200, crate::server::openapi::generate_openapi(program));
    }

    if method == "GET" && clean_path == "/__health" {
        return json_response(200, r#"{"status":"ok"}"#.to_string());
    }

    let mut best_match = None;
    for route in routes(program) {
        if method_name(route.method) != method {
            continue;
        }
        match match_route(route.path, clean_path) {
            Ok(Some(params)) => {
                let specificity = route_path_specificity(route.path);
                if best_match
                    .as_ref()
                    .is_none_or(|(_, _, best_specificity)| specificity > *best_specificity)
                {
                    best_match = Some((route, params, specificity));
                }
            }
            Ok(None) => {}
            Err(e) => return json_response(400, format!(r#"{{"error":"{}"}}"#, escape_json(&e))),
        }
    }

    if let Some((route, mut params, _)) = best_match {
        let query_values = match parse_query_string(query_string) {
            Ok(values) => values,
            Err(e) => return json_response(400, format!(r#"{{"error":"{}"}}"#, escape_json(&e))),
        };
        if let Err(e) = bind_query_params(&route, &query_values, &mut params, storage, program) {
            return json_response(
                route_error_status(&e),
                format!(r#"{{"error":"{}"}}"#, escape_json(&e)),
            );
        }
        let auth_context = match authenticate_route(program, storage, &route, headers) {
            Ok(user) => user,
            Err(e) => {
                return json_response(
                    route_error_status(&e),
                    format!(r#"{{"error":"{}"}}"#, escape_json(&e)),
                )
            }
        };
        return match eval_route(
            &route,
            &params,
            storage,
            program,
            headers,
            request_body,
            auth_context.as_ref(),
        ) {
            Ok(response) => response,
            Err(e) => json_response(
                route_error_status(&e),
                format!(r#"{{"error":"{}"}}"#, escape_json(&e)),
            ),
        };
    }

    json_response(404, r#"{"error":"route not found"}"#.to_string())
}

fn authenticate_route(
    program: &Program,
    storage: &Storage,
    route: &RouteView<'_>,
    headers: &[(String, String)],
) -> Result<Option<AuthenticatedUser>, String> {
    match route.auth {
        Some(guard) => auth::authenticate_request(program, storage, guard, headers).map(Some),
        None => Ok(None),
    }
}

fn route_path_specificity(path: &str) -> usize {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty() && !segment.starts_with(':'))
        .count()
}

fn bind_query_params(
    route: &RouteView<'_>,
    query_values: &HashMap<String, String>,
    params: &mut HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
) -> Result<(), String> {
    for param in route.query_params {
        if let Some(raw) = query_values.get(&param.name) {
            params.insert(
                param.name.clone(),
                query_value_to_server_value(&param.name, &param.ty, raw)?,
            );
        } else if let Some(default) = &param.default {
            params.insert(
                param.name.clone(),
                eval_default_value(storage, program, default)?,
            );
        } else if type_is_optional(&param.ty) {
            params.insert(param.name.clone(), ServerValue::Null);
        } else {
            return Err(format!(
                "Requisicao invalida: query param '{}' ausente",
                param.name
            ));
        }
    }
    Ok(())
}

fn query_value_to_server_value(name: &str, ty: &Type, raw: &str) -> Result<ServerValue, String> {
    match ty {
        Type::Optional(inner) => query_value_to_server_value(name, inner, raw),
        Type::Array(inner) => parse_query_array(name, inner, raw),
        Type::String | Type::Date => Ok(ServerValue::Str(raw.to_string())),
        Type::Int => {
            let value = raw
                .parse::<i64>()
                .map_err(|_| format!("Requisicao invalida: query param '{}' espera int", name))?;
            Ok(ServerValue::Number(value as f64))
        }
        Type::Float => {
            let value = raw
                .parse::<f64>()
                .map_err(|_| format!("Requisicao invalida: query param '{}' espera float", name))?;
            if !value.is_finite() {
                return Err(format!(
                    "Requisicao invalida: query param '{}' espera float finito",
                    name
                ));
            }
            Ok(ServerValue::Number(value))
        }
        Type::Money => parse_query_money(name, raw),
        Type::Bool => match raw {
            "true" => Ok(ServerValue::Bool(true)),
            "false" => Ok(ServerValue::Bool(false)),
            _ => Err(format!(
                "Requisicao invalida: query param '{}' espera bool",
                name
            )),
        },
        _ => Err(format!(
            "Requisicao invalida: query param '{}' usa tipo nao suportado",
            name
        )),
    }
}

fn parse_query_array(name: &str, item_ty: &Type, raw: &str) -> Result<ServerValue, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(ServerValue::Array(Vec::new()));
    }

    let mut items = Vec::new();
    for item in raw.split(',') {
        let item = item.trim();
        if item.is_empty() {
            return Err(format!(
                "Requisicao invalida: query param '{}' espera array separado por virgula sem itens vazios",
                name
            ));
        }
        items.push(query_value_to_server_value(name, item_ty, item)?);
    }

    Ok(ServerValue::Array(items))
}

fn parse_query_money(name: &str, raw: &str) -> Result<ServerValue, String> {
    let raw = raw.trim();
    let invalid = || {
        format!(
            "Requisicao invalida: query param '{}' espera money no formato amount:currency",
            name
        )
    };

    let (amount, currency) = if let Some((amount, currency)) = raw.split_once(':') {
        (amount.trim(), currency.trim())
    } else {
        let mut parts = raw.split_whitespace();
        let Some(amount) = parts.next() else {
            return Err(invalid());
        };
        let Some(currency) = parts.next() else {
            return Err(invalid());
        };
        if parts.next().is_some() {
            return Err(invalid());
        }
        (amount, currency)
    };

    if amount.is_empty()
        || currency.is_empty()
        || !currency.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        return Err(invalid());
    }

    let amount = amount.parse::<f64>().map_err(|_| invalid())?;
    if !amount.is_finite() {
        return Err(invalid());
    }

    Ok(ServerValue::Money(amount, currency.to_string()))
}

fn is_model_create_call(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::StaticCall {
            method,
            args,
            ..
        } if method == "create" && args.is_empty()
    )
}

fn eval_route(
    route: &RouteView<'_>,
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    headers: &[(String, String)],
    request_body: &str,
    auth_context: Option<&AuthenticatedUser>,
) -> Result<HttpResponse, String> {
    for stmt in route.body {
        match stmt {
            Stmt::Return { value, .. } => {
                if let Some(auth_response) = auth::eval_auth_return(
                    value,
                    program,
                    storage,
                    headers,
                    request_body,
                    auth_context,
                ) {
                    let auth_response = auth_response?;
                    return Ok(HttpResponse {
                        status: auth_response.status,
                        content_type: "application/json",
                        body: auth_response.body,
                        headers: auth_response.headers,
                    });
                }
                let body =
                    eval_expr_json(value, params, storage, program, request_body, route.method)?;
                let status =
                    if matches!(route.method, HttpMethod::Post) && is_model_create_call(value) {
                        201
                    } else {
                        200
                    };
                return Ok(json_response(status, body));
            }
            Stmt::Print { .. } | Stmt::ExprStmt { .. } | Stmt::Let { .. } | Stmt::Const { .. } => {}
            Stmt::Assign { .. } | Stmt::If { .. } | Stmt::While { .. } | Stmt::For { .. } => {}
        }
    }

    Ok(json_response(200, "null".to_string()))
}

fn eval_expr_json(
    expr: &Expr,
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<String, String> {
    Ok(server_value_json(eval_expr_value(
        expr,
        params,
        storage,
        program,
        request_body,
        route_method,
    )?))
}

pub(crate) fn eval_expr_value(
    expr: &Expr,
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<ServerValue, String> {
    match expr {
        Expr::Integer { value, .. } => Ok(ServerValue::Number(*value as f64)),
        Expr::Float { value, .. } => Ok(ServerValue::Number(*value)),
        Expr::StringLit { value, .. } => Ok(ServerValue::Str(value.clone())),
        Expr::Bool { value, .. } => Ok(ServerValue::Bool(*value)),
        Expr::Money {
            value, currency, ..
        } => Ok(ServerValue::Money(*value, currency.clone())),
        Expr::Array { items, .. } => {
            let mut out = Vec::new();
            for item in items {
                out.push(eval_expr_value(
                    item,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?);
            }
            Ok(ServerValue::Array(out))
        }
        Expr::Object { model, fields, .. } => {
            let mut out = Vec::new();
            for field in fields {
                out.push((
                    field.name.clone(),
                    eval_expr_value(
                        &field.value,
                        params,
                        storage,
                        program,
                        request_body,
                        route_method,
                    )?,
                ));
            }
            if let Some(model_fields) = model_fields(program, model) {
                let mut ordered = Vec::new();
                for field in model_fields {
                    if let Some(pos) = out.iter().position(|(name, _)| name == &field.name) {
                        ordered.push(out.remove(pos));
                    } else if let Some(default) = &field.default {
                        ordered.push((
                            field.name.clone(),
                            eval_expr_value(
                                default,
                                params,
                                storage,
                                program,
                                request_body,
                                route_method,
                            )?,
                        ));
                    } else if type_is_optional(&field.ty) {
                        ordered.push((field.name.clone(), ServerValue::Null));
                    }
                }
                ordered.extend(out);
                out = ordered;
            }
            Ok(ServerValue::Object(out))
        }
        Expr::FieldAccess { object, field, .. } => {
            match eval_expr_value(object, params, storage, program, request_body, route_method)? {
                ServerValue::Object(fields) => fields
                    .into_iter()
                    .find(|(name, _)| name == field)
                    .map(|(_, value)| value)
                    .ok_or_else(|| format!("Campo '{}' nao encontrado", field)),
                other => Err(format!(
                    "Acesso a campo '{}' espera objeto HTTP, encontrado {}",
                    field,
                    other.display()
                )),
            }
        }
        Expr::Nil { .. } => Ok(ServerValue::Null),
        Expr::Ident { name, .. } => params
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Parâmetro '{}' não encontrado", name)),
        Expr::BinOp {
            left, op, right, ..
        } => {
            let left = eval_expr_value(left, params, storage, program, request_body, route_method)?;
            let right =
                eval_expr_value(right, params, storage, program, request_body, route_method)?;
            eval_binop(left, op, right)
        }
        Expr::UnaryOp { .. } => {
            Err("Operação unária ainda não suportada em route HTTP".to_string())
        }
        Expr::Call { name, args, .. } if name == "str" && args.len() == 1 => Ok(ServerValue::Str(
            eval_expr_value(
                &args[0],
                params,
                storage,
                program,
                request_body,
                route_method,
            )?
            .display(),
        )),
        Expr::Call { name, .. } => Err(format!("Função '{}' não suportada em route HTTP", name)),
        Expr::StaticCall {
            ty, method, args, ..
        } => {
            if method == "all" && args.is_empty() {
                return Ok(ServerValue::Json(storage.read_model_raw_json(ty)?));
            }
            if method == "all" && (args.len() == 2 || args.len() == 4) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!(
                        "{}::all() com argumentos so pode ser usado em GET",
                        ty
                    ));
                }
                let options = eval_all_list_options(
                    ty,
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.list_model_records(
                    program,
                    ty,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "page" && (args.len() == 2 || args.len() == 4) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::page() so pode ser usado em GET", ty));
                }
                let options = eval_page_list_options(
                    ty,
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.list_model_records_page(
                    program,
                    ty,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "create" && args.is_empty() {
                if !matches!(route_method, HttpMethod::Post) {
                    return Err(format!("{}::create() so pode ser usado em POST", ty));
                }
                return storage.create_model_record(program, ty, request_body);
            }
            if method == "find" && args.len() == 2 {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::find() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => return Err(format!("{}::find() espera campo string literal", ty)),
                };
                let expected = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.find_model_record(program, ty, field, &expected);
            }
            if method == "where" && (args.len() == 2 || args.len() == 4 || args.len() == 6) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => return Err(format!("{}::where() espera campo string literal", ty)),
                };
                let expected = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_list_options(
                    ty,
                    "where",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.filter_model_records(
                    program,
                    ty,
                    field,
                    &expected,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_page" && (args.len() == 4 || args.len() == 6) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_page() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => return Err(format!("{}::where_page() espera campo string literal", ty)),
                };
                let expected = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_page_list_options(
                    ty,
                    "where_page",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.filter_model_records_page(
                    program,
                    ty,
                    field,
                    &expected,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_not" && (args.len() == 2 || args.len() == 4 || args.len() == 6) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_not() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => return Err(format!("{}::where_not() espera campo string literal", ty)),
                };
                let expected = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_list_options(
                    ty,
                    "where_not",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.filter_model_records_not(
                    program,
                    ty,
                    field,
                    &expected,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_not_page" && (args.len() == 4 || args.len() == 6) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_not_page() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_not_page() espera campo string literal",
                            ty
                        ));
                    }
                };
                let expected = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_page_list_options(
                    ty,
                    "where_not_page",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ListOptions {
                    ordering,
                    pagination,
                } = options;
                let items = storage
                    .filter_model_records_not(program, ty, field, &expected, ordering, None)?;
                return storage.paginated_array_response(items, pagination);
            }
            if method == "where_not_in" && (args.len() == 2 || args.len() == 4 || args.len() == 6) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_not_in() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_not_in() espera campo string literal",
                            ty
                        ));
                    }
                };
                let values = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ServerValue::Array(values) = values else {
                    return Err(format!(
                        "Requisicao invalida: {}::where_not_in() espera array de valores",
                        ty
                    ));
                };
                let options = eval_where_list_options(
                    ty,
                    "where_not_in",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.filter_model_records_by_not_in(
                    program,
                    ty,
                    field,
                    &values,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_not_in_page" && (args.len() == 4 || args.len() == 6) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!(
                        "{}::where_not_in_page() so pode ser usado em GET",
                        ty
                    ));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_not_in_page() espera campo string literal",
                            ty
                        ));
                    }
                };
                let values = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ServerValue::Array(values) = values else {
                    return Err(format!(
                        "Requisicao invalida: {}::where_not_in_page() espera array de valores",
                        ty
                    ));
                };
                let options = eval_where_page_list_options(
                    ty,
                    "where_not_in_page",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ListOptions {
                    ordering,
                    pagination,
                } = options;
                let items = storage
                    .filter_model_records_by_not_in(program, ty, field, &values, ordering, None)?;
                return storage.paginated_array_response(items, pagination);
            }
            if method == "where_not_in_optional"
                && (args.len() == 2 || args.len() == 4 || args.len() == 6)
            {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!(
                        "{}::where_not_in_optional() so pode ser usado em GET",
                        ty
                    ));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_not_in_optional() espera campo string literal",
                            ty
                        ));
                    }
                };
                let values = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_list_options(
                    ty,
                    "where_not_in_optional",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                if matches!(values, ServerValue::Null) {
                    return storage.list_model_records(
                        program,
                        ty,
                        options.ordering,
                        options.pagination,
                    );
                }
                let ServerValue::Array(values) = values else {
                    return Err(format!(
                        "Requisicao invalida: {}::where_not_in_optional() espera array opcional de valores",
                        ty
                    ));
                };
                return storage.filter_model_records_by_not_in(
                    program,
                    ty,
                    field,
                    &values,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_not_in_optional_page" && (args.len() == 4 || args.len() == 6) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!(
                        "{}::where_not_in_optional_page() so pode ser usado em GET",
                        ty
                    ));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_not_in_optional_page() espera campo string literal",
                            ty
                        ));
                    }
                };
                let values = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_page_list_options(
                    ty,
                    "where_not_in_optional_page",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ListOptions {
                    ordering,
                    pagination,
                } = options;
                if matches!(values, ServerValue::Null) {
                    return storage.list_model_records_page(program, ty, ordering, pagination);
                }
                let ServerValue::Array(values) = values else {
                    return Err(format!(
                        "Requisicao invalida: {}::where_not_in_optional_page() espera array opcional de valores",
                        ty
                    ));
                };
                let items = storage
                    .filter_model_records_by_not_in(program, ty, field, &values, ordering, None)?;
                return storage.paginated_array_response(items, pagination);
            }
            if method == "where_optional" && (args.len() == 2 || args.len() == 4 || args.len() == 6)
            {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_optional() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_optional() espera campo string literal",
                            ty
                        ));
                    }
                };
                let expected = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_list_options(
                    ty,
                    "where_optional",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                if matches!(expected, ServerValue::Null) {
                    return storage.list_model_records(
                        program,
                        ty,
                        options.ordering,
                        options.pagination,
                    );
                }
                return storage.filter_model_records(
                    program,
                    ty,
                    field,
                    &expected,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_optional_page" && (args.len() == 4 || args.len() == 6) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!(
                        "{}::where_optional_page() so pode ser usado em GET",
                        ty
                    ));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_optional_page() espera campo string literal",
                            ty
                        ));
                    }
                };
                let expected = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_page_list_options(
                    ty,
                    "where_optional_page",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                if matches!(expected, ServerValue::Null) {
                    return storage.list_model_records_page(
                        program,
                        ty,
                        options.ordering,
                        options.pagination,
                    );
                }
                return storage.filter_model_records_page(
                    program,
                    ty,
                    field,
                    &expected,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_in" && (args.len() == 2 || args.len() == 4 || args.len() == 6) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_in() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => return Err(format!("{}::where_in() espera campo string literal", ty)),
                };
                let values = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ServerValue::Array(values) = values else {
                    return Err(format!(
                        "Requisicao invalida: {}::where_in() espera array de valores",
                        ty
                    ));
                };
                let options = eval_where_list_options(
                    ty,
                    "where_in",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.filter_model_records_by_in(
                    program,
                    ty,
                    field,
                    &values,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_in_page" && (args.len() == 4 || args.len() == 6) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_in_page() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_in_page() espera campo string literal",
                            ty
                        ));
                    }
                };
                let values = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ServerValue::Array(values) = values else {
                    return Err(format!(
                        "Requisicao invalida: {}::where_in_page() espera array de valores",
                        ty
                    ));
                };
                let options = eval_where_page_list_options(
                    ty,
                    "where_in_page",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ListOptions {
                    ordering,
                    pagination,
                } = options;
                let items = storage
                    .filter_model_records_by_in(program, ty, field, &values, ordering, None)?;
                return storage.paginated_array_response(items, pagination);
            }
            if method == "where_in_optional"
                && (args.len() == 2 || args.len() == 4 || args.len() == 6)
            {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!(
                        "{}::where_in_optional() so pode ser usado em GET",
                        ty
                    ));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_in_optional() espera campo string literal",
                            ty
                        ));
                    }
                };
                let values = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_list_options(
                    ty,
                    "where_in_optional",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                if matches!(values, ServerValue::Null) {
                    return storage.list_model_records(
                        program,
                        ty,
                        options.ordering,
                        options.pagination,
                    );
                }
                let ServerValue::Array(values) = values else {
                    return Err(format!(
                        "Requisicao invalida: {}::where_in_optional() espera array opcional de valores",
                        ty
                    ));
                };
                return storage.filter_model_records_by_in(
                    program,
                    ty,
                    field,
                    &values,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_in_optional_page" && (args.len() == 4 || args.len() == 6) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!(
                        "{}::where_in_optional_page() so pode ser usado em GET",
                        ty
                    ));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_in_optional_page() espera campo string literal",
                            ty
                        ));
                    }
                };
                let values = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_page_list_options(
                    ty,
                    "where_in_optional_page",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ListOptions {
                    ordering,
                    pagination,
                } = options;
                if matches!(values, ServerValue::Null) {
                    return storage.list_model_records_page(program, ty, ordering, pagination);
                }
                let ServerValue::Array(values) = values else {
                    return Err(format!(
                        "Requisicao invalida: {}::where_in_optional_page() espera array opcional de valores",
                        ty
                    ));
                };
                let items = storage
                    .filter_model_records_by_in(program, ty, field, &values, ordering, None)?;
                return storage.paginated_array_response(items, pagination);
            }
            if method == "where_compare" && (args.len() == 3 || args.len() == 5 || args.len() == 7)
            {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_compare() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_compare() espera campo string literal",
                            ty
                        ));
                    }
                };
                let operator = match &args[1] {
                    Expr::StringLit { value, .. } => parse_compare_operator(ty, value)?,
                    _ => {
                        return Err(format!(
                            "{}::where_compare() espera operador string literal",
                            ty
                        ));
                    }
                };
                let expected = eval_expr_value(
                    &args[2],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_compare_list_options(
                    ty,
                    "where_compare",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.filter_model_records_by_comparison(
                    program,
                    ty,
                    field,
                    operator,
                    &expected,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_compare_page" && (args.len() == 5 || args.len() == 7) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!(
                        "{}::where_compare_page() so pode ser usado em GET",
                        ty
                    ));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_compare_page() espera campo string literal",
                            ty
                        ));
                    }
                };
                let operator = match &args[1] {
                    Expr::StringLit { value, .. } => parse_compare_operator(ty, value)?,
                    _ => {
                        return Err(format!(
                            "{}::where_compare_page() espera operador string literal",
                            ty
                        ));
                    }
                };
                let expected = eval_expr_value(
                    &args[2],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_compare_list_options(
                    ty,
                    "where_compare_page",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ListOptions {
                    ordering,
                    pagination,
                } = options;
                let items = storage.filter_model_records_by_comparison(
                    program, ty, field, operator, &expected, ordering, None,
                )?;
                return storage.paginated_array_response(items, pagination);
            }
            if method == "where_text" && (args.len() == 3 || args.len() == 5 || args.len() == 7) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_text() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => return Err(format!("{}::where_text() espera campo string literal", ty)),
                };
                let operator = match &args[1] {
                    Expr::StringLit { value, .. } => parse_text_operator(ty, value)?,
                    _ => {
                        return Err(format!(
                            "{}::where_text() espera operador textual string literal",
                            ty
                        ));
                    }
                };
                let expected = eval_expr_value(
                    &args[2],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_compare_list_options(
                    ty,
                    "where_text",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.filter_model_records_by_text(
                    program,
                    ty,
                    field,
                    operator,
                    &expected,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_text_page" && (args.len() == 5 || args.len() == 7) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!(
                        "{}::where_text_page() so pode ser usado em GET",
                        ty
                    ));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_text_page() espera campo string literal",
                            ty
                        ))
                    }
                };
                let operator = match &args[1] {
                    Expr::StringLit { value, .. } => parse_text_operator(ty, value)?,
                    _ => {
                        return Err(format!(
                            "{}::where_text_page() espera operador textual string literal",
                            ty
                        ));
                    }
                };
                let expected = eval_expr_value(
                    &args[2],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_compare_list_options(
                    ty,
                    "where_text_page",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ListOptions {
                    ordering,
                    pagination,
                } = options;
                let items = storage.filter_model_records_by_text(
                    program, ty, field, operator, &expected, ordering, None,
                )?;
                return storage.paginated_array_response(items, pagination);
            }
            if method == "where_between" && (args.len() == 3 || args.len() == 5 || args.len() == 7)
            {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_between() so pode ser usado em GET", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_between() espera campo string literal",
                            ty
                        ));
                    }
                };
                let min = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let max = eval_expr_value(
                    &args[2],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_compare_list_options(
                    ty,
                    "where_between",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.filter_model_records_by_range(
                    program,
                    ty,
                    field,
                    &min,
                    &max,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_between_page" && (args.len() == 5 || args.len() == 7) {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!(
                        "{}::where_between_page() so pode ser usado em GET",
                        ty
                    ));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => {
                        return Err(format!(
                            "{}::where_between_page() espera campo string literal",
                            ty
                        ));
                    }
                };
                let min = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let max = eval_expr_value(
                    &args[2],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let options = eval_where_compare_list_options(
                    ty,
                    "where_between_page",
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ListOptions {
                    ordering,
                    pagination,
                } = options;
                let items = storage.filter_model_records_by_range(
                    program, ty, field, &min, &max, ordering, None,
                )?;
                return storage.paginated_array_response(items, pagination);
            }
            if method == "where_all" && where_all_filter_arg_count(args).is_some() {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_all() so pode ser usado em GET", ty));
                }
                let (filters, options) = eval_where_all_filters_and_options(
                    ty,
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.filter_model_records_by_filters(
                    program,
                    ty,
                    &filters,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_all_page" && where_all_page_filter_arg_count(args).is_some() {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_all_page() so pode ser usado em GET", ty));
                }
                let (filters, options) = eval_where_all_page_filters_and_options(
                    ty,
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ListOptions {
                    ordering,
                    pagination,
                } = options;
                let items = storage
                    .filter_model_records_by_filters(program, ty, &filters, ordering, None)?;
                return storage.paginated_array_response(items, pagination);
            }
            if method == "where_any" && where_all_filter_arg_count(args).is_some() {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_any() so pode ser usado em GET", ty));
                }
                let (filters, options) = eval_where_any_filters_and_options(
                    ty,
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.filter_model_records_by_any_filters(
                    program,
                    ty,
                    &filters,
                    options.ordering,
                    options.pagination,
                );
            }
            if method == "where_any_page" && where_all_page_filter_arg_count(args).is_some() {
                if !matches!(route_method, HttpMethod::Get) {
                    return Err(format!("{}::where_any_page() so pode ser usado em GET", ty));
                }
                let (filters, options) = eval_where_any_page_filters_and_options(
                    ty,
                    args,
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                let ListOptions {
                    ordering,
                    pagination,
                } = options;
                let items = storage
                    .filter_model_records_by_any_filters(program, ty, &filters, ordering, None)?;
                return storage.paginated_array_response(items, pagination);
            }
            if method == "update" && args.len() == 2 {
                if !matches!(route_method, HttpMethod::Put) {
                    return Err(format!("{}::update() so pode ser usado em PUT", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => return Err(format!("{}::update() espera campo string literal", ty)),
                };
                let expected = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.update_model_record(program, ty, field, &expected, request_body);
            }
            if method == "delete" && args.len() == 2 {
                if !matches!(route_method, HttpMethod::Delete) {
                    return Err(format!("{}::delete() so pode ser usado em DELETE", ty));
                }
                let field = match &args[0] {
                    Expr::StringLit { value, .. } => value,
                    _ => return Err(format!("{}::delete() espera campo string literal", ty)),
                };
                let expected = eval_expr_value(
                    &args[1],
                    params,
                    storage,
                    program,
                    request_body,
                    route_method,
                )?;
                return storage.delete_model_record(program, ty, field, &expected);
            }
            Err(format!(
                "Static call '{}::{}' nao suportada em HTTP",
                ty, method
            ))
        }
    }
}
