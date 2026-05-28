use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::auth_ops::{
    AuthOperationRequestBodyKind, AuthOperationReturnKind, AuthStaticOperation,
    CheckedAuthOperationArgs,
};
use crate::model_ops::{
    CheckedModelOperationArgs, ModelOperationOpenApiFeature, ModelStaticOperation,
};
use crate::route_hir::{
    checked_routes, CheckedRouteAuthOperation, CheckedRouteExpr, CheckedRouteModelOperation,
    CheckedRouteView,
};

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

    for route in checked_routes(program) {
        let openapi_path = route.path.replace(':', "{").replace_segments_for_openapi();
        let params = route_parameters(&route, &parameter_components);
        let schema = route_response_schema(program, &route);
        let response_status = route_response_status(&route);
        let success_response_ref = openapi_success_response_ref(response_status, &schema);
        let request_body = route_request_body_ref(program, &route);
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
        if route_has_auth_rate_limit_response(&route) {
            operation.push_str(&openapi_error_response("429", "Too Many Requests"));
        }
        if route.auth.is_some() {
            operation.push_str(&openapi_error_response("401", "Unauthorized"));
        }
        if route.auth.and_then(|guard| guard.role.as_ref()).is_some() || route_requires_csrf(&route)
        {
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

    for route in checked_routes(program) {
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

fn openapi_component_request_bodies(program: &Program) -> Vec<(String, String)> {
    let mut components = Vec::new();
    let mut seen = HashSet::new();

    for route in checked_routes(program) {
        if let Some((name, body)) = route_request_body_component(program, &route) {
            if seen.insert(name.clone()) {
                components.push((name, body));
            }
        }
    }

    components
}

fn openapi_request_bodies(components: &[(String, String)]) -> String {
    components
        .iter()
        .map(|(name, body)| format!(r#""{}":{}"#, escape_json(name), body))
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

fn openapi_request_body_ref(component_name: &str) -> String {
    format!(
        r##"{{"$ref":"#/components/requestBodies/{}"}}"##,
        escape_json(component_name)
    )
}

fn openapi_component_responses(program: &Program) -> Vec<(String, String)> {
    let mut responses = Vec::new();
    let mut seen = HashSet::new();

    for route in checked_routes(program) {
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

pub(crate) fn route_response_schema(program: &Program, route: &CheckedRouteView<'_>) -> String {
    route
        .return_expr
        .as_ref()
        .map(|expr| openapi_checked_expr_schema(program, route, expr))
        .unwrap_or_else(|| "{}".to_string())
}

fn openapi_checked_expr_schema(
    program: &Program,
    route: &CheckedRouteView<'_>,
    expr: &CheckedRouteExpr<'_>,
) -> String {
    match expr {
        CheckedRouteExpr::AuthOperation(operation) => {
            openapi_auth_operation_schema(program, route, operation)
                .unwrap_or_else(|| "{}".to_string())
        }
        CheckedRouteExpr::ModelOperation(operation) => openapi_model_operation_schema(operation),
        CheckedRouteExpr::Expr(expr) => {
            openapi_expr_schema(program, expr, route.params, route.query_params)
        }
    }
}

fn openapi_auth_operation_schema(
    program: &Program,
    route: &CheckedRouteView<'_>,
    operation: &CheckedRouteAuthOperation<'_>,
) -> Option<String> {
    match operation.operation.return_kind() {
        AuthOperationReturnKind::AuthSuccess => operation.checked_args.and_then(|args| {
            auth_config_from_checked_args(program, args).map(|config| {
                format!(
                    r#"{{"type":"object","properties":{{"user":{},"token":{{"type":"string"}},"csrf_token":{{"type":"string"}},"expires_in":{{"type":"integer"}}}}}}"#,
                    openapi_response_schema_for_type(&Type::Model(config.model.clone()))
                )
            })
        }),
        AuthOperationReturnKind::CurrentUser => route
            .auth
            .and_then(|guard| auth_config(program, &guard.auth))
            .map(|config| openapi_response_schema_for_type(&Type::Model(config.model.clone()))),
        AuthOperationReturnKind::Bool => Some(r#"{"type":"boolean"}"#.to_string()),
    }
}

fn openapi_model_operation_schema(operation: &CheckedRouteModelOperation<'_>) -> String {
    if operation
        .checked_args
        .is_some_and(CheckedModelOperationArgs::has_page_response)
    {
        openapi_page_schema(operation.model)
    } else {
        openapi_response_schema_for_type(&operation.operation.return_type(operation.model))
    }
}

fn auth_config_from_checked_args<'a>(
    program: &'a Program,
    args: CheckedAuthOperationArgs<'_>,
) -> Option<&'a AuthConfig> {
    args.auth_config_name()
        .and_then(|name| auth_config(program, name))
}

fn route_parameters(
    route: &CheckedRouteView<'_>,
    components: &OpenApiParameterComponents,
) -> String {
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

    if route_requires_csrf(route) {
        params.push(openapi_csrf_header_parameter());
    }

    params.join(",")
}

fn route_requires_csrf(route: &CheckedRouteView<'_>) -> bool {
    route.auth.is_some()
        && matches!(
            route.method,
            HttpMethod::Post | HttpMethod::Put | HttpMethod::Delete
        )
}

fn openapi_csrf_header_parameter() -> String {
    r#"{"name":"X-Nexus-CSRF-Token","in":"header","required":false,"description":"Required when authenticating unsafe methods with the Nexus session cookie. Bearer token requests do not need this header.","schema":{"type":"string"}}"#.to_string()
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

fn route_request_body_ref(program: &Program, route: &CheckedRouteView<'_>) -> Option<String> {
    route_request_body_component_name(program, route).map(|name| openapi_request_body_ref(&name))
}

pub(crate) fn route_request_body_model(route: &CheckedRouteView<'_>) -> Option<String> {
    let Some(CheckedRouteExpr::ModelOperation(operation)) = route.return_expr else {
        return None;
    };
    if operation.operation.uses_request_body() && operation.checked_args.is_some() {
        Some(operation.model.to_string())
    } else {
        None
    }
}

pub(crate) fn route_request_body_component_name(
    program: &Program,
    route: &CheckedRouteView<'_>,
) -> Option<String> {
    route_request_body_component(program, route).map(|(name, _)| name)
}

fn route_request_body_component(
    program: &Program,
    route: &CheckedRouteView<'_>,
) -> Option<(String, String)> {
    if let Some(model) = route_request_body_model(route) {
        return Some((
            openapi_request_body_component_name(&model),
            openapi_request_body_component(&model),
        ));
    }

    let Some(CheckedRouteExpr::AuthOperation(operation)) = route.return_expr else {
        return None;
    };
    if !operation.operation.uses_request_body() {
        return None;
    }
    let checked_args = operation.checked_args?;
    let config = auth_config_from_checked_args(program, checked_args)?;
    Some((
        openapi_auth_request_body_component_name(operation.operation, &config.name),
        openapi_auth_request_body_component(program, operation.operation, config),
    ))
}

pub(crate) fn openapi_auth_request_body_component_name(
    operation: AuthStaticOperation,
    auth_name: &str,
) -> String {
    format!(
        "NexusAuth{}RequestBody_{}",
        title_case_ascii(operation.method_name()),
        auth_name
    )
}

fn title_case_ascii(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    out.push(first.to_ascii_uppercase());
    out.extend(chars);
    out
}

fn openapi_auth_request_body_component(
    program: &Program,
    operation: AuthStaticOperation,
    config: &AuthConfig,
) -> String {
    format!(
        r#"{{"required":true,"content":{{"application/json":{{"schema":{}}}}}}}"#,
        openapi_auth_request_body_schema(program, operation, config)
    )
}

fn openapi_auth_request_body_schema(
    program: &Program,
    operation: AuthStaticOperation,
    config: &AuthConfig,
) -> String {
    match operation.request_body_kind() {
        Some(AuthOperationRequestBodyKind::Register) => {
            openapi_auth_register_request_schema(program, config)
        }
        Some(AuthOperationRequestBodyKind::Login) => openapi_auth_login_request_schema(config),
        None => "{}".to_string(),
    }
}

fn openapi_auth_register_request_schema(program: &Program, config: &AuthConfig) -> String {
    let fields = model_fields(program, &config.model).unwrap_or_default();
    let mut properties = fields
        .iter()
        .map(|field| {
            format!(
                r#""{}":{}"#,
                escape_json(&field.name),
                openapi_field_schema(field)
            )
        })
        .collect::<Vec<_>>();
    properties.push(format!(
        r#""password":{{"type":"string","minLength":{}}}"#,
        config.password_min
    ));

    let mut required = fields
        .iter()
        .filter(|field| field.default.is_none() && !type_is_optional(&field.ty))
        .map(|field| format!(r#""{}""#, escape_json(&field.name)))
        .collect::<Vec<_>>();
    required.push(r#""password""#.to_string());

    format!(
        r#"{{"type":"object","properties":{{{}}},"required":[{}]}}"#,
        properties.join(","),
        required.join(",")
    )
}

fn openapi_auth_login_request_schema(config: &AuthConfig) -> String {
    format!(
        r#"{{"type":"object","properties":{{"{}":{{"type":"string"}},"password":{{"type":"string"}}}},"required":["{}","password"]}}"#,
        escape_json(&config.identity),
        escape_json(&config.identity)
    )
}

pub(crate) fn route_response_status(route: &CheckedRouteView<'_>) -> &'static str {
    if let Some(CheckedRouteExpr::AuthOperation(operation)) = route.return_expr {
        return operation.operation.success_status_name();
    }

    let model_create = matches!(route.method, HttpMethod::Post)
        && route_has_model_operation(route, |operation, _args| operation.is_create());

    if model_create {
        "201"
    } else {
        "200"
    }
}

fn route_operation_id(route: &CheckedRouteView<'_>) -> String {
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

fn route_tag(route: &CheckedRouteView<'_>) -> String {
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

pub(crate) fn route_has_bad_request_response(route: &CheckedRouteView<'_>) -> bool {
    !route.query_params.is_empty()
        || route_has_model_operation(route, |operation, _args| operation.uses_request_body())
        || route_has_auth_operation(route, |operation| operation.has_bad_request_response())
}

pub(crate) fn route_has_not_found_response(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |operation, _args| operation.has_not_found_response())
}

pub(crate) fn route_has_conflict_response(program: &Program, route: &CheckedRouteView<'_>) -> bool {
    route_model_operation(route)
        .and_then(|operation| {
            if operation.operation.may_conflict_on_unique_fields()
                && operation.checked_args.is_some()
            {
                Some(operation.model)
            } else {
                None
            }
        })
        .and_then(|model| model_fields(program, model))
        .map(has_unique_fields)
        .unwrap_or(false)
}

fn route_has_auth_rate_limit_response(route: &CheckedRouteView<'_>) -> bool {
    route_has_auth_operation(route, AuthStaticOperation::has_rate_limit_response)
}

fn route_has_auth_operation(
    route: &CheckedRouteView<'_>,
    predicate: impl Fn(AuthStaticOperation) -> bool,
) -> bool {
    matches!(
        route.return_expr,
        Some(CheckedRouteExpr::AuthOperation(operation)) if predicate(operation.operation)
    )
}

fn route_has_pagination(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |_operation, args| args.has_pagination())
}

fn route_has_ordering(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |_operation, args| args.has_ordering())
}

fn route_has_total_count(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |_operation, args| args.has_page_response())
}

fn route_has_composite_filters(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |operation, args| {
        operation.has_openapi_feature_for_checked_args(
            ModelOperationOpenApiFeature::CompositeFilters,
            args,
        )
    })
}

fn route_has_or_filters(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |operation, args| {
        operation
            .has_openapi_feature_for_checked_args(ModelOperationOpenApiFeature::OrFilters, args)
    })
}

fn route_has_exclusion_filters(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |operation, args| {
        operation.has_openapi_feature_for_checked_args(
            ModelOperationOpenApiFeature::ExclusionFilters,
            args,
        )
    })
}

fn route_has_optional_filters(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |operation, args| {
        operation.has_openapi_feature_for_checked_args(
            ModelOperationOpenApiFeature::OptionalFilters,
            args,
        )
    })
}

fn route_has_in_filters(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |operation, args| {
        operation
            .has_openapi_feature_for_checked_args(ModelOperationOpenApiFeature::InFilters, args)
    })
}

fn route_has_comparison_filters(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |operation, args| {
        operation.has_openapi_feature_for_checked_args(
            ModelOperationOpenApiFeature::ComparisonFilters,
            args,
        )
    })
}

fn route_has_text_filters(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |operation, args| {
        operation
            .has_openapi_feature_for_checked_args(ModelOperationOpenApiFeature::TextFilters, args)
    })
}

fn route_has_range_filters(route: &CheckedRouteView<'_>) -> bool {
    route_has_model_operation(route, |operation, args| {
        operation
            .has_openapi_feature_for_checked_args(ModelOperationOpenApiFeature::RangeFilters, args)
    })
}

fn route_has_model_operation(
    route: &CheckedRouteView<'_>,
    predicate: impl for<'a> Fn(ModelStaticOperation, CheckedModelOperationArgs<'a>) -> bool,
) -> bool {
    route_model_operation(route)
        .and_then(|operation| {
            operation
                .checked_args
                .map(|args| predicate(operation.operation, args))
        })
        .unwrap_or(false)
}

fn route_model_operation<'a>(
    route: &CheckedRouteView<'a>,
) -> Option<CheckedRouteModelOperation<'a>> {
    match route.return_expr {
        Some(CheckedRouteExpr::ModelOperation(operation)) => Some(operation),
        _ => None,
    }
}

fn openapi_expr_schema(
    program: &Program,
    expr: &Expr,
    params: &[String],
    query_params: &[QueryParam],
) -> String {
    infer_http_expr_type(program, expr, params, query_params)
        .map(|ty| openapi_response_schema_for_type(&ty))
        .unwrap_or_else(|| "{}".to_string())
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
