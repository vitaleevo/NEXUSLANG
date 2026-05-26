use std::collections::{HashMap, HashSet};

use crate::ast::*;

use super::http::method_name;
use super::storage::*;

pub fn generate_openapi(program: &Program) -> String {
    let mut path_items = Vec::new();
    let mut path_indices = HashMap::new();
    let mut operation_ids = HashMap::new();
    let mut route_tags = Vec::new();
    let mut seen_route_tags = HashSet::new();
    let parameter_components = openapi_component_parameters(program);
    let request_body_components = openapi_component_request_bodies(program);
    let response_components = openapi_component_responses(program);

    for route in routes(program) {
        let openapi_path = route.path.replace(':', "{").replace_segments_for_openapi();
        let params = route_parameters(&route, &parameter_components);
        let schema = route_response_schema(program, &route);
        let response_status = route_response_status(&route);
        let success_response_ref = openapi_success_response_ref(response_status, &schema);
        let request_body = route_request_body_ref(&route);
        let operation_id = unique_operation_id(route_operation_id(&route), &mut operation_ids);
        let tag = route_tag(&route);
        if seen_route_tags.insert(tag.clone()) {
            route_tags.push(tag.clone());
        }

        let mut operation = String::new();
        operation.push('"');
        operation.push_str(&method_name(route.method).to_lowercase());
        operation.push_str(r#"":{"summary":""#);
        operation.push_str(method_name(route.method));
        operation.push(' ');
        operation.push_str(&escape_json(route.path));
        operation.push_str(r#"","operationId":""#);
        operation.push_str(&escape_json(&operation_id));
        operation.push_str(r#"","tags":[""#);
        operation.push_str(&escape_json(&tag));
        operation.push_str(r#""]"#);
        operation.push_str(r#","parameters":["#);
        operation.push_str(&params);
        operation.push(']');
        if let Some(request_body_ref) = request_body {
            operation.push_str(r#","requestBody":"#);
            operation.push_str(&request_body_ref);
        }
        if route.auth.is_some() {
            operation.push_str(r#","security":[{"NexusSession":[]},{"NexusBearer":[]}]"#);
        }
        if route_has_pagination(&route) {
            operation.push_str(r#","x-nexus-pagination":true"#);
        }
        if route_has_total_count(&route) {
            operation.push_str(r#","x-nexus-total-count":true"#);
        }
        if route_has_ordering(&route) {
            operation.push_str(r#","x-nexus-ordering":true"#);
        }
        if route_has_composite_filters(&route) {
            operation.push_str(r#","x-nexus-composite-filters":true"#);
        }
        if route_has_or_filters(&route) {
            operation.push_str(r#","x-nexus-or-filters":true"#);
        }
        if route_has_exclusion_filters(&route) {
            operation.push_str(r#","x-nexus-exclusion-filters":true"#);
        }
        if route_has_optional_filters(&route) {
            operation.push_str(r#","x-nexus-optional-filters":true"#);
        }
        if route_has_in_filters(&route) {
            operation.push_str(r#","x-nexus-in-filters":true"#);
        }
        if route_has_comparison_filters(&route) {
            operation.push_str(r#","x-nexus-comparison-filters":true"#);
        }
        if route_has_text_filters(&route) {
            operation.push_str(r#","x-nexus-text-filters":true"#);
        }
        if route_has_range_filters(&route) {
            operation.push_str(r#","x-nexus-range-filters":true"#);
        }
        operation.push_str(r#","responses":{"#);
        operation.push('"');
        operation.push_str(response_status);
        operation.push_str(r#"":"#);
        if let Some(success_response_ref) = success_response_ref {
            operation.push_str(&success_response_ref);
        } else {
            operation.push_str(&openapi_success_response(response_status, &schema));
        }
        if route_has_bad_request_response(&route) {
            operation.push_str(&openapi_error_response("400", "Bad Request"));
        }
        if route_has_not_found_response(&route) {
            operation.push_str(&openapi_error_response("404", "Not Found"));
        }
        if route_has_conflict_response(program, &route) {
            operation.push_str(&openapi_error_response("409", "Conflict"));
        }
        if route.auth.is_some() {
            operation.push_str(&openapi_error_response("401", "Unauthorized"));
        }
        if route.auth.and_then(|guard| guard.role.as_ref()).is_some() {
            operation.push_str(&openapi_error_response("403", "Forbidden"));
        }
        operation.push_str(r#"}}"#);
        push_openapi_path_operation(&mut path_items, &mut path_indices, openapi_path, operation);
    }

    let paths = openapi_paths(&path_items);
    let schemas = openapi_component_schemas(program);
    let parameters = parameter_components.to_openapi();
    let request_bodies = openapi_request_bodies(&request_body_components);
    let responses = openapi_responses(&response_components);
    let security_schemes = openapi_security_schemes(program);
    let tags = openapi_tags(&route_tags);
    format!(
        r#"{{"openapi":"3.0.0","info":{{"title":"NexusLang API","version":"0.1.0"}},"tags":[{}],"paths":{{{}}},"components":{{"schemas":{{{}}},"parameters":{{{}}},"requestBodies":{{{}}},"responses":{{{}}}{} }}}}"#,
        tags, paths, schemas, parameters, request_bodies, responses, security_schemes
    )
}

fn openapi_security_schemes(program: &Program) -> String {
    if !has_auth(program) {
        return String::new();
    }
    r#","securitySchemes":{"NexusSession":{"type":"apiKey","in":"cookie","name":"__Host-nexus_session"},"NexusBearer":{"type":"http","scheme":"bearer"}}"#.to_string()
}

fn push_openapi_path_operation(
    path_items: &mut Vec<(String, Vec<String>)>,
    path_indices: &mut HashMap<String, usize>,
    path: String,
    operation: String,
) {
    let index = if let Some(index) = path_indices.get(&path) {
        *index
    } else {
        let index = path_items.len();
        path_indices.insert(path.clone(), index);
        path_items.push((path, Vec::new()));
        index
    };

    path_items[index].1.push(operation);
}

fn openapi_paths(path_items: &[(String, Vec<String>)]) -> String {
    path_items
        .iter()
        .map(|(path, operations)| {
            format!(r#""{}":{{{}}}"#, escape_json(path), operations.join(","))
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Default)]
struct OpenApiParameterComponents {
    entries: Vec<(String, String)>,
    names_by_definition: HashMap<String, String>,
    used_names: HashSet<String>,
}

impl OpenApiParameterComponents {
    fn insert(&mut self, base_name: String, definition: String) {
        if self.names_by_definition.contains_key(&definition) {
            return;
        }

        let name = self.unique_name(base_name);
        self.names_by_definition
            .insert(definition.clone(), name.clone());
        self.entries.push((name, definition));
    }

    fn unique_name(&mut self, base_name: String) -> String {
        if self.used_names.insert(base_name.clone()) {
            return base_name;
        }

        for suffix in 2.. {
            let candidate = format!("{}_{}", base_name, suffix);
            if self.used_names.insert(candidate.clone()) {
                return candidate;
            }
        }

        unreachable!()
    }

    fn ref_for_definition(&self, definition: &str) -> Option<String> {
        self.names_by_definition.get(definition).map(|name| {
            format!(
                r##"{{"$ref":"#/components/parameters/{}"}}"##,
                escape_json(name)
            )
        })
    }

    fn to_openapi(&self) -> String {
        self.entries
            .iter()
            .map(|(name, definition)| format!(r#""{}":{}"#, escape_json(name), definition))
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn openapi_component_parameters(program: &Program) -> OpenApiParameterComponents {
    let mut components = OpenApiParameterComponents::default();

    for route in routes(program) {
        for param in route.params {
            components.insert(
                openapi_parameter_component_base_name("NexusPathParam", param),
                openapi_path_parameter_definition(param),
            );
        }
        for param in route.query_params {
            components.insert(
                openapi_parameter_component_base_name("NexusQueryParam", &param.name),
                openapi_query_parameter_definition(param),
            );
        }
    }

    components
}

fn openapi_parameter_component_base_name(prefix: &str, name: &str) -> String {
    format!(
        "{}_{}",
        prefix,
        operation_id_segment(name).unwrap_or_else(|| "param".to_string())
    )
}

fn openapi_component_request_bodies(program: &Program) -> Vec<String> {
    let mut models = Vec::new();
    let mut seen = HashSet::new();

    for route in routes(program) {
        if let Some(model) = route_request_body_model(&route) {
            if seen.insert(model.clone()) {
                models.push(model);
            }
        }
    }

    models
}

fn openapi_request_bodies(models: &[String]) -> String {
    models
        .iter()
        .map(|model| {
            format!(
                r#""{}":{}"#,
                escape_json(&openapi_request_body_component_name(model)),
                openapi_request_body_component(model)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn openapi_request_body_component_name(model: &str) -> String {
    format!("NexusRequestBody_{}", model)
}

fn openapi_request_body_component(model: &str) -> String {
    format!(
        r##"{{"required":true,"content":{{"application/json":{{"schema":{{"$ref":"#/components/schemas/{}"}}}}}}}}"##,
        escape_json(model)
    )
}

fn openapi_request_body_ref(model: &str) -> String {
    format!(
        r##"{{"$ref":"#/components/requestBodies/{}"}}"##,
        escape_json(&openapi_request_body_component_name(model))
    )
}

fn openapi_component_responses(program: &Program) -> Vec<(String, String)> {
    let mut responses = Vec::new();
    let mut seen = HashSet::new();

    for route in routes(program) {
        let schema = route_response_schema(program, &route);
        let status = route_response_status(&route);
        if let Some(name) = openapi_success_response_component_name(status, &schema) {
            if seen.insert(name.clone()) {
                responses.push((name, openapi_success_response(status, &schema)));
            }
        }
    }

    responses
}

fn openapi_responses(responses: &[(String, String)]) -> String {
    responses
        .iter()
        .map(|(name, response)| format!(r#""{}":{}"#, escape_json(name), response))
        .collect::<Vec<_>>()
        .join(",")
}

fn openapi_success_response_ref(status: &str, schema: &str) -> Option<String> {
    openapi_success_response_component_name(status, schema).map(|name| {
        format!(
            r##"{{"$ref":"#/components/responses/{}"}}"##,
            escape_json(&name)
        )
    })
}

fn openapi_success_response_component_name(status: &str, schema: &str) -> Option<String> {
    let schema_component = openapi_schema_ref_component_name(schema)?;
    Some(format!("NexusResponse{}_{}", status, schema_component))
}

fn openapi_schema_ref_component_name(schema: &str) -> Option<String> {
    schema
        .strip_prefix(r##"{"$ref":"#/components/schemas/"##)?
        .strip_suffix(r##""}"##)
        .map(|name| name.to_string())
}

fn openapi_success_response(status: &str, schema: &str) -> String {
    format!(
        r#"{{"description":"{}","content":{{"application/json":{{"schema":{}}}}}}}"#,
        if status == "201" { "Created" } else { "OK" },
        schema
    )
}

fn openapi_component_schemas(program: &Program) -> String {
    let models = program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Model { name, fields, .. } => Some((name, fields)),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut schemas = models
        .iter()
        .map(|(name, fields)| {
            format!(
                r#""{}":{}"#,
                escape_json(name),
                openapi_model_schema(fields)
            )
        })
        .collect::<Vec<_>>();

    schemas.extend(models.iter().map(|(name, _)| {
        format!(
            r#""{}":{}"#,
            escape_json(&openapi_page_component_name(name)),
            openapi_page_component_schema(name)
        )
    }));
    schemas.extend(models.iter().map(|(name, _)| {
        format!(
            r#""{}":{}"#,
            escape_json(&openapi_list_component_name(name)),
            openapi_list_component_schema(name)
        )
    }));
    schemas.push(format!(r#""NexusError":{}"#, openapi_error_schema()));
    schemas.join(",")
}

fn openapi_error_response(status: &str, description: &str) -> String {
    format!(
        r##","{}":{{"description":"{}","content":{{"application/json":{{"schema":{{"$ref":"#/components/schemas/NexusError"}}}}}}}}"##,
        status, description
    )
}

fn openapi_error_schema() -> &'static str {
    r#"{"type":"object","properties":{"error":{"type":"string"}},"required":["error"]}"#
}

fn openapi_model_schema(fields: &[Field]) -> String {
    let properties = fields
        .iter()
        .map(|field| {
            format!(
                r#""{}":{}"#,
                escape_json(&field.name),
                openapi_field_schema(field)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let required = fields
        .iter()
        .filter(|field| field.default.is_none() && !type_is_optional(&field.ty))
        .map(|field| format!(r#""{}""#, escape_json(&field.name)))
        .collect::<Vec<_>>()
        .join(",");

    if required.is_empty() {
        format!(r#"{{"type":"object","properties":{{{}}}}}"#, properties)
    } else {
        format!(
            r#"{{"type":"object","properties":{{{}}},"required":[{}]}}"#,
            properties, required
        )
    }
}

fn openapi_field_schema(field: &Field) -> String {
    let mut schema = openapi_schema_for_type(&field.ty);
    if field.unique {
        schema = schema_with_property(schema, "x-nexus-unique", "true".to_string());
    }
    if field.index {
        schema = schema_with_property(schema, "x-nexus-index", "true".to_string());
    }
    if let Some(min) = &field.min {
        if let Some((name, value)) = openapi_min_max_property(&field.ty, "min", min) {
            schema = schema_with_property(schema, name, value);
        }
    }
    if let Some(max) = &field.max {
        if let Some((name, value)) = openapi_min_max_property(&field.ty, "max", max) {
            schema = schema_with_property(schema, name, value);
        }
    }
    if let Some(default) = &field.default {
        if let Some(default_json) = openapi_default_value(default) {
            schema = schema_with_property(schema, "default", default_json);
        }
    }
    schema
}

pub(crate) fn route_response_schema(program: &Program, route: &RouteView<'_>) -> String {
    route
        .body
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::Return { value, .. } => Some(
                openapi_auth_expr_schema(program, route, value).unwrap_or_else(|| {
                    openapi_expr_schema(program, value, route.params, route.query_params)
                }),
            ),
            _ => None,
        })
        .unwrap_or_else(|| "{}".to_string())
}

fn openapi_auth_expr_schema(
    program: &Program,
    route: &RouteView<'_>,
    expr: &Expr,
) -> Option<String> {
    let Expr::StaticCall {
        ty, method, args, ..
    } = expr
    else {
        return None;
    };
    if ty != "Auth" {
        return None;
    }
    match method.as_str() {
        "register" | "login" => auth_config_from_args(program, args).map(|config| {
            format!(
                r#"{{"type":"object","properties":{{"user":{},"token":{{"type":"string"}},"expires_in":{{"type":"integer"}}}}}}"#,
                openapi_response_schema_for_type(&Type::Model(config.model.clone()))
            )
        }),
        "user" => route
            .auth
            .and_then(|guard| auth_config(program, &guard.auth))
            .map(|config| openapi_response_schema_for_type(&Type::Model(config.model.clone()))),
        "logout" => Some(r#"{"type":"boolean"}"#.to_string()),
        _ => None,
    }
}

fn auth_config_from_args<'a>(program: &'a Program, args: &[Expr]) -> Option<&'a AuthConfig> {
    let [Expr::Ident { name, .. }] = args else {
        return None;
    };
    auth_config(program, name)
}

fn route_parameters(route: &RouteView<'_>, components: &OpenApiParameterComponents) -> String {
    let mut params = route
        .params
        .iter()
        .map(|param| {
            let definition = openapi_path_parameter_definition(param);
            components
                .ref_for_definition(&definition)
                .unwrap_or(definition)
        })
        .collect::<Vec<_>>();

    params.extend(route.query_params.iter().map(|param| {
        let definition = openapi_query_parameter_definition(param);
        components
            .ref_for_definition(&definition)
            .unwrap_or(definition)
    }));

    params.join(",")
}

fn openapi_path_parameter_definition(param: &str) -> String {
    format!(
        r#"{{"name":"{}","in":"path","required":true,"schema":{{"type":"string"}}}}"#,
        escape_json(param)
    )
}

fn openapi_query_parameter_definition(param: &QueryParam) -> String {
    let mut schema = openapi_query_param_schema_for_type(&param.ty);
    if let Some(default) = &param.default {
        if let Some(default_json) = openapi_query_default_value(&param.ty, default) {
            schema = schema_with_property(schema, "default", default_json);
        }
    }
    let array_style = if query_param_is_array(&param.ty) {
        r#","style":"form","explode":false"#
    } else {
        ""
    };
    format!(
        r#"{{"name":"{}","in":"query","required":{},"schema":{}{}}}"#,
        escape_json(&param.name),
        query_param_required(param),
        schema,
        array_style
    )
}

fn route_request_body_ref(route: &RouteView<'_>) -> Option<String> {
    route_request_body_model(route).map(|model| openapi_request_body_ref(&model))
}

pub(crate) fn route_request_body_model(route: &RouteView<'_>) -> Option<String> {
    route_create_model(route).or_else(|| route_update_model(route))
}

pub(crate) fn route_response_status(route: &RouteView<'_>) -> &'static str {
    let auth_register = route.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Return {
                value:
                    Expr::StaticCall {
                        ty,
                        method,
                        ..
                    },
                ..
            } if ty == "Auth" && method == "register"
        )
    });
    let model_create =
        matches!(route.method, HttpMethod::Post) && route_create_model(route).is_some();

    if auth_register || model_create {
        "201"
    } else {
        "200"
    }
}

fn route_operation_id(route: &RouteView<'_>) -> String {
    let mut parts = vec![method_name(route.method).to_lowercase()];

    for segment in route.path.split('/').filter(|segment| !segment.is_empty()) {
        if let Some(param) = segment.strip_prefix(':') {
            parts.push("by".to_string());
            if let Some(part) = operation_id_segment(param) {
                parts.push(part);
            }
        } else if let Some(part) = operation_id_segment(segment) {
            parts.push(part);
        }
    }

    parts.join("_")
}

fn route_tag(route: &RouteView<'_>) -> String {
    route
        .path
        .split('/')
        .filter(|segment| !segment.is_empty() && !segment.starts_with(':'))
        .find_map(operation_id_segment)
        .unwrap_or_else(|| "routes".to_string())
}

fn openapi_tags(tags: &[String]) -> String {
    tags.iter()
        .map(|tag| format!(r#"{{"name":"{}"}}"#, escape_json(tag)))
        .collect::<Vec<_>>()
        .join(",")
}

fn unique_operation_id(base: String, operation_ids: &mut HashMap<String, usize>) -> String {
    let count = operation_ids.entry(base.clone()).or_insert(0);
    *count += 1;

    if *count == 1 {
        base
    } else {
        format!("{}_{}", base, *count)
    }
}

fn operation_id_segment(segment: &str) -> Option<String> {
    let mut normalized = String::new();
    let mut previous_was_separator = false;

    for ch in segment.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !normalized.is_empty() && !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }

    if normalized.ends_with('_') {
        normalized.pop();
    }

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub(crate) fn route_has_bad_request_response(route: &RouteView<'_>) -> bool {
    !route.query_params.is_empty()
        || route_create_model(route).is_some()
        || route_update_model(route).is_some()
}

pub(crate) fn route_has_not_found_response(route: &RouteView<'_>) -> bool {
    route_find_model(route).is_some()
        || route_update_model(route).is_some()
        || route_delete_model(route).is_some()
}

pub(crate) fn route_has_conflict_response(program: &Program, route: &RouteView<'_>) -> bool {
    route_create_model(route)
        .or_else(|| route_update_model(route))
        .and_then(|model| model_fields(program, &model))
        .map(has_unique_fields)
        .unwrap_or(false)
}

fn route_has_pagination(route: &RouteView<'_>) -> bool {
    route.body.iter().any(|stmt| match stmt {
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "all" => all_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "page" => page_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where" => where_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_page" => where_page_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not" => where_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not_page" => where_page_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not_in" => where_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not_in_page" => where_page_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not_in_optional" => where_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not_in_optional_page" => where_page_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_optional" => where_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_optional_page" => where_page_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_in" => where_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_in_page" => where_page_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_in_optional" => where_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_in_optional_page" => where_page_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_compare" => where_compare_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_compare_page" => advanced_page_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_text" => where_compare_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_text_page" => advanced_page_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_between" => where_compare_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_between_page" => advanced_page_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_all" => where_all_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_all_page" => where_all_page_filter_arg_count(args).is_some(),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_any" => where_all_args_have_pagination(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_any_page" => where_all_page_filter_arg_count(args).is_some(),
        _ => false,
    })
}

fn route_has_ordering(route: &RouteView<'_>) -> bool {
    route.body.iter().any(|stmt| match stmt {
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "all" => all_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "page" => page_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where" => where_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_page" => where_page_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not" => where_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not_page" => where_page_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not_in" => where_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not_in_page" => where_page_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not_in_optional" => where_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_not_in_optional_page" => where_page_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_optional" => where_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_optional_page" => where_page_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_in" => where_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_in_page" => where_page_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_in_optional" => where_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_in_optional_page" => where_page_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_compare" => where_compare_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_compare_page" => advanced_page_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_text" => where_compare_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_text_page" => advanced_page_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_between" => where_compare_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_between_page" => advanced_page_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_all" => where_all_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_all_page" => where_all_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_any" => where_all_args_have_ordering(args),
        Stmt::Return {
            value: Expr::StaticCall { method, args, .. },
            ..
        } if method == "where_any_page" => where_all_args_have_ordering(args),
        _ => false,
    })
}

fn route_has_total_count(route: &RouteView<'_>) -> bool {
    route.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Return {
                value: Expr::StaticCall { method, args, .. },
                ..
            } if (method == "page" && page_args_have_pagination(args))
                || (method == "where_page" && where_page_args_have_pagination(args))
                || (method == "where_not_page" && where_page_args_have_pagination(args))
                || (method == "where_not_in_page" && where_page_args_have_pagination(args))
                || (method == "where_not_in_optional_page" && where_page_args_have_pagination(args))
                || (method == "where_optional_page" && where_page_args_have_pagination(args))
                || (method == "where_in_page" && where_page_args_have_pagination(args))
                || (method == "where_in_optional_page" && where_page_args_have_pagination(args))
                || (method == "where_compare_page" && advanced_page_args_have_pagination(args))
                || (method == "where_text_page" && advanced_page_args_have_pagination(args))
                || (method == "where_between_page" && advanced_page_args_have_pagination(args))
                || (method == "where_all_page" && where_all_page_filter_arg_count(args).is_some())
                || (method == "where_any_page" && where_all_page_filter_arg_count(args).is_some())
        )
    })
}

fn route_has_composite_filters(route: &RouteView<'_>) -> bool {
    route.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Return {
                value: Expr::StaticCall { method, args, .. },
                ..
            } if (method == "where_all" && where_all_filter_arg_count(args).is_some())
                || (method == "where_all_page" && where_all_page_filter_arg_count(args).is_some())
        )
    })
}

fn route_has_or_filters(route: &RouteView<'_>) -> bool {
    route.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Return {
                value: Expr::StaticCall { method, args, .. },
                ..
            } if (method == "where_any" && where_all_filter_arg_count(args).is_some())
                || (method == "where_any_page" && where_all_page_filter_arg_count(args).is_some())
        )
    })
}

fn route_has_exclusion_filters(route: &RouteView<'_>) -> bool {
    route.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Return {
                value: Expr::StaticCall { method, args, .. },
                ..
            } if (method == "where_not" && (args.len() == 2 || args.len() == 4 || args.len() == 6))
                || (method == "where_not_page" && where_page_args_have_pagination(args))
                || (method == "where_not_in" && (args.len() == 2 || args.len() == 4 || args.len() == 6))
                || (method == "where_not_in_page" && where_page_args_have_pagination(args))
                || (method == "where_not_in_optional" && (args.len() == 2 || args.len() == 4 || args.len() == 6))
                || (method == "where_not_in_optional_page" && where_page_args_have_pagination(args))
        )
    })
}

fn route_has_optional_filters(route: &RouteView<'_>) -> bool {
    route.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Return {
                value: Expr::StaticCall { method, args, .. },
                ..
            } if (method == "where_optional" && (args.len() == 2 || args.len() == 4 || args.len() == 6))
                || (method == "where_optional_page" && where_page_args_have_pagination(args))
                || (method == "where_in_optional" && (args.len() == 2 || args.len() == 4 || args.len() == 6))
                || (method == "where_in_optional_page" && where_page_args_have_pagination(args))
                || (method == "where_not_in_optional" && (args.len() == 2 || args.len() == 4 || args.len() == 6))
                || (method == "where_not_in_optional_page" && where_page_args_have_pagination(args))
        )
    })
}

fn route_has_in_filters(route: &RouteView<'_>) -> bool {
    route.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Return {
                value: Expr::StaticCall { method, args, .. },
                ..
            } if (method == "where_in" && (args.len() == 2 || args.len() == 4 || args.len() == 6))
                || (method == "where_in_page" && where_page_args_have_pagination(args))
                || (method == "where_in_optional" && (args.len() == 2 || args.len() == 4 || args.len() == 6))
                || (method == "where_in_optional_page" && where_page_args_have_pagination(args))
                || (method == "where_not_in" && (args.len() == 2 || args.len() == 4 || args.len() == 6))
                || (method == "where_not_in_page" && where_page_args_have_pagination(args))
                || (method == "where_not_in_optional" && (args.len() == 2 || args.len() == 4 || args.len() == 6))
                || (method == "where_not_in_optional_page" && where_page_args_have_pagination(args))
        )
    })
}

fn route_has_comparison_filters(route: &RouteView<'_>) -> bool {
    route.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Return {
                value: Expr::StaticCall { method, args, .. },
                ..
            } if (method == "where_compare" && (args.len() == 3 || args.len() == 5 || args.len() == 7))
                || (method == "where_compare_page" && advanced_page_args_have_pagination(args))
        )
    })
}

fn route_has_text_filters(route: &RouteView<'_>) -> bool {
    route.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Return {
                value: Expr::StaticCall { method, args, .. },
                ..
            } if (method == "where_text" && (args.len() == 3 || args.len() == 5 || args.len() == 7))
                || (method == "where_text_page" && advanced_page_args_have_pagination(args))
        )
    })
}

fn route_has_range_filters(route: &RouteView<'_>) -> bool {
    route.body.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Return {
                value: Expr::StaticCall { method, args, .. },
                ..
            } if (method == "where_between" && (args.len() == 3 || args.len() == 5 || args.len() == 7))
                || (method == "where_between_page" && advanced_page_args_have_pagination(args))
        )
    })
}

fn all_args_have_pagination(args: &[Expr]) -> bool {
    (args.len() == 2 && !starts_ordering_args(args)) || args.len() == 4
}

fn all_args_have_ordering(args: &[Expr]) -> bool {
    (args.len() == 2 && starts_ordering_args(args)) || args.len() == 4
}

fn page_args_have_pagination(args: &[Expr]) -> bool {
    args.len() == 2 || args.len() == 4
}

fn page_args_have_ordering(args: &[Expr]) -> bool {
    args.len() == 4
}

fn where_args_have_pagination(args: &[Expr]) -> bool {
    (args.len() == 4 && !starts_ordering_args(&args[2..])) || args.len() == 6
}

fn where_args_have_ordering(args: &[Expr]) -> bool {
    (args.len() == 4 && starts_ordering_args(&args[2..])) || args.len() == 6
}

fn where_page_args_have_pagination(args: &[Expr]) -> bool {
    args.len() == 4 || args.len() == 6
}

fn where_page_args_have_ordering(args: &[Expr]) -> bool {
    args.len() == 6
}

fn where_compare_args_have_pagination(args: &[Expr]) -> bool {
    (args.len() == 5 && !starts_ordering_args(&args[3..])) || args.len() == 7
}

fn where_compare_args_have_ordering(args: &[Expr]) -> bool {
    (args.len() == 5 && starts_ordering_args(&args[3..])) || args.len() == 7
}

fn advanced_page_args_have_pagination(args: &[Expr]) -> bool {
    args.len() == 5 || args.len() == 7
}

fn advanced_page_args_have_ordering(args: &[Expr]) -> bool {
    args.len() == 7
}

fn starts_ordering_args(args: &[Expr]) -> bool {
    args.first().is_some_and(expr_is_string_lit)
}

fn expr_is_string_lit(expr: &Expr) -> bool {
    matches!(expr, Expr::StringLit { .. })
}

fn where_all_filter_arg_count(args: &[Expr]) -> Option<usize> {
    if args.len() < 4 {
        return None;
    }
    let filter_arg_count = if where_all_args_have_ordering(args) {
        args.len() - 4
    } else if where_all_args_have_pagination(args) {
        args.len() - 2
    } else {
        args.len()
    };
    if filter_arg_count >= 4 && filter_arg_count % 2 == 0 {
        Some(filter_arg_count)
    } else {
        None
    }
}

fn where_all_page_filter_arg_count(args: &[Expr]) -> Option<usize> {
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

fn where_all_args_have_pagination(args: &[Expr]) -> bool {
    args.len() >= 6 && !expr_is_string_lit(&args[args.len() - 2])
}

fn where_all_args_have_ordering(args: &[Expr]) -> bool {
    args.len() >= 8
        && expr_is_string_lit(&args[args.len() - 4])
        && expr_is_order_direction_lit(&args[args.len() - 3])
        && !expr_is_string_lit(&args[args.len() - 2])
}

fn expr_is_order_direction_lit(expr: &Expr) -> bool {
    matches!(expr, Expr::StringLit { value, .. } if value == "asc" || value == "desc")
}

fn route_create_model(route: &RouteView<'_>) -> Option<String> {
    route.body.iter().find_map(|stmt| match stmt {
        Stmt::Return {
            value: Expr::StaticCall {
                ty, method, args, ..
            },
            ..
        } if method == "create" && args.is_empty() => Some(ty.clone()),
        _ => None,
    })
}

fn route_find_model(route: &RouteView<'_>) -> Option<String> {
    route.body.iter().find_map(|stmt| match stmt {
        Stmt::Return {
            value: Expr::StaticCall {
                ty, method, args, ..
            },
            ..
        } if method == "find" && args.len() == 2 => Some(ty.clone()),
        _ => None,
    })
}

fn route_update_model(route: &RouteView<'_>) -> Option<String> {
    route.body.iter().find_map(|stmt| match stmt {
        Stmt::Return {
            value: Expr::StaticCall {
                ty, method, args, ..
            },
            ..
        } if method == "update" && args.len() == 2 => Some(ty.clone()),
        _ => None,
    })
}

fn route_delete_model(route: &RouteView<'_>) -> Option<String> {
    route.body.iter().find_map(|stmt| match stmt {
        Stmt::Return {
            value: Expr::StaticCall {
                ty, method, args, ..
            },
            ..
        } if method == "delete" && args.len() == 2 => Some(ty.clone()),
        _ => None,
    })
}

fn openapi_expr_schema(
    program: &Program,
    expr: &Expr,
    params: &[String],
    query_params: &[QueryParam],
) -> String {
    if let Some(schema) = openapi_page_expr_schema(expr) {
        return schema;
    }
    infer_http_expr_type(program, expr, params, query_params)
        .map(|ty| openapi_response_schema_for_type(&ty))
        .unwrap_or_else(|| "{}".to_string())
}

fn openapi_page_expr_schema(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "page" && page_args_have_pagination(args) => Some(openapi_page_schema(ty)),
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_page" && where_page_args_have_pagination(args) => {
            Some(openapi_page_schema(ty))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_not_page" && where_page_args_have_pagination(args) => {
            Some(openapi_page_schema(ty))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_not_in_page" && where_page_args_have_pagination(args) => {
            Some(openapi_page_schema(ty))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_not_in_optional_page" && where_page_args_have_pagination(args) => {
            Some(openapi_page_schema(ty))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_optional_page" && where_page_args_have_pagination(args) => {
            Some(openapi_page_schema(ty))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_in_page" && where_page_args_have_pagination(args) => {
            Some(openapi_page_schema(ty))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_in_optional_page" && where_page_args_have_pagination(args) => {
            Some(openapi_page_schema(ty))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_compare_page" && advanced_page_args_have_pagination(args) => {
            Some(openapi_page_schema(ty))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_text_page" && advanced_page_args_have_pagination(args) => {
            Some(openapi_page_schema(ty))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_between_page" && advanced_page_args_have_pagination(args) => {
            Some(openapi_page_schema(ty))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_all_page" && where_all_page_filter_arg_count(args).is_some() => {
            Some(openapi_page_schema(ty))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_any_page" && where_all_page_filter_arg_count(args).is_some() => {
            Some(openapi_page_schema(ty))
        }
        _ => None,
    }
}

fn openapi_page_schema(model: &str) -> String {
    format!(
        r##"{{"$ref":"#/components/schemas/{}"}}"##,
        escape_json(&openapi_page_component_name(model))
    )
}

fn openapi_page_component_name(model: &str) -> String {
    format!("NexusPage_{}", model)
}

fn openapi_page_component_schema(model: &str) -> String {
    format!(
        r##"{{"type":"object","properties":{{"total":{{"type":"integer"}},"items":{{"type":"array","items":{{"$ref":"#/components/schemas/{}"}}}}}},"required":["total","items"]}}"##,
        escape_json(model)
    )
}

fn openapi_response_schema_for_type(ty: &Type) -> String {
    match ty {
        Type::Array(inner) => {
            if let Type::Model(model) = inner.as_ref() {
                return openapi_list_schema(model);
            }
            openapi_schema_for_type(ty)
        }
        _ => openapi_schema_for_type(ty),
    }
}

fn openapi_list_schema(model: &str) -> String {
    format!(
        r##"{{"$ref":"#/components/schemas/{}"}}"##,
        escape_json(&openapi_list_component_name(model))
    )
}

fn openapi_list_component_name(model: &str) -> String {
    format!("NexusList_{}", model)
}

fn openapi_list_component_schema(model: &str) -> String {
    format!(
        r##"{{"type":"array","items":{{"$ref":"#/components/schemas/{}"}}}}"##,
        escape_json(model)
    )
}

fn infer_http_expr_type(
    program: &Program,
    expr: &Expr,
    params: &[String],
    query_params: &[QueryParam],
) -> Option<Type> {
    match expr {
        Expr::Integer { .. } => Some(Type::Int),
        Expr::Float { .. } => Some(Type::Float),
        Expr::StringLit { .. } => Some(Type::String),
        Expr::Bool { .. } => Some(Type::Bool),
        Expr::Money { .. } => Some(Type::Money),
        Expr::Nil { .. } => Some(Type::Nil),
        Expr::Array { items, .. } => {
            let item_ty = items
                .first()
                .and_then(|item| infer_http_expr_type(program, item, params, query_params))
                .unwrap_or(Type::Unknown);
            Some(Type::Array(Box::new(item_ty)))
        }
        Expr::Object { model, .. } => Some(Type::Model(model.clone())),
        Expr::Ident { name, .. } if params.iter().any(|param| param == name) => Some(Type::String),
        Expr::Ident { name, .. } => query_params
            .iter()
            .find(|param| param.name == *name)
            .map(|param| param.ty.clone()),
        Expr::FieldAccess { object, field, .. } => {
            let Type::Model(model) = infer_http_expr_type(program, object, params, query_params)?
            else {
                return None;
            };
            model_fields(program, &model)?
                .iter()
                .find(|candidate| candidate.name == *field)
                .map(|candidate| candidate.ty.clone())
        }
        Expr::BinOp {
            left,
            op: BinOp::Add,
            right,
            ..
        } => {
            let left = infer_http_expr_type(program, left, params, query_params)?;
            let right = infer_http_expr_type(program, right, params, query_params)?;
            infer_add_type(&left, &right)
        }
        Expr::BinOp { .. } | Expr::UnaryOp { .. } => None,
        Expr::Call { name, args, .. } if name == "str" && args.len() == 1 => Some(Type::String),
        Expr::Call { .. } => None,
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "all" && (args.is_empty() || args.len() == 2 || args.len() == 4) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "page" && page_args_have_pagination(args) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "create" && args.is_empty() => Some(Type::Model(ty.clone())),
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "find" && args.len() == 2 => Some(Type::Model(ty.clone())),
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where" && (args.len() == 2 || args.len() == 4 || args.len() == 6) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_page" && where_page_args_have_pagination(args) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_not_page" && where_page_args_have_pagination(args) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_not" && (args.len() == 2 || args.len() == 4 || args.len() == 6) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_not_in_page" && where_page_args_have_pagination(args) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_not_in"
            && (args.len() == 2 || args.len() == 4 || args.len() == 6) =>
        {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_not_in_optional_page" && where_page_args_have_pagination(args) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_not_in_optional"
            && (args.len() == 2 || args.len() == 4 || args.len() == 6) =>
        {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_optional_page" && where_page_args_have_pagination(args) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_optional"
            && (args.len() == 2 || args.len() == 4 || args.len() == 6) =>
        {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_in" && (args.len() == 2 || args.len() == 4 || args.len() == 6) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_in_page" && where_page_args_have_pagination(args) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_in_optional"
            && (args.len() == 2 || args.len() == 4 || args.len() == 6) =>
        {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_in_optional_page" && where_page_args_have_pagination(args) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_compare_page" && advanced_page_args_have_pagination(args) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_compare"
            && (args.len() == 3 || args.len() == 5 || args.len() == 7) =>
        {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_text_page" && advanced_page_args_have_pagination(args) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_text" && (args.len() == 3 || args.len() == 5 || args.len() == 7) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_between_page" && advanced_page_args_have_pagination(args) => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_between"
            && (args.len() == 3 || args.len() == 5 || args.len() == 7) =>
        {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_all_page" && where_all_page_filter_arg_count(args).is_some() => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_all" && where_all_filter_arg_count(args).is_some() => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_any_page" && where_all_page_filter_arg_count(args).is_some() => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "where_any" && where_all_filter_arg_count(args).is_some() => {
            Some(Type::Array(Box::new(Type::Model(ty.clone()))))
        }
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "update" && args.len() == 2 => Some(Type::Model(ty.clone())),
        Expr::StaticCall {
            ty, method, args, ..
        } if method == "delete" && args.len() == 2 => Some(Type::Model(ty.clone())),
        Expr::StaticCall { .. } => None,
    }
}

fn infer_add_type(left: &Type, right: &Type) -> Option<Type> {
    if matches!(left, Type::String) || matches!(right, Type::String) {
        return Some(Type::String);
    }

    match (left, right) {
        (Type::Money, Type::Money) => Some(Type::Money),
        (Type::Int, Type::Int) => Some(Type::Int),
        (Type::Int, Type::Float) | (Type::Float, Type::Int) | (Type::Float, Type::Float) => {
            Some(Type::Float)
        }
        _ => None,
    }
}

fn openapi_schema_for_type(ty: &Type) -> String {
    match ty {
        Type::String => r#"{"type":"string"}"#.to_string(),
        Type::Int => r#"{"type":"integer"}"#.to_string(),
        Type::Float => r#"{"type":"number"}"#.to_string(),
        Type::Bool => r#"{"type":"boolean"}"#.to_string(),
        Type::Money => r#"{"type":"object","properties":{"amount":{"type":"number"},"currency":{"type":"string"}},"required":["amount","currency"]}"#.to_string(),
        Type::Date => r#"{"type":"string","format":"date"}"#.to_string(),
        Type::Array(inner) => format!(r#"{{"type":"array","items":{}}}"#, openapi_schema_for_type(inner)),
        Type::Optional(inner) => schema_with_property(openapi_schema_for_type(inner), "nullable", "true".to_string()),
        Type::Model(name) => format!(r##"{{"$ref":"#/components/schemas/{}"}}"##, escape_json(name)),
        Type::Nil | Type::Void | Type::Unknown => "{}".to_string(),
    }
}

fn openapi_query_param_schema_for_type(ty: &Type) -> String {
    match ty {
        Type::Optional(inner) => schema_with_property(
            openapi_query_param_schema_for_type(inner),
            "nullable",
            "true".to_string(),
        ),
        Type::Array(inner) => format!(
            r#"{{"type":"array","items":{}}}"#,
            openapi_query_param_schema_for_type(inner)
        ),
        Type::Money => {
            r#"{"type":"string","format":"nexus-money","example":"1000:kz"}"#.to_string()
        }
        _ => openapi_schema_for_type(ty),
    }
}

fn query_param_is_array(ty: &Type) -> bool {
    match ty {
        Type::Optional(inner) => query_param_is_array(inner),
        Type::Array(_) => true,
        _ => false,
    }
}

fn schema_with_property(schema: String, name: &str, value: String) -> String {
    if schema == "{}" {
        return format!(r#"{{"{}":{}}}"#, name, value);
    }

    let Some(schema) = schema.strip_suffix('}') else {
        return schema;
    };
    format!(r#"{schema},"{}":{}}}"#, name, value)
}

fn openapi_query_default_value(ty: &Type, expr: &Expr) -> Option<String> {
    match (ty, expr) {
        (Type::Optional(_), Expr::Nil { .. }) => Some("null".to_string()),
        (Type::Optional(inner), _) => openapi_query_default_value(inner, expr),
        (Type::Array(inner), Expr::Array { items, .. }) => {
            let mut values = Vec::new();
            for item in items {
                values.push(openapi_query_default_value(inner, item)?);
            }
            Some(format!("[{}]", values.join(",")))
        }
        (
            Type::Money,
            Expr::Money {
                value, currency, ..
            },
        ) => Some(format!(
            r#""{}:{}""#,
            format_number(*value),
            escape_json(currency)
        )),
        _ => openapi_default_value(expr),
    }
}

fn openapi_min_max_property(
    ty: &Type,
    constraint: &str,
    expr: &Expr,
) -> Option<(&'static str, String)> {
    match ty {
        Type::Optional(inner) => openapi_min_max_property(inner, constraint, expr),
        Type::String => match (constraint, expr) {
            ("min", Expr::Integer { value, .. }) => Some(("minLength", value.to_string())),
            ("max", Expr::Integer { value, .. }) => Some(("maxLength", value.to_string())),
            _ => None,
        },
        Type::Int | Type::Float => match constraint {
            "min" => Some(("minimum", openapi_default_value(expr)?)),
            "max" => Some(("maximum", openapi_default_value(expr)?)),
            _ => None,
        },
        Type::Money | Type::Date => match constraint {
            "min" => Some(("x-nexus-min", openapi_default_value(expr)?)),
            "max" => Some(("x-nexus-max", openapi_default_value(expr)?)),
            _ => None,
        },
        _ => None,
    }
}

fn openapi_default_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Integer { value, .. } => Some(value.to_string()),
        Expr::Float { value, .. } => Some(format_number(*value)),
        Expr::StringLit { value, .. } => Some(format!(r#""{}""#, escape_json(value))),
        Expr::Bool { value, .. } => Some(value.to_string()),
        Expr::Money {
            value, currency, ..
        } => Some(format!(
            r#"{{"amount":{},"currency":"{}"}}"#,
            format_number(*value),
            escape_json(currency)
        )),
        Expr::Array { items, .. } => {
            let mut values = Vec::new();
            for item in items {
                values.push(openapi_default_value(item)?);
            }
            Some(format!("[{}]", values.join(",")))
        }
        Expr::Nil { .. } => Some("null".to_string()),
        Expr::Object { .. }
        | Expr::FieldAccess { .. }
        | Expr::Ident { .. }
        | Expr::BinOp { .. }
        | Expr::UnaryOp { .. }
        | Expr::Call { .. }
        | Expr::StaticCall { .. } => None,
    }
}
