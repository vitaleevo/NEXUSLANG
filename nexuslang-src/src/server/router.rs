use std::collections::HashMap;

use crate::ast::*;
use crate::model_ops::{
    CheckedModelOperationArgs, ModelOperationStorageCategory, ModelStaticOperation,
};
use crate::route_hir::{checked_routes, CheckedRouteExpr, CheckedRouteView};

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
    for route in checked_routes(program) {
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
        let auth_context = match authenticate_route(program, storage, &route, method, headers) {
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
    route: &CheckedRouteView<'_>,
    method: &str,
    headers: &[(String, String)],
) -> Result<Option<AuthenticatedUser>, String> {
    match route.auth {
        Some(guard) => {
            auth::authenticate_request(program, storage, guard, method, headers).map(Some)
        }
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
    route: &CheckedRouteView<'_>,
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

fn eval_route(
    route: &CheckedRouteView<'_>,
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    headers: &[(String, String)],
    request_body: &str,
    auth_context: Option<&AuthenticatedUser>,
) -> Result<HttpResponse, String> {
    let Some(return_expr) = &route.return_expr else {
        return Ok(json_response(200, "null".to_string()));
    };

    if let CheckedRouteExpr::AuthOperation(auth_operation) = return_expr {
        let Some(checked_args) = auth_operation.checked_args else {
            return Err(auth_operation.operation.argument_error(auth_operation.args));
        };
        let auth_response = auth::eval_checked_auth_return(
            auth_operation.operation,
            checked_args,
            program,
            storage,
            headers,
            request_body,
            auth_context,
        )?;
        return Ok(HttpResponse {
            status: auth_response.status,
            content_type: "application/json",
            body: auth_response.body,
            headers: auth_response.headers,
        });
    }

    let body = eval_checked_expr_json(
        return_expr,
        params,
        storage,
        program,
        request_body,
        route.method,
    )?;
    let status = if matches!(route.method, HttpMethod::Post)
        && matches!(
            return_expr,
            CheckedRouteExpr::ModelOperation(operation)
                if operation.operation.is_create()
                    && operation.checked_args.is_some()
        ) {
        201
    } else {
        200
    };
    Ok(json_response(status, body))
}

fn eval_checked_expr_json(
    expr: &CheckedRouteExpr<'_>,
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<String, String> {
    Ok(server_value_json(eval_checked_expr_value(
        expr,
        params,
        storage,
        program,
        request_body,
        route_method,
    )?))
}

fn eval_checked_expr_value(
    expr: &CheckedRouteExpr<'_>,
    params: &HashMap<String, ServerValue>,
    storage: &Storage,
    program: &Program,
    request_body: &str,
    route_method: &HttpMethod,
) -> Result<ServerValue, String> {
    match expr {
        CheckedRouteExpr::ModelOperation(operation) => {
            let ctx = RouteEvalContext {
                params,
                storage,
                program,
                request_body,
                route_method,
            };
            let Some(args) = operation.checked_args else {
                return unsupported_model_static_call(operation.model, operation.operation);
            };
            eval_model_static_operation(operation.operation, operation.model, args, &ctx)
        }
        CheckedRouteExpr::AuthOperation(operation) => Err(format!(
            "Auth::{}() nao pode ser usado como expressao HTTP aninhada",
            operation.operation.method_name()
        )),
        CheckedRouteExpr::Expr(expr) => {
            eval_expr_value(expr, params, storage, program, request_body, route_method)
        }
    }
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
            let Some(operation) = ModelStaticOperation::from_method(method) else {
                return Err(format!(
                    "Static call '{}::{}' nao suportada em HTTP",
                    ty, method
                ));
            };
            let ctx = RouteEvalContext {
                params,
                storage,
                program,
                request_body,
                route_method,
            };
            let Some(args) = operation.checked_args(args) else {
                return unsupported_model_static_call(ty, operation);
            };
            eval_model_static_operation(operation, ty, args, &ctx)
        }
    }
}

struct RouteEvalContext<'a> {
    params: &'a HashMap<String, ServerValue>,
    storage: &'a Storage,
    program: &'a Program,
    request_body: &'a str,
    route_method: &'a HttpMethod,
}

fn eval_model_static_operation(
    operation: ModelStaticOperation,
    ty: &str,
    args: CheckedModelOperationArgs<'_>,
    ctx: &RouteEvalContext<'_>,
) -> Result<ServerValue, String> {
    match operation.storage_category() {
        ModelOperationStorageCategory::ListRecords => {
            if args.raw.is_empty() {
                return Ok(ServerValue::Json(ctx.storage.read_model_raw_json(ty)?));
            }
            ensure_all_args_get(ty, ctx.route_method)?;
            let options = eval_all_list_options(
                ty,
                args,
                ctx.params,
                ctx.storage,
                ctx.program,
                ctx.request_body,
                ctx.route_method,
            )?;
            ctx.storage
                .list_model_records(ctx.program, ty, options.ordering, options.pagination)
        }
        ModelOperationStorageCategory::PageRecords => {
            ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
            let options = eval_page_list_options(
                ty,
                args,
                ctx.params,
                ctx.storage,
                ctx.program,
                ctx.request_body,
                ctx.route_method,
            )?;
            ctx.storage.list_model_records_page(
                ctx.program,
                ty,
                options.ordering,
                options.pagination,
            )
        }
        ModelOperationStorageCategory::CreateRecord => {
            ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
            ctx.storage
                .create_model_record(ctx.program, ty, ctx.request_body)
        }
        ModelOperationStorageCategory::FindRecord => {
            ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
            let (field_arg, value_arg) = lookup_args(ty, operation, args)?;
            let field = model_string_arg(
                ty,
                operation.method_name(),
                field_arg,
                "campo string literal",
            )?;
            let expected = eval_route_arg(value_arg, ctx)?;
            ctx.storage
                .find_model_record(ctx.program, ty, field, &expected)
        }
        ModelOperationStorageCategory::EqualityFilter { negated } => {
            ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
            let method = operation.method_name();
            let (field_arg, value_arg) = lookup_args(ty, operation, args)?;
            let field = model_string_arg(ty, method, field_arg, "campo string literal")?;
            let expected = eval_route_arg(value_arg, ctx)?;
            if args.has_page_response() {
                let options = eval_where_page_list_options(
                    ty,
                    method,
                    args,
                    ctx.params,
                    ctx.storage,
                    ctx.program,
                    ctx.request_body,
                    ctx.route_method,
                )?;
                if negated {
                    let ListOptions {
                        ordering,
                        pagination,
                    } = options;
                    let items = ctx.storage.filter_model_records_not(
                        ctx.program,
                        ty,
                        field,
                        &expected,
                        ordering,
                        None,
                    )?;
                    return ctx.storage.paginated_array_response(items, pagination);
                }
                return ctx.storage.filter_model_records_page(
                    ctx.program,
                    ty,
                    field,
                    &expected,
                    options.ordering,
                    options.pagination,
                );
            }

            let options = eval_where_list_options(
                ty,
                method,
                args,
                ctx.params,
                ctx.storage,
                ctx.program,
                ctx.request_body,
                ctx.route_method,
            )?;
            if negated {
                ctx.storage.filter_model_records_not(
                    ctx.program,
                    ty,
                    field,
                    &expected,
                    options.ordering,
                    options.pagination,
                )
            } else {
                ctx.storage.filter_model_records(
                    ctx.program,
                    ty,
                    field,
                    &expected,
                    options.ordering,
                    options.pagination,
                )
            }
        }
        ModelOperationStorageCategory::InclusionFilter { negated } => {
            ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
            let method = operation.method_name();
            let (field_arg, value_arg) = lookup_args(ty, operation, args)?;
            let field = model_string_arg(ty, method, field_arg, "campo string literal")?;
            let values = expect_array_values(ty, method, eval_route_arg(value_arg, ctx)?, false)?;
            if args.has_page_response() {
                let options = eval_where_page_list_options(
                    ty,
                    method,
                    args,
                    ctx.params,
                    ctx.storage,
                    ctx.program,
                    ctx.request_body,
                    ctx.route_method,
                )?;
                let ListOptions {
                    ordering,
                    pagination,
                } = options;
                let items = if negated {
                    ctx.storage.filter_model_records_by_not_in(
                        ctx.program,
                        ty,
                        field,
                        &values,
                        ordering,
                        None,
                    )?
                } else {
                    ctx.storage.filter_model_records_by_in(
                        ctx.program,
                        ty,
                        field,
                        &values,
                        ordering,
                        None,
                    )?
                };
                return ctx.storage.paginated_array_response(items, pagination);
            }

            let options = eval_where_list_options(
                ty,
                method,
                args,
                ctx.params,
                ctx.storage,
                ctx.program,
                ctx.request_body,
                ctx.route_method,
            )?;
            if negated {
                ctx.storage.filter_model_records_by_not_in(
                    ctx.program,
                    ty,
                    field,
                    &values,
                    options.ordering,
                    options.pagination,
                )
            } else {
                ctx.storage.filter_model_records_by_in(
                    ctx.program,
                    ty,
                    field,
                    &values,
                    options.ordering,
                    options.pagination,
                )
            }
        }
        ModelOperationStorageCategory::OptionalEqualityFilter => {
            eval_optional_filter(operation, ty, args, ctx)
        }
        ModelOperationStorageCategory::OptionalInclusionFilter
        | ModelOperationStorageCategory::OptionalExclusionFilter => {
            eval_optional_array_filter(operation, ty, args, ctx)
        }
        ModelOperationStorageCategory::ComparisonFilter => {
            eval_compare_filter(operation, ty, args, ctx)
        }
        ModelOperationStorageCategory::TextFilter => eval_text_filter(operation, ty, args, ctx),
        ModelOperationStorageCategory::RangeFilter => eval_between_filter(operation, ty, args, ctx),
        ModelOperationStorageCategory::CompositeFilter { .. } => {
            eval_composite_filter(operation, ty, args, ctx)
        }
        ModelOperationStorageCategory::UpdateRecord => {
            ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
            let method = operation.method_name();
            let (field_arg, value_arg) = lookup_args(ty, operation, args)?;
            let field = model_string_arg(ty, method, field_arg, "campo string literal")?;
            let expected = eval_route_arg(value_arg, ctx)?;
            ctx.storage
                .update_model_record(ctx.program, ty, field, &expected, ctx.request_body)
        }
        ModelOperationStorageCategory::DeleteRecord => {
            ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
            let method = operation.method_name();
            let (field_arg, value_arg) = lookup_args(ty, operation, args)?;
            let field = model_string_arg(ty, method, field_arg, "campo string literal")?;
            let expected = eval_route_arg(value_arg, ctx)?;
            ctx.storage
                .delete_model_record(ctx.program, ty, field, &expected)
        }
    }
}

fn eval_optional_filter(
    operation: ModelStaticOperation,
    ty: &str,
    args: CheckedModelOperationArgs<'_>,
    ctx: &RouteEvalContext<'_>,
) -> Result<ServerValue, String> {
    let paged = args.has_page_response();
    ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
    let method = operation.method_name();
    let (field_arg, value_arg) = lookup_args(ty, operation, args)?;
    let field = model_string_arg(ty, method, field_arg, "campo string literal")?;
    let expected = eval_route_arg(value_arg, ctx)?;

    if paged {
        let options = eval_where_page_list_options(
            ty,
            method,
            args,
            ctx.params,
            ctx.storage,
            ctx.program,
            ctx.request_body,
            ctx.route_method,
        )?;
        if matches!(expected, ServerValue::Null) {
            return ctx.storage.list_model_records_page(
                ctx.program,
                ty,
                options.ordering,
                options.pagination,
            );
        }
        return ctx.storage.filter_model_records_page(
            ctx.program,
            ty,
            field,
            &expected,
            options.ordering,
            options.pagination,
        );
    }

    let options = eval_where_list_options(
        ty,
        method,
        args,
        ctx.params,
        ctx.storage,
        ctx.program,
        ctx.request_body,
        ctx.route_method,
    )?;
    if matches!(expected, ServerValue::Null) {
        return ctx.storage.list_model_records(
            ctx.program,
            ty,
            options.ordering,
            options.pagination,
        );
    }
    ctx.storage.filter_model_records(
        ctx.program,
        ty,
        field,
        &expected,
        options.ordering,
        options.pagination,
    )
}

fn eval_optional_array_filter(
    operation: ModelStaticOperation,
    ty: &str,
    args: CheckedModelOperationArgs<'_>,
    ctx: &RouteEvalContext<'_>,
) -> Result<ServerValue, String> {
    let paged = args.has_page_response();
    ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
    let method = operation.method_name();
    let (field_arg, value_arg) = lookup_args(ty, operation, args)?;
    let field = model_string_arg(ty, method, field_arg, "campo string literal")?;
    let values = eval_route_arg(value_arg, ctx)?;
    let negated = match operation.storage_category() {
        ModelOperationStorageCategory::OptionalInclusionFilter => false,
        ModelOperationStorageCategory::OptionalExclusionFilter => true,
        _ => return unsupported_model_static_call(ty, operation),
    };

    if paged {
        let options = eval_where_page_list_options(
            ty,
            method,
            args,
            ctx.params,
            ctx.storage,
            ctx.program,
            ctx.request_body,
            ctx.route_method,
        )?;
        let ListOptions {
            ordering,
            pagination,
        } = options;
        if matches!(values, ServerValue::Null) {
            return ctx
                .storage
                .list_model_records_page(ctx.program, ty, ordering, pagination);
        }
        let values = expect_array_values(ty, method, values, true)?;
        let items = if negated {
            ctx.storage.filter_model_records_by_not_in(
                ctx.program,
                ty,
                field,
                &values,
                ordering,
                None,
            )?
        } else {
            ctx.storage.filter_model_records_by_in(
                ctx.program,
                ty,
                field,
                &values,
                ordering,
                None,
            )?
        };
        return ctx.storage.paginated_array_response(items, pagination);
    }

    let options = eval_where_list_options(
        ty,
        method,
        args,
        ctx.params,
        ctx.storage,
        ctx.program,
        ctx.request_body,
        ctx.route_method,
    )?;
    if matches!(values, ServerValue::Null) {
        return ctx.storage.list_model_records(
            ctx.program,
            ty,
            options.ordering,
            options.pagination,
        );
    }
    let values = expect_array_values(ty, method, values, true)?;
    if negated {
        ctx.storage.filter_model_records_by_not_in(
            ctx.program,
            ty,
            field,
            &values,
            options.ordering,
            options.pagination,
        )
    } else {
        ctx.storage.filter_model_records_by_in(
            ctx.program,
            ty,
            field,
            &values,
            options.ordering,
            options.pagination,
        )
    }
}

fn eval_compare_filter(
    operation: ModelStaticOperation,
    ty: &str,
    args: CheckedModelOperationArgs<'_>,
    ctx: &RouteEvalContext<'_>,
) -> Result<ServerValue, String> {
    let paged = args.has_page_response();
    ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
    let method = operation.method_name();
    let (field_arg, operator_arg, value_arg) = advanced_filter_args(ty, operation, args)?;
    let field = model_string_arg(ty, method, field_arg, "campo string literal")?;
    let operator = parse_compare_operator(
        ty,
        model_string_arg(ty, method, operator_arg, "operador string literal")?,
    )?;
    let expected = eval_route_arg(value_arg, ctx)?;
    let options = eval_where_compare_list_options(
        ty,
        method,
        args,
        ctx.params,
        ctx.storage,
        ctx.program,
        ctx.request_body,
        ctx.route_method,
    )?;
    if paged {
        let ListOptions {
            ordering,
            pagination,
        } = options;
        let items = ctx.storage.filter_model_records_by_comparison(
            ctx.program,
            ty,
            field,
            operator,
            &expected,
            ordering,
            None,
        )?;
        return ctx.storage.paginated_array_response(items, pagination);
    }
    ctx.storage.filter_model_records_by_comparison(
        ctx.program,
        ty,
        field,
        operator,
        &expected,
        options.ordering,
        options.pagination,
    )
}

fn eval_text_filter(
    operation: ModelStaticOperation,
    ty: &str,
    args: CheckedModelOperationArgs<'_>,
    ctx: &RouteEvalContext<'_>,
) -> Result<ServerValue, String> {
    let paged = args.has_page_response();
    ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
    let method = operation.method_name();
    let (field_arg, operator_arg, value_arg) = advanced_filter_args(ty, operation, args)?;
    let field = model_string_arg(ty, method, field_arg, "campo string literal")?;
    let operator = parse_text_operator(
        ty,
        model_string_arg(ty, method, operator_arg, "operador textual string literal")?,
    )?;
    let expected = eval_route_arg(value_arg, ctx)?;
    let options = eval_where_compare_list_options(
        ty,
        method,
        args,
        ctx.params,
        ctx.storage,
        ctx.program,
        ctx.request_body,
        ctx.route_method,
    )?;
    if paged {
        let ListOptions {
            ordering,
            pagination,
        } = options;
        let items = ctx.storage.filter_model_records_by_text(
            ctx.program,
            ty,
            field,
            operator,
            &expected,
            ordering,
            None,
        )?;
        return ctx.storage.paginated_array_response(items, pagination);
    }
    ctx.storage.filter_model_records_by_text(
        ctx.program,
        ty,
        field,
        operator,
        &expected,
        options.ordering,
        options.pagination,
    )
}

fn eval_between_filter(
    operation: ModelStaticOperation,
    ty: &str,
    args: CheckedModelOperationArgs<'_>,
    ctx: &RouteEvalContext<'_>,
) -> Result<ServerValue, String> {
    let paged = args.has_page_response();
    ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;
    let method = operation.method_name();
    let (field_arg, min_arg, max_arg) = range_filter_args(ty, operation, args)?;
    let field = model_string_arg(ty, method, field_arg, "campo string literal")?;
    let min = eval_route_arg(min_arg, ctx)?;
    let max = eval_route_arg(max_arg, ctx)?;
    let options = eval_where_compare_list_options(
        ty,
        method,
        args,
        ctx.params,
        ctx.storage,
        ctx.program,
        ctx.request_body,
        ctx.route_method,
    )?;
    if paged {
        let ListOptions {
            ordering,
            pagination,
        } = options;
        let items = ctx.storage.filter_model_records_by_range(
            ctx.program,
            ty,
            field,
            &min,
            &max,
            ordering,
            None,
        )?;
        return ctx.storage.paginated_array_response(items, pagination);
    }
    ctx.storage.filter_model_records_by_range(
        ctx.program,
        ty,
        field,
        &min,
        &max,
        options.ordering,
        options.pagination,
    )
}

fn eval_composite_filter(
    operation: ModelStaticOperation,
    ty: &str,
    args: CheckedModelOperationArgs<'_>,
    ctx: &RouteEvalContext<'_>,
) -> Result<ServerValue, String> {
    let paged = args.has_page_response();
    let ModelOperationStorageCategory::CompositeFilter { any } = operation.storage_category()
    else {
        return unsupported_model_static_call(ty, operation);
    };
    ensure_model_runtime_route_method(ty, operation, args, ctx.route_method)?;

    if paged {
        let (filters, options) = if any {
            eval_where_any_page_filters_and_options(
                ty,
                args,
                ctx.params,
                ctx.storage,
                ctx.program,
                ctx.request_body,
                ctx.route_method,
            )?
        } else {
            eval_where_all_page_filters_and_options(
                ty,
                args,
                ctx.params,
                ctx.storage,
                ctx.program,
                ctx.request_body,
                ctx.route_method,
            )?
        };
        let ListOptions {
            ordering,
            pagination,
        } = options;
        let items = if any {
            ctx.storage.filter_model_records_by_any_filters(
                ctx.program,
                ty,
                &filters,
                ordering,
                None,
            )?
        } else {
            ctx.storage.filter_model_records_by_filters(
                ctx.program,
                ty,
                &filters,
                ordering,
                None,
            )?
        };
        return ctx.storage.paginated_array_response(items, pagination);
    }

    let (filters, options) = if any {
        eval_where_any_filters_and_options(
            ty,
            args,
            ctx.params,
            ctx.storage,
            ctx.program,
            ctx.request_body,
            ctx.route_method,
        )?
    } else {
        eval_where_all_filters_and_options(
            ty,
            args,
            ctx.params,
            ctx.storage,
            ctx.program,
            ctx.request_body,
            ctx.route_method,
        )?
    };
    if any {
        ctx.storage.filter_model_records_by_any_filters(
            ctx.program,
            ty,
            &filters,
            options.ordering,
            options.pagination,
        )
    } else {
        ctx.storage.filter_model_records_by_filters(
            ctx.program,
            ty,
            &filters,
            options.ordering,
            options.pagination,
        )
    }
}

fn eval_route_arg(expr: &Expr, ctx: &RouteEvalContext<'_>) -> Result<ServerValue, String> {
    eval_expr_value(
        expr,
        ctx.params,
        ctx.storage,
        ctx.program,
        ctx.request_body,
        ctx.route_method,
    )
}

fn model_string_arg<'a>(
    ty: &str,
    method: &str,
    expr: &'a Expr,
    expected: &str,
) -> Result<&'a str, String> {
    match expr {
        Expr::StringLit { value, .. } => Ok(value),
        _ => Err(format!("{}::{}() espera {}", ty, method, expected)),
    }
}

