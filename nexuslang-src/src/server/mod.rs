pub mod http;
pub mod json;
pub mod openapi;
pub mod router;
pub mod sqlite;
pub mod storage;
pub mod storage_backend;

pub use http::{serve_file, HttpResponse};
pub use openapi::generate_openapi;
pub use storage_backend::{default_data_dir, Storage};

pub fn handle_request_for_test(
    source: &str,
    method: &str,
    path: &str,
    storage: &Storage,
) -> Result<HttpResponse, String> {
    let program = crate::parse_checked_source(source)?;
    Ok(router::handle_request(&program, storage, method, path, ""))
}

pub fn handle_request_with_body_for_test(
    source: &str,
    method: &str,
    path: &str,
    body: &str,
    storage: &Storage,
) -> Result<HttpResponse, String> {
    let program = crate::parse_checked_source(source)?;
    Ok(router::handle_request(
        &program, storage, method, path, body,
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::ast::*;
    use crate::parse_checked_source;

    use super::http::method_name;
    use super::openapi::*;
    use super::storage::*;

    type OpenApiValidation = (&'static str, fn(&JsonValue));

    const OPENAPI_QA_SOURCE: &str = r#"
model Customer {
    name: string unique
    status: string = "active" index
    balance: money min 100 kz max 5000 kz
    display_name: string min 2 max 80
    score: int min 0 max 100
    email: string?
}

route GET /customers/:name ?(active: bool = true) {
    return Customer::find("name", name)
}

route PUT /customers/:name {
    return Customer::update("name", name)
}

route POST /customers {
    return Customer::create()
}

route GET /customers/search ?(statuses: [string]) {
    return Customer::where_in("status", statuses)
}

route GET /customers/page ?(status: string?, limit: int = 10, offset: int = 0) {
    return Customer::where_optional_page("status", status, "name", "asc", limit, offset)
}
"#;

    fn representative_openapi() -> String {
        let program = parse_checked_source(OPENAPI_QA_SOURCE).unwrap();
        generate_openapi(&program)
    }

    fn parse_openapi_document(openapi: &str) -> JsonValue {
        match parse_json(openapi) {
            Ok(document @ JsonValue::Object(_)) => document,
            Ok(other) => panic!(
                "OpenAPI gerado deveria ter raiz object, encontrado {}",
                json_type_label(&other)
            ),
            Err(err) => panic!("OpenAPI gerado nao e JSON parseavel: {}\n{}", err, openapi),
        }
    }

    fn json_object_field<'a>(value: &'a JsonValue, name: &str) -> Option<&'a JsonValue> {
        let JsonValue::Object(fields) = value else {
            return None;
        };

        fields
            .iter()
            .find_map(|(key, value)| if key == name { Some(value) } else { None })
    }

    fn json_object_fields<'a>(value: &'a JsonValue, context: &str) -> &'a [(String, JsonValue)] {
        match value {
            JsonValue::Object(fields) => fields,
            other => panic!(
                "{} deveria ser object, encontrado {}",
                context,
                json_type_label(other)
            ),
        }
    }

    fn expect_object_field<'a>(value: &'a JsonValue, name: &str) -> &'a JsonValue {
        match json_object_field(value, name) {
            Some(field @ JsonValue::Object(_)) => field,
            Some(field) => panic!(
                "campo JSON '{}' deveria ser object, encontrado {}",
                name,
                json_type_label(field)
            ),
            None => panic!("campo JSON '{}' ausente", name),
        }
    }

    fn expect_json_field<'a>(value: &'a JsonValue, name: &str) -> &'a JsonValue {
        json_object_field(value, name).unwrap_or_else(|| panic!("campo JSON '{}' ausente", name))
    }

    fn expect_array_field<'a>(value: &'a JsonValue, name: &str) -> &'a [JsonValue] {
        match json_object_field(value, name) {
            Some(JsonValue::Array(items)) => items,
            Some(field) => panic!(
                "campo JSON '{}' deveria ser array, encontrado {}",
                name,
                json_type_label(field)
            ),
            None => panic!("campo JSON '{}' ausente", name),
        }
    }

    fn expect_bool_field_value(value: &JsonValue, name: &str) -> bool {
        match json_object_field(value, name) {
            Some(JsonValue::Bool(actual)) => *actual,
            Some(field) => panic!(
                "campo JSON '{}' deveria ser bool, encontrado {}",
                name,
                json_type_label(field)
            ),
            None => panic!("campo JSON '{}' ausente", name),
        }
    }

    fn expect_number_field_value(value: &JsonValue, name: &str) -> f64 {
        match json_object_field(value, name) {
            Some(JsonValue::Number(actual)) => *actual,
            Some(field) => panic!(
                "campo JSON '{}' deveria ser number, encontrado {}",
                name,
                json_type_label(field)
            ),
            None => panic!("campo JSON '{}' ausente", name),
        }
    }

    fn expect_string_value<'a>(value: &'a JsonValue, context: &str) -> &'a str {
        match value {
            JsonValue::String(actual) => actual,
            other => panic!(
                "{} deveria ser string, encontrado {}",
                context,
                json_type_label(other)
            ),
        }
    }

    fn expect_string_field_value<'a>(value: &'a JsonValue, name: &str) -> &'a str {
        match json_object_field(value, name) {
            Some(field) => expect_string_value(field, &format!("campo JSON '{}'", name)),
            None => panic!("campo JSON '{}' ausente", name),
        }
    }

    fn expect_string_field_present(value: &JsonValue, name: &str) {
        let actual = expect_string_field_value(value, name);
        assert!(
            !actual.is_empty(),
            "campo JSON '{}' deveria ser string nao vazia",
            name
        );
    }

    fn expect_string_field(value: &JsonValue, name: &str, expected: &str) {
        assert_eq!(expect_string_field_value(value, name), expected);
    }

    fn assert_json_field_absent(value: &JsonValue, name: &str, context: &str) {
        assert!(
            json_object_field(value, name).is_none(),
            "{} nao deveria conter campo JSON '{}'",
            context,
            name
        );
    }

    fn assert_string_array_field(value: &JsonValue, name: &str, expected: &[&str]) {
        let actual = expect_array_field(value, name)
            .iter()
            .map(|item| expect_string_value(item, &format!("item de {}", name)))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "campo JSON '{}' inesperado", name);
    }

    fn is_openapi_http_method(name: &str) -> bool {
        matches!(
            name,
            "get" | "put" | "post" | "delete" | "patch" | "options" | "head" | "trace"
        )
    }

    fn collect_json_refs<'a>(value: &'a JsonValue, refs: &mut Vec<&'a str>) {
        match value {
            JsonValue::Object(fields) => {
                for (key, field) in fields {
                    if key == "$ref" {
                        match field {
                            JsonValue::String(reference) => refs.push(reference),
                            other => panic!(
                                "campo JSON '$ref' deveria ser string, encontrado {}",
                                json_type_label(other)
                            ),
                        }
                    }

                    collect_json_refs(field, refs);
                }
            }
            JsonValue::Array(items) => {
                for item in items {
                    collect_json_refs(item, refs);
                }
            }
            JsonValue::String(_) | JsonValue::Number(_) | JsonValue::Bool(_) | JsonValue::Null => {}
        }
    }

    fn assert_component_ref_exists(document: &JsonValue, reference: &str) {
        let _ = component_ref_target(document, reference);
    }

    fn component_ref_target<'a>(document: &'a JsonValue, reference: &str) -> &'a JsonValue {
        let Some(rest) = reference.strip_prefix("#/components/") else {
            panic!(
                "OpenAPI $ref '{}' deveria apontar para #/components/...",
                reference
            );
        };

        let mut parts = rest.split('/');
        let bucket = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default();
        assert!(
            !bucket.is_empty() && !name.is_empty() && parts.next().is_none(),
            "OpenAPI $ref '{}' deveria usar #/components/<bucket>/<name>",
            reference
        );

        let components = expect_object_field(document, "components");
        let bucket_value = expect_object_field(components, bucket);
        json_object_field(bucket_value, name).unwrap_or_else(|| {
            panic!(
                "OpenAPI $ref '{}' aponta para componente inexistente",
                reference
            )
        })
    }

    fn expect_ref_value<'a>(value: &'a JsonValue, context: &str) -> &'a str {
        match json_object_field(value, "$ref") {
            Some(field) => expect_string_value(field, &format!("{}.$ref", context)),
            None => panic!("{} deveria conter $ref", context),
        }
    }

    fn resolve_component_ref_value<'a>(
        document: &'a JsonValue,
        value: &'a JsonValue,
        context: &str,
    ) -> &'a JsonValue {
        if let Some(reference) = json_object_field(value, "$ref") {
            let reference = expect_string_value(reference, &format!("{}.$ref", context));
            component_ref_target(document, reference)
        } else {
            value
        }
    }

    fn openapi_json_content_schema<'a>(value: &'a JsonValue, _context: &str) -> &'a JsonValue {
        let content = expect_object_field(value, "content");
        let json_content = expect_object_field(content, "application/json");
        expect_object_field(json_content, "schema")
    }

    fn assert_openapi_json_schema_content(value: &JsonValue, context: &str) {
        let schema = openapi_json_content_schema(value, context);
        assert!(
            !json_object_fields(schema, context).is_empty(),
            "{} deveria conter schema JSON nao vazio",
            context
        );
    }

    fn openapi_response_schema<'a>(
        document: &'a JsonValue,
        response: &'a JsonValue,
        context: &str,
    ) -> &'a JsonValue {
        let response = resolve_component_ref_value(document, response, context);
        openapi_json_content_schema(response, context)
    }

    fn assert_json_matches_source(actual: &JsonValue, expected_source: &str, context: &str) {
        let expected = parse_json(expected_source).unwrap_or_else(|err| {
            panic!(
                "{} tem schema esperado invalido: {}\n{}",
                context, err, expected_source
            )
        });

        assert_eq!(
            json_value_json(actual),
            json_value_json(&expected),
            "{} deveria bater com o schema esperado",
            context
        );
    }

    fn openapi_operation_for_route<'a>(
        document: &'a JsonValue,
        route: &RouteView<'_>,
    ) -> &'a JsonValue {
        let paths = expect_object_field(document, "paths");
        let openapi_path = route.path.replace(':', "{").replace_segments_for_openapi();
        let path_item = json_object_field(paths, &openapi_path)
            .unwrap_or_else(|| panic!("OpenAPI path '{}' ausente", openapi_path));
        let method = method_name(route.method).to_lowercase();
        json_object_field(path_item, &method)
            .unwrap_or_else(|| panic!("OpenAPI operation '{} {}' ausente", method, openapi_path))
    }

    fn assert_operation_request_body_matches_route(
        document: &JsonValue,
        operation: &JsonValue,
        route: &RouteView<'_>,
        context: &str,
    ) {
        let Some(model) = route_request_body_model(route) else {
            assert_json_field_absent(operation, "requestBody", context);
            return;
        };

        let request_body = expect_json_field(operation, "requestBody");
        let request_body_context = format!("{}.requestBody", context);
        let actual_ref = expect_ref_value(request_body, &request_body_context);
        let expected_ref = format!(
            "#/components/requestBodies/{}",
            openapi_request_body_component_name(&model)
        );
        assert_eq!(
            actual_ref, expected_ref,
            "{} deveria apontar para requestBody do model real",
            request_body_context
        );

        let component = component_ref_target(document, actual_ref);
        assert!(
            expect_bool_field_value(component, "required"),
            "{} deveria ser required",
            request_body_context
        );
        let schema = openapi_json_content_schema(component, &request_body_context);
        let expected_schema = format!(
            r##"{{"$ref":"#/components/schemas/{}"}}"##,
            escape_json(&model)
        );
        assert_json_matches_source(schema, &expected_schema, &request_body_context);
    }

    fn expected_route_response_status(
        program: &Program,
        route: &RouteView<'_>,
        status: &str,
    ) -> bool {
        match status {
            "200" => route_response_status(route) == "200",
            "201" => route_response_status(route) == "201",
            "400" => route_has_bad_request_response(route),
            "404" => route_has_not_found_response(route),
            "409" => route_has_conflict_response(program, route),
            _ => false,
        }
    }

    fn assert_error_response_matches_component_schema(
        document: &JsonValue,
        response: &JsonValue,
        status: &str,
        description: &str,
        context: &str,
    ) {
        let response = resolve_component_ref_value(document, response, context);
        expect_string_field(response, "description", description);
        let schema = openapi_json_content_schema(response, context);
        assert_json_matches_source(
            schema,
            r##"{"$ref":"#/components/schemas/NexusError"}"##,
            &format!("{} response {}", context, status),
        );
    }

    fn assert_operation_responses_match_route(
        document: &JsonValue,
        program: &Program,
        operation: &JsonValue,
        route: &RouteView<'_>,
        context: &str,
    ) {
        let responses = expect_object_field(operation, "responses");

        for status in ["200", "201", "400", "404", "409"] {
            let actual = json_object_field(responses, status);
            let expected = expected_route_response_status(program, route, status);
            assert_eq!(
                actual.is_some(),
                expected,
                "{} response {} nao bate com o contrato real da route",
                context,
                status
            );
        }

        let success_status = route_response_status(route);
        let success_context = format!("{} response {}", context, success_status);
        let success_response = expect_json_field(responses, success_status);
        let resolved_success =
            resolve_component_ref_value(document, success_response, &success_context);
        expect_string_field(
            resolved_success,
            "description",
            if success_status == "201" {
                "Created"
            } else {
                "OK"
            },
        );
        let success_schema = openapi_response_schema(document, success_response, &success_context);
        let expected_schema = route_response_schema(program, route);
        assert_json_matches_source(success_schema, &expected_schema, &success_context);

        for (status, description) in [
            ("400", "Bad Request"),
            ("404", "Not Found"),
            ("409", "Conflict"),
        ] {
            if let Some(response) = json_object_field(responses, status) {
                assert_error_response_matches_component_schema(
                    document,
                    response,
                    status,
                    description,
                    context,
                );
            }
        }
    }

    fn assert_openapi_document_has_minimum_structure(document: &JsonValue) {
        expect_string_field(document, "openapi", "3.0.0");
        let info = expect_object_field(document, "info");
        expect_string_field(info, "title", "NexusLang API");
        expect_string_field(info, "version", "0.1.0");
        expect_array_field(document, "tags");
        expect_object_field(document, "paths");

        let components = expect_object_field(document, "components");
        for name in ["schemas", "parameters", "requestBodies", "responses"] {
            expect_object_field(components, name);
        }
    }

    fn assert_openapi_paths_and_operations_have_minimum_structure(document: &JsonValue) {
        let paths = expect_object_field(document, "paths");
        let path_items = json_object_fields(paths, "paths");

        assert!(!path_items.is_empty(), "OpenAPI deveria conter paths");
        for (path, path_item) in path_items {
            assert!(
                path.starts_with('/'),
                "OpenAPI path '{}' deveria comecar com '/'",
                path
            );

            let operations = json_object_fields(path_item, path);
            assert!(
                !operations.is_empty(),
                "Path Item '{}' deveria conter ao menos uma operation",
                path
            );

            for (method, operation) in operations {
                assert!(
                    is_openapi_http_method(method),
                    "Path Item '{}' contem metodo OpenAPI invalido '{}'",
                    path,
                    method
                );

                let context = format!("operation {} {}", method, path);
                json_object_fields(operation, &context);
                expect_string_field_present(operation, "summary");
                expect_string_field_present(operation, "operationId");
                assert!(
                    !expect_array_field(operation, "tags").is_empty(),
                    "{} deveria conter ao menos uma tag",
                    context
                );
                expect_array_field(operation, "parameters");
                let responses = expect_object_field(operation, "responses");
                assert!(
                    !json_object_fields(responses, "responses").is_empty(),
                    "{} deveria conter ao menos uma response",
                    context
                );
            }
        }
    }

    fn assert_openapi_reusable_components_have_minimum_structure(document: &JsonValue) {
        let components = expect_object_field(document, "components");

        let schemas = expect_object_field(components, "schemas");
        for (name, schema) in json_object_fields(schemas, "components.schemas") {
            let context = format!("components.schemas.{}", name);
            assert!(
                !json_object_fields(schema, &context).is_empty(),
                "{} deveria ser object nao vazio",
                context
            );
        }

        let parameters = expect_object_field(components, "parameters");
        for (name, parameter) in json_object_fields(parameters, "components.parameters") {
            let context = format!("components.parameters.{}", name);
            json_object_fields(parameter, &context);
            expect_string_field_present(parameter, "name");
            let location = expect_string_field_value(parameter, "in");
            assert!(
                matches!(location, "path" | "query" | "header" | "cookie"),
                "{} usa local OpenAPI invalido '{}'",
                context,
                location
            );
            let required = expect_bool_field_value(parameter, "required");
            if location == "path" {
                assert!(required, "{} path parameter deveria ser required", context);
            }
            assert!(
                matches!(expect_json_field(parameter, "schema"), JsonValue::Object(_)),
                "{} deveria conter schema object",
                context
            );
        }

        let request_bodies = expect_object_field(components, "requestBodies");
        for (name, request_body) in json_object_fields(request_bodies, "components.requestBodies") {
            let context = format!("components.requestBodies.{}", name);
            json_object_fields(request_body, &context);
            assert_openapi_json_schema_content(request_body, &context);
        }

        let responses = expect_object_field(components, "responses");
        for (name, response) in json_object_fields(responses, "components.responses") {
            let context = format!("components.responses.{}", name);
            json_object_fields(response, &context);
            expect_string_field_present(response, "description");
            if json_object_field(response, "content").is_some() {
                assert_openapi_json_schema_content(response, &context);
            }
        }
    }

    fn assert_openapi_model_schemas_match_nexuslang_semantics(document: &JsonValue) {
        let components = expect_object_field(document, "components");
        let schemas = expect_object_field(components, "schemas");
        let customer = expect_object_field(schemas, "Customer");
        let customer_context = "components.schemas.Customer";

        expect_string_field(customer, "type", "object");
        let properties = expect_object_field(customer, "properties");
        for field in [
            "name",
            "status",
            "balance",
            "display_name",
            "score",
            "email",
        ] {
            expect_object_field(properties, field);
        }
        assert_string_array_field(
            customer,
            "required",
            &["name", "balance", "display_name", "score"],
        );

        let name = expect_object_field(properties, "name");
        expect_string_field(name, "type", "string");
        assert!(expect_bool_field_value(name, "x-nexus-unique"));
        assert_json_field_absent(name, "nullable", customer_context);

        let status = expect_object_field(properties, "status");
        expect_string_field(status, "type", "string");
        expect_string_field(status, "default", "active");
        assert!(expect_bool_field_value(status, "x-nexus-index"));

        let balance = expect_object_field(properties, "balance");
        expect_string_field(balance, "type", "object");
        let balance_properties = expect_object_field(balance, "properties");
        expect_string_field(
            expect_object_field(balance_properties, "amount"),
            "type",
            "number",
        );
        expect_string_field(
            expect_object_field(balance_properties, "currency"),
            "type",
            "string",
        );
        assert_string_array_field(balance, "required", &["amount", "currency"]);
        let balance_min = expect_object_field(balance, "x-nexus-min");
        assert_eq!(expect_number_field_value(balance_min, "amount"), 100.0);
        expect_string_field(balance_min, "currency", "kz");
        let balance_max = expect_object_field(balance, "x-nexus-max");
        assert_eq!(expect_number_field_value(balance_max, "amount"), 5000.0);
        expect_string_field(balance_max, "currency", "kz");

        let display_name = expect_object_field(properties, "display_name");
        expect_string_field(display_name, "type", "string");
        assert_eq!(expect_number_field_value(display_name, "minLength"), 2.0);
        assert_eq!(expect_number_field_value(display_name, "maxLength"), 80.0);

        let score = expect_object_field(properties, "score");
        expect_string_field(score, "type", "integer");
        assert_eq!(expect_number_field_value(score, "minimum"), 0.0);
        assert_eq!(expect_number_field_value(score, "maximum"), 100.0);

        let email = expect_object_field(properties, "email");
        expect_string_field(email, "type", "string");
        assert!(expect_bool_field_value(email, "nullable"));
    }

    fn assert_openapi_operations_match_route_contracts_and_components(
        document: &JsonValue,
        program: &Program,
    ) {
        let mut route_count = 0;

        for route in routes(program) {
            route_count += 1;
            let operation = openapi_operation_for_route(document, &route);
            let context = format!("operation {} {}", method_name(route.method), route.path);

            assert_operation_request_body_matches_route(document, operation, &route, &context);
            assert_operation_responses_match_route(document, program, operation, &route, &context);
        }

        assert!(
            route_count > 0,
            "OpenAPI deveria validar ao menos uma route"
        );
    }

    fn assert_openapi_component_refs_resolve(document: &JsonValue) {
        let mut refs = Vec::new();

        collect_json_refs(document, &mut refs);

        assert!(
            !refs.is_empty(),
            "OpenAPI gerado deveria conter referencias internas"
        );
        for reference in refs {
            assert_component_ref_exists(document, reference);
        }
    }

    fn assert_openapi_operation_ids_are_unique_and_tags_are_declared(document: &JsonValue) {
        let mut declared_tags = HashSet::new();
        let mut operation_ids = HashSet::new();
        let mut operation_count = 0;

        for tag in expect_array_field(document, "tags") {
            let name = expect_string_field_value(tag, "name");
            assert!(!name.is_empty(), "top-level tag deveria ter name nao vazio");
            assert!(
                declared_tags.insert(name),
                "top-level tag '{}' declarada mais de uma vez",
                name
            );
        }
        assert!(
            !declared_tags.is_empty(),
            "OpenAPI deveria declarar tags top-level"
        );

        let paths = expect_object_field(document, "paths");
        for (path, path_item) in json_object_fields(paths, "paths") {
            for (method, operation) in json_object_fields(path_item, path) {
                assert!(
                    is_openapi_http_method(method),
                    "Path Item '{}' contem metodo OpenAPI invalido '{}'",
                    path,
                    method
                );

                operation_count += 1;
                let context = format!("operation {} {}", method, path);
                let operation_id = expect_string_field_value(operation, "operationId");
                assert!(
                    !operation_id.is_empty(),
                    "{} deveria ter operationId nao vazio",
                    context
                );
                assert!(
                    operation_ids.insert(operation_id),
                    "{} reutiliza operationId '{}'",
                    context,
                    operation_id
                );

                let tags = expect_array_field(operation, "tags");
                assert!(!tags.is_empty(), "{} deveria conter tags", context);
                for tag in tags {
                    let tag_name = expect_string_value(tag, "operation tag");
                    assert!(
                        declared_tags.contains(tag_name),
                        "{} usa tag '{}' ausente de tags top-level",
                        context,
                        tag_name
                    );
                }
            }
        }

        assert!(
            operation_count > 0,
            "OpenAPI deveria conter ao menos uma operation"
        );
    }

    fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
        if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_string()
        } else if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else {
            "panic sem mensagem".to_string()
        }
    }

    fn capture_openapi_qa_failure(check: impl FnOnce()) -> Option<String> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(check))
            .err()
            .map(|payload| panic_payload_message(payload.as_ref()))
    }

    #[test]
    fn openapi_generated_document_is_json_parseable() {
        let openapi = representative_openapi();

        let _document = parse_openapi_document(&openapi);
    }

    #[test]
    fn openapi_generated_document_has_minimum_structure() {
        let openapi = representative_openapi();
        let document = parse_openapi_document(&openapi);

        assert_openapi_document_has_minimum_structure(&document);
    }

    #[test]
    fn openapi_generated_paths_and_operations_have_minimum_structure() {
        let openapi = representative_openapi();
        let document = parse_openapi_document(&openapi);

        assert_openapi_paths_and_operations_have_minimum_structure(&document);
    }

    #[test]
    fn openapi_generated_reusable_components_have_minimum_structure() {
        let openapi = representative_openapi();
        let document = parse_openapi_document(&openapi);

        assert_openapi_reusable_components_have_minimum_structure(&document);
    }

    #[test]
    fn openapi_generated_model_schemas_match_nexuslang_semantics() {
        let openapi = representative_openapi();
        let document = parse_openapi_document(&openapi);

        assert_openapi_model_schemas_match_nexuslang_semantics(&document);
    }

    #[test]
    fn openapi_generated_operations_match_route_contracts_and_components() {
        let program = parse_checked_source(OPENAPI_QA_SOURCE).unwrap();
        let openapi = generate_openapi(&program);
        let document = parse_openapi_document(&openapi);

        assert_openapi_operations_match_route_contracts_and_components(&document, &program);
    }

    #[test]
    fn openapi_generated_component_refs_resolve() {
        let openapi = representative_openapi();
        let document = parse_openapi_document(&openapi);

        assert_openapi_component_refs_resolve(&document);
    }

    #[test]
    fn openapi_generated_operation_ids_are_unique_and_tags_are_declared() {
        let openapi = representative_openapi();
        let document = parse_openapi_document(&openapi);

        assert_openapi_operation_ids_are_unique_and_tags_are_declared(&document);
    }

    #[test]
    fn openapi_1_0_contract_coherence_suite_runs_core_validations() {
        let program = parse_checked_source(OPENAPI_QA_SOURCE).unwrap();
        let openapi = generate_openapi(&program);
        let document = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            parse_openapi_document(&openapi)
        })) {
            Ok(document) => document,
            Err(payload) => panic!(
                "OpenAPI 1.0 coherence suite falhou em JSON parseavel: {}\n\nOpenAPI:\n{}",
                panic_payload_message(payload.as_ref()),
                openapi
            ),
        };

        let validations: [OpenApiValidation; 6] = [
            (
                "estrutura raiz minima",
                assert_openapi_document_has_minimum_structure,
            ),
            (
                "paths e operations minimos",
                assert_openapi_paths_and_operations_have_minimum_structure,
            ),
            (
                "componentes reutilizaveis minimos",
                assert_openapi_reusable_components_have_minimum_structure,
            ),
            (
                "schemas de models seguem semantica NexusLang",
                assert_openapi_model_schemas_match_nexuslang_semantics,
            ),
            (
                "refs internas resolvem",
                assert_openapi_component_refs_resolve,
            ),
            (
                "operationIds unicos e tags declaradas",
                assert_openapi_operation_ids_are_unique_and_tags_are_declared,
            ),
        ];
        let mut failures = Vec::new();

        for (name, validation) in validations {
            if let Some(message) = capture_openapi_qa_failure(|| validation(&document)) {
                failures.push(format!("- {}: {}", name, message));
            }
        }
        if let Some(message) = capture_openapi_qa_failure(|| {
            assert_openapi_operations_match_route_contracts_and_components(&document, &program)
        }) {
            failures.push(format!(
                "- operations batem com routes e componentes: {}",
                message
            ));
        }

        assert!(
            failures.is_empty(),
            "OpenAPI 1.0 coherence suite falhou:\n{}",
            failures.join("\n")
        );
    }
}