fn expect_array_values(
    ty: &str,
    method: &str,
    value: ServerValue,
    optional: bool,
) -> Result<Vec<ServerValue>, String> {
    let ServerValue::Array(values) = value else {
        let qualifier = if optional { "array opcional" } else { "array" };
        return Err(format!(
            "Requisicao invalida: {}::{}() espera {} de valores",
            ty, method, qualifier
        ));
    };
    Ok(values)
}

fn ensure_all_args_get(ty: &str, route_method: &HttpMethod) -> Result<(), String> {
    let Some(required) = ModelStaticOperation::All.required_route_method(2) else {
        return Ok(());
    };
    if required.matches(route_method) {
        Ok(())
    } else {
        Err(format!(
            "{}::all() com argumentos so pode ser usado em GET",
            ty
        ))
    }
}

fn ensure_model_runtime_route_method(
    ty: &str,
    operation: ModelStaticOperation,
    args: CheckedModelOperationArgs<'_>,
    route_method: &HttpMethod,
) -> Result<(), String> {
    let Some(required) = operation.required_route_method(args.raw.len()) else {
        return Ok(());
    };
    if required.matches(route_method) {
        Ok(())
    } else {
        Err(format!(
            "{}::{}() so pode ser usado em {}",
            ty,
            operation.method_name(),
            required.name()
        ))
    }
}

fn lookup_args<'a>(
    ty: &str,
    operation: ModelStaticOperation,
    args: CheckedModelOperationArgs<'a>,
) -> Result<(&'a Expr, &'a Expr), String> {
    args.lookup()
        .ok_or_else(|| unsupported_model_static_call_message(ty, operation))
}

fn advanced_filter_args<'a>(
    ty: &str,
    operation: ModelStaticOperation,
    args: CheckedModelOperationArgs<'a>,
) -> Result<(&'a Expr, &'a Expr, &'a Expr), String> {
    args.advanced_filter()
        .ok_or_else(|| unsupported_model_static_call_message(ty, operation))
}

fn range_filter_args<'a>(
    ty: &str,
    operation: ModelStaticOperation,
    args: CheckedModelOperationArgs<'a>,
) -> Result<(&'a Expr, &'a Expr, &'a Expr), String> {
    args.range_filter()
        .ok_or_else(|| unsupported_model_static_call_message(ty, operation))
}

fn unsupported_model_static_call(
    ty: &str,
    operation: ModelStaticOperation,
) -> Result<ServerValue, String> {
    Err(unsupported_model_static_call_message(ty, operation))
}

fn unsupported_model_static_call_message(ty: &str, operation: ModelStaticOperation) -> String {
    format!(
        "Static call '{}::{}' nao suportada em HTTP",
        ty,
        operation.method_name()
    )
}
