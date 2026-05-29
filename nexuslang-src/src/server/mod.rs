pub mod auth;
pub mod http;
pub mod json;
pub mod openapi;
pub mod router;
pub mod sqlite;
pub mod storage;
pub mod storage_backend;

pub use http::{serve_file, serve_file_with_storage_driver, HttpResponse};
pub use openapi::generate_openapi;
pub use storage_backend::{
    default_data_dir, Storage, StorageDriver, StorageMigrationAction, StorageMigrationBlocker,
    StorageMigrationPlan, NEXUS_DATA_DIR_ENV,
};

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

pub fn handle_request_with_headers_and_body_for_test(
    source: &str,
    method: &str,
    path: &str,
    headers: &[(String, String)],
    body: &str,
    storage: &Storage,
) -> Result<HttpResponse, String> {
    let program = crate::parse_checked_source(source)?;
    Ok(router::handle_request_with_headers(
        &program, storage, method, path, headers, body,
    ))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, fs, path::PathBuf};

    use crate::ast::*;
    use crate::auth_ops::AuthStaticOperation;
    use crate::model_ops::ModelStaticOperation;
    use crate::parse_checked_source;
    use crate::route_hir::{checked_routes, CheckedRouteExpr, CheckedRouteView};

    use super::http::method_name;
    use super::openapi::*;
    use super::storage::*;
    use super::storage_backend::Storage;

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

    const MODEL_OPERATION_MATRIX_SOURCE: &str = r#"
model Customer {
    name: string unique
    status: string = "active" index
    tenant: string
    balance: float
}

route GET /contract/all {
    return Customer::all()
}

route GET /contract/page ?(limit: int = 2, offset: int = 1) {
    return Customer::page("name", "asc", limit, offset)
}

route POST /contract/create {
    return Customer::create()
}

route GET /contract/find/:name {
    return Customer::find("name", name)
}

route GET /contract/where ?(status: string) {
    return Customer::where("status", status)
}

route GET /contract/where-page ?(status: string, limit: int = 1, offset: int = 1) {
    return Customer::where_page("status", status, "name", "asc", limit, offset)
}

route GET /contract/where-not ?(status: string) {
    return Customer::where_not("status", status)
}

route GET /contract/where-not-page ?(status: string, limit: int = 1, offset: int = 1) {
    return Customer::where_not_page("status", status, "name", "asc", limit, offset)
}

route GET /contract/where-not-in ?(statuses: [string]) {
    return Customer::where_not_in("status", statuses)
}

route GET /contract/where-not-in-page ?(statuses: [string], limit: int = 1, offset: int = 0) {
    return Customer::where_not_in_page("status", statuses, "name", "asc", limit, offset)
}

route GET /contract/where-not-in-optional ?(statuses: [string]?) {
    return Customer::where_not_in_optional("status", statuses)
}

route GET /contract/where-not-in-optional-page ?(statuses: [string]?, limit: int = 1, offset: int = 0) {
    return Customer::where_not_in_optional_page("status", statuses, "name", "asc", limit, offset)
}

route GET /contract/where-optional ?(status: string?) {
    return Customer::where_optional("status", status)
}

route GET /contract/where-optional-page ?(status: string?, limit: int = 1, offset: int = 0) {
    return Customer::where_optional_page("status", status, "name", "asc", limit, offset)
}

route GET /contract/where-in ?(statuses: [string]) {
    return Customer::where_in("status", statuses)
}

route GET /contract/where-in-page ?(statuses: [string], limit: int = 2, offset: int = 1) {
    return Customer::where_in_page("status", statuses, "name", "asc", limit, offset)
}

route GET /contract/where-in-optional ?(statuses: [string]?) {
    return Customer::where_in_optional("status", statuses)
}

route GET /contract/where-in-optional-page ?(statuses: [string]?, limit: int = 2, offset: int = 1) {
    return Customer::where_in_optional_page("status", statuses, "name", "asc", limit, offset)
}

route GET /contract/where-compare ?(min: float) {
    return Customer::where_compare("balance", ">=", min)
}

route GET /contract/where-compare-page ?(min: float, limit: int = 1, offset: int = 1) {
    return Customer::where_compare_page("balance", ">=", min, "name", "asc", limit, offset)
}

route GET /contract/where-text ?(term: string) {
    return Customer::where_text("name", "contains", term)
}

route GET /contract/where-text-page ?(term: string, limit: int = 1, offset: int = 1) {
    return Customer::where_text_page("name", "contains", term, "name", "asc", limit, offset)
}

route GET /contract/where-between ?(min: float, max: float) {
    return Customer::where_between("balance", min, max)
}

route GET /contract/where-between-page ?(min: float, max: float, limit: int = 1, offset: int = 1) {
    return Customer::where_between_page("balance", min, max, "name", "asc", limit, offset)
}

route GET /contract/where-all ?(status: string, tenant: string) {
    return Customer::where_all("status", status, "tenant", tenant)
}

route GET /contract/where-all-page ?(status: string, tenant: string, limit: int = 1, offset: int = 0) {
    return Customer::where_all_page("status", status, "tenant", tenant, "name", "asc", limit, offset)
}

route GET /contract/where-any ?(status: string, tenant: string) {
    return Customer::where_any("status", status, "tenant", tenant)
}

route GET /contract/where-any-page ?(status: string, tenant: string, limit: int = 1, offset: int = 1) {
    return Customer::where_any_page("status", status, "tenant", tenant, "name", "asc", limit, offset)
}

route PUT /contract/update/:name {
    return Customer::update("name", name)
}

route DELETE /contract/delete/:name {
    return Customer::delete("name", name)
}
"#;

    const AUTH_OPERATION_MATRIX_SOURCE: &str = r#"
model ContractUser {
    email: string unique
    name: string
    role: string = "user" index
}

auth ContractAuth {
    model: ContractUser
    identity: email
    role: role
    password_min: 15
    session_ttl_minutes: 60
    idle_ttl_minutes: 10
}

route POST /contract/auth/register {
    return Auth::register(ContractAuth)
}

route POST /contract/auth/login {
    return Auth::login(ContractAuth)
}

route POST /contract/auth/logout auth(ContractAuth) {
    return Auth::logout()
}

route GET /contract/auth/user auth(ContractAuth) {
    return Auth::user()
}
"#;

    const MODEL_OPERATION_MATRIX_RECORDS: &str = r#"[{"name":"Ana","status":"active","tenant":"north","balance":120},{"name":"Bia","status":"active","tenant":"south","balance":80},{"name":"Cris","status":"blocked","tenant":"north","balance":220},{"name":"Dina","status":"prospect","tenant":"west","balance":40}]"#;
    const MODEL_OPERATION_OPENAPI_FLAGS: [&str; 11] = [
        "x-nexus-pagination",
        "x-nexus-total-count",
        "x-nexus-ordering",
        "x-nexus-composite-filters",
        "x-nexus-or-filters",
        "x-nexus-exclusion-filters",
        "x-nexus-optional-filters",
        "x-nexus-in-filters",
        "x-nexus-comparison-filters",
        "x-nexus-text-filters",
        "x-nexus-range-filters",
    ];

    struct ModelOperationContractCase {
        operation: ModelStaticOperation,
        label: &'static str,
        method: &'static str,
        route_path: &'static str,
        openapi_path: &'static str,
        http_path: &'static str,
        body: Option<&'static str>,
        expected_status: u16,
        expected_body: &'static str,
        success_status: &'static str,
        response_component: &'static str,
        request_body: bool,
        bad_request: bool,
        not_found: bool,
        conflict: bool,
        openapi_flags: &'static [&'static str],
    }

    struct AuthOperationContractCase {
        operation: AuthStaticOperation,
        label: &'static str,
        method: &'static str,
        route_path: &'static str,
        openapi_path: &'static str,
        http_path: &'static str,
        success_status: &'static str,
        auth_config: Option<&'static str>,
        request_body: bool,
        requires_auth: bool,
        csrf_header: bool,
        bad_request: bool,
        rate_limit: bool,
        forbidden: bool,
        expected_http_status: u16,
        expected_body_fragment: &'static str,
    }

    fn model_operation_contract_cases() -> [ModelOperationContractCase; 30] {
        [
            ModelOperationContractCase {
                operation: ModelStaticOperation::All,
                label: "all",
                method: "GET",
                route_path: "/contract/all",
                openapi_path: "/contract/all",
                http_path: "/contract/all",
                body: None,
                expected_status: 200,
                expected_body: MODEL_OPERATION_MATRIX_RECORDS,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: false,
                not_found: false,
                conflict: false,
                openapi_flags: &[],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::Page,
                label: "page",
                method: "GET",
                route_path: "/contract/page",
                openapi_path: "/contract/page",
                http_path: "/contract/page?limit=2&offset=1",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":4,"items":[{"name":"Bia","status":"active","tenant":"south","balance":80},{"name":"Cris","status":"blocked","tenant":"north","balance":220}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::Create,
                label: "create",
                method: "POST",
                route_path: "/contract/create",
                openapi_path: "/contract/create",
                http_path: "/contract/create",
                body: Some(r#"{"name":"Eva","tenant":"north","balance":160}"#),
                expected_status: 201,
                expected_body: r#"{"name":"Eva","status":"active","tenant":"north","balance":160}"#,
                success_status: "201",
                response_component: "Customer",
                request_body: true,
                bad_request: true,
                not_found: false,
                conflict: true,
                openapi_flags: &[],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::Find,
                label: "find",
                method: "GET",
                route_path: "/contract/find/:name",
                openapi_path: "/contract/find/{name}",
                http_path: "/contract/find/Ana",
                body: None,
                expected_status: 200,
                expected_body: r#"{"name":"Ana","status":"active","tenant":"north","balance":120}"#,
                success_status: "200",
                response_component: "Customer",
                request_body: false,
                bad_request: false,
                not_found: true,
                conflict: false,
                openapi_flags: &[],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::Where,
                label: "where",
                method: "GET",
                route_path: "/contract/where",
                openapi_path: "/contract/where",
                http_path: "/contract/where?status=active",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Ana","status":"active","tenant":"north","balance":120},{"name":"Bia","status":"active","tenant":"south","balance":80}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WherePage,
                label: "where_page",
                method: "GET",
                route_path: "/contract/where-page",
                openapi_path: "/contract/where-page",
                http_path: "/contract/where-page?status=active&limit=1&offset=1",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":2,"items":[{"name":"Bia","status":"active","tenant":"south","balance":80}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereNot,
                label: "where_not",
                method: "GET",
                route_path: "/contract/where-not",
                openapi_path: "/contract/where-not",
                http_path: "/contract/where-not?status=active",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Cris","status":"blocked","tenant":"north","balance":220},{"name":"Dina","status":"prospect","tenant":"west","balance":40}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &["x-nexus-exclusion-filters"],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereNotPage,
                label: "where_not_page",
                method: "GET",
                route_path: "/contract/where-not-page",
                openapi_path: "/contract/where-not-page",
                http_path: "/contract/where-not-page?status=active&limit=1&offset=1",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":2,"items":[{"name":"Dina","status":"prospect","tenant":"west","balance":40}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                    "x-nexus-exclusion-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereNotIn,
                label: "where_not_in",
                method: "GET",
                route_path: "/contract/where-not-in",
                openapi_path: "/contract/where-not-in",
                http_path: "/contract/where-not-in?statuses=active,blocked",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Dina","status":"prospect","tenant":"west","balance":40}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &["x-nexus-exclusion-filters", "x-nexus-in-filters"],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereNotInPage,
                label: "where_not_in_page",
                method: "GET",
                route_path: "/contract/where-not-in-page",
                openapi_path: "/contract/where-not-in-page",
                http_path: "/contract/where-not-in-page?statuses=active,blocked&limit=1&offset=0",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":1,"items":[{"name":"Dina","status":"prospect","tenant":"west","balance":40}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                    "x-nexus-exclusion-filters",
                    "x-nexus-in-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereNotInOptional,
                label: "where_not_in_optional",
                method: "GET",
                route_path: "/contract/where-not-in-optional",
                openapi_path: "/contract/where-not-in-optional",
                http_path: "/contract/where-not-in-optional?statuses=active,blocked",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Dina","status":"prospect","tenant":"west","balance":40}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-exclusion-filters",
                    "x-nexus-optional-filters",
                    "x-nexus-in-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereNotInOptionalPage,
                label: "where_not_in_optional_page",
                method: "GET",
                route_path: "/contract/where-not-in-optional-page",
                openapi_path: "/contract/where-not-in-optional-page",
                http_path:
                    "/contract/where-not-in-optional-page?statuses=active,blocked&limit=1&offset=0",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":1,"items":[{"name":"Dina","status":"prospect","tenant":"west","balance":40}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                    "x-nexus-exclusion-filters",
                    "x-nexus-optional-filters",
                    "x-nexus-in-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereOptional,
                label: "where_optional",
                method: "GET",
                route_path: "/contract/where-optional",
                openapi_path: "/contract/where-optional",
                http_path: "/contract/where-optional?status=active",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Ana","status":"active","tenant":"north","balance":120},{"name":"Bia","status":"active","tenant":"south","balance":80}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &["x-nexus-optional-filters"],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereOptionalPage,
                label: "where_optional_page",
                method: "GET",
                route_path: "/contract/where-optional-page",
                openapi_path: "/contract/where-optional-page",
                http_path: "/contract/where-optional-page?status=active&limit=1&offset=0",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":2,"items":[{"name":"Ana","status":"active","tenant":"north","balance":120}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                    "x-nexus-optional-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereIn,
                label: "where_in",
                method: "GET",
                route_path: "/contract/where-in",
                openapi_path: "/contract/where-in",
                http_path: "/contract/where-in?statuses=active,blocked",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Ana","status":"active","tenant":"north","balance":120},{"name":"Bia","status":"active","tenant":"south","balance":80},{"name":"Cris","status":"blocked","tenant":"north","balance":220}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &["x-nexus-in-filters"],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereInPage,
                label: "where_in_page",
                method: "GET",
                route_path: "/contract/where-in-page",
                openapi_path: "/contract/where-in-page",
                http_path: "/contract/where-in-page?statuses=active,blocked&limit=2&offset=1",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":3,"items":[{"name":"Bia","status":"active","tenant":"south","balance":80},{"name":"Cris","status":"blocked","tenant":"north","balance":220}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                    "x-nexus-in-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereInOptional,
                label: "where_in_optional",
                method: "GET",
                route_path: "/contract/where-in-optional",
                openapi_path: "/contract/where-in-optional",
                http_path: "/contract/where-in-optional?statuses=active,blocked",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Ana","status":"active","tenant":"north","balance":120},{"name":"Bia","status":"active","tenant":"south","balance":80},{"name":"Cris","status":"blocked","tenant":"north","balance":220}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &["x-nexus-optional-filters", "x-nexus-in-filters"],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereInOptionalPage,
                label: "where_in_optional_page",
                method: "GET",
                route_path: "/contract/where-in-optional-page",
                openapi_path: "/contract/where-in-optional-page",
                http_path:
                    "/contract/where-in-optional-page?statuses=active,blocked&limit=2&offset=1",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":3,"items":[{"name":"Bia","status":"active","tenant":"south","balance":80},{"name":"Cris","status":"blocked","tenant":"north","balance":220}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                    "x-nexus-optional-filters",
                    "x-nexus-in-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereCompare,
                label: "where_compare",
                method: "GET",
                route_path: "/contract/where-compare",
                openapi_path: "/contract/where-compare",
                http_path: "/contract/where-compare?min=100",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Ana","status":"active","tenant":"north","balance":120},{"name":"Cris","status":"blocked","tenant":"north","balance":220}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &["x-nexus-comparison-filters"],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereComparePage,
                label: "where_compare_page",
                method: "GET",
                route_path: "/contract/where-compare-page",
                openapi_path: "/contract/where-compare-page",
                http_path: "/contract/where-compare-page?min=50&limit=1&offset=1",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":3,"items":[{"name":"Bia","status":"active","tenant":"south","balance":80}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                    "x-nexus-comparison-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereText,
                label: "where_text",
                method: "GET",
                route_path: "/contract/where-text",
                openapi_path: "/contract/where-text",
                http_path: "/contract/where-text?term=i",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Bia","status":"active","tenant":"south","balance":80},{"name":"Cris","status":"blocked","tenant":"north","balance":220},{"name":"Dina","status":"prospect","tenant":"west","balance":40}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &["x-nexus-text-filters"],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereTextPage,
                label: "where_text_page",
                method: "GET",
                route_path: "/contract/where-text-page",
                openapi_path: "/contract/where-text-page",
                http_path: "/contract/where-text-page?term=i&limit=1&offset=1",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":3,"items":[{"name":"Cris","status":"blocked","tenant":"north","balance":220}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                    "x-nexus-text-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereBetween,
                label: "where_between",
                method: "GET",
                route_path: "/contract/where-between",
                openapi_path: "/contract/where-between",
                http_path: "/contract/where-between?min=50&max=150",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Ana","status":"active","tenant":"north","balance":120},{"name":"Bia","status":"active","tenant":"south","balance":80}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &["x-nexus-range-filters"],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereBetweenPage,
                label: "where_between_page",
                method: "GET",
                route_path: "/contract/where-between-page",
                openapi_path: "/contract/where-between-page",
                http_path: "/contract/where-between-page?min=50&max=150&limit=1&offset=1",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":2,"items":[{"name":"Bia","status":"active","tenant":"south","balance":80}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                    "x-nexus-range-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereAll,
                label: "where_all",
                method: "GET",
                route_path: "/contract/where-all",
                openapi_path: "/contract/where-all",
                http_path: "/contract/where-all?status=active&tenant=north",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Ana","status":"active","tenant":"north","balance":120}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &["x-nexus-composite-filters"],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereAllPage,
                label: "where_all_page",
                method: "GET",
                route_path: "/contract/where-all-page",
                openapi_path: "/contract/where-all-page",
                http_path: "/contract/where-all-page?status=active&tenant=south&limit=1&offset=0",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":1,"items":[{"name":"Bia","status":"active","tenant":"south","balance":80}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                    "x-nexus-composite-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereAny,
                label: "where_any",
                method: "GET",
                route_path: "/contract/where-any",
                openapi_path: "/contract/where-any",
                http_path: "/contract/where-any?status=blocked&tenant=south",
                body: None,
                expected_status: 200,
                expected_body: r#"[{"name":"Bia","status":"active","tenant":"south","balance":80},{"name":"Cris","status":"blocked","tenant":"north","balance":220}]"#,
                success_status: "200",
                response_component: "NexusList_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &["x-nexus-or-filters"],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::WhereAnyPage,
                label: "where_any_page",
                method: "GET",
                route_path: "/contract/where-any-page",
                openapi_path: "/contract/where-any-page",
                http_path: "/contract/where-any-page?status=active&tenant=north&limit=1&offset=1",
                body: None,
                expected_status: 200,
                expected_body: r#"{"total":3,"items":[{"name":"Bia","status":"active","tenant":"south","balance":80}]}"#,
                success_status: "200",
                response_component: "NexusPage_Customer",
                request_body: false,
                bad_request: true,
                not_found: false,
                conflict: false,
                openapi_flags: &[
                    "x-nexus-pagination",
                    "x-nexus-total-count",
                    "x-nexus-ordering",
                    "x-nexus-or-filters",
                ],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::Update,
                label: "update",
                method: "PUT",
                route_path: "/contract/update/:name",
                openapi_path: "/contract/update/{name}",
                http_path: "/contract/update/Bia",
                body: Some(r#"{"name":"Bia","status":"active","tenant":"south","balance":95}"#),
                expected_status: 200,
                expected_body: r#"{"name":"Bia","status":"active","tenant":"south","balance":95}"#,
                success_status: "200",
                response_component: "Customer",
                request_body: true,
                bad_request: true,
                not_found: true,
                conflict: true,
                openapi_flags: &[],
            },
            ModelOperationContractCase {
                operation: ModelStaticOperation::Delete,
                label: "delete",
                method: "DELETE",
                route_path: "/contract/delete/:name",
                openapi_path: "/contract/delete/{name}",
                http_path: "/contract/delete/Ana",
                body: None,
                expected_status: 200,
                expected_body: r#"{"name":"Ana","status":"active","tenant":"north","balance":120}"#,
                success_status: "200",
                response_component: "Customer",
                request_body: false,
                bad_request: false,
                not_found: true,
                conflict: false,
                openapi_flags: &[],
            },
        ]
    }

    fn auth_operation_contract_cases() -> [AuthOperationContractCase; 4] {
        [
            AuthOperationContractCase {
                operation: AuthStaticOperation::Register,
                label: "register",
                method: "POST",
                route_path: "/contract/auth/register",
                openapi_path: "/contract/auth/register",
                http_path: "/contract/auth/register",
                success_status: "201",
                auth_config: Some("ContractAuth"),
                request_body: true,
                requires_auth: false,
                csrf_header: false,
                bad_request: true,
                rate_limit: true,
                forbidden: false,
                expected_http_status: 201,
                expected_body_fragment: r#""email":"matrix-register@example.com""#,
            },
            AuthOperationContractCase {
                operation: AuthStaticOperation::Login,
                label: "login",
                method: "POST",
                route_path: "/contract/auth/login",
                openapi_path: "/contract/auth/login",
                http_path: "/contract/auth/login",
                success_status: "200",
                auth_config: Some("ContractAuth"),
                request_body: true,
                requires_auth: false,
                csrf_header: false,
                bad_request: true,
                rate_limit: true,
                forbidden: false,
                expected_http_status: 200,
                expected_body_fragment: r#""email":"matrix-login@example.com""#,
            },
            AuthOperationContractCase {
                operation: AuthStaticOperation::Logout,
                label: "logout",
                method: "POST",
                route_path: "/contract/auth/logout",
                openapi_path: "/contract/auth/logout",
                http_path: "/contract/auth/logout",
                success_status: "200",
                auth_config: None,
                request_body: false,
                requires_auth: true,
                csrf_header: true,
                bad_request: false,
                rate_limit: false,
                forbidden: true,
                expected_http_status: 200,
                expected_body_fragment: "true",
            },
            AuthOperationContractCase {
                operation: AuthStaticOperation::User,
                label: "user",
                method: "GET",
                route_path: "/contract/auth/user",
                openapi_path: "/contract/auth/user",
                http_path: "/contract/auth/user",
                success_status: "200",
                auth_config: None,
                request_body: false,
                requires_auth: true,
                csrf_header: false,
                bad_request: false,
                rate_limit: false,
                forbidden: false,
                expected_http_status: 200,
                expected_body_fragment: r#""email":"matrix-user@example.com""#,
            },
        ]
    }

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
        route: &CheckedRouteView<'_>,
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
        program: &Program,
        operation: &JsonValue,
        route: &CheckedRouteView<'_>,
        context: &str,
    ) {
        let Some(component_name) = route_request_body_component_name(program, route) else {
            assert_json_field_absent(operation, "requestBody", context);
            return;
        };

        let request_body = expect_json_field(operation, "requestBody");
        let request_body_context = format!("{}.requestBody", context);
        let actual_ref = expect_ref_value(request_body, &request_body_context);
        let expected_ref = format!("#/components/requestBodies/{}", component_name);
        assert_eq!(
            actual_ref, expected_ref,
            "{} deveria apontar para requestBody real da route",
            request_body_context
        );

        let component = component_ref_target(document, actual_ref);
        assert!(
            expect_bool_field_value(component, "required"),
            "{} deveria ser required",
            request_body_context
        );
        let schema = openapi_json_content_schema(component, &request_body_context);
        if let Some(model) = route_request_body_model(route) {
            let expected_schema = format!(
                r##"{{"$ref":"#/components/schemas/{}"}}"##,
                escape_json(&model)
            );
            assert_json_matches_source(schema, &expected_schema, &request_body_context);
        } else {
            json_object_fields(schema, &request_body_context);
        }
    }

    fn expected_route_response_status(
        program: &Program,
        route: &CheckedRouteView<'_>,
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
        route: &CheckedRouteView<'_>,
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

        for route in checked_routes(program) {
            route_count += 1;
            let operation = openapi_operation_for_route(document, &route);
            let context = format!("operation {} {}", method_name(route.method), route.path);

            assert_operation_request_body_matches_route(
                document, program, operation, &route, &context,
            );
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

    fn assert_model_operation_matrix_covers_all_operations(cases: &[ModelOperationContractCase]) {
        assert_eq!(
            cases.len(),
            ModelStaticOperation::ALL.len(),
            "matriz de contrato deve ter um caso por ModelStaticOperation"
        );

        let mut covered = Vec::new();
        for case in cases {
            assert!(
                !covered.contains(&case.operation),
                "operacao {:?} duplicada na matriz",
                case.operation
            );
            covered.push(case.operation);
        }

        for operation in ModelStaticOperation::ALL {
            assert!(
                covered.contains(&operation),
                "operacao {:?} ausente da matriz de contrato",
                operation
            );
        }
    }

    fn assert_auth_operation_matrix_covers_all_operations(cases: &[AuthOperationContractCase]) {
        assert_eq!(
            cases.len(),
            AuthStaticOperation::ALL.len(),
            "matriz de contrato deve ter um caso por AuthStaticOperation"
        );

        let mut covered = Vec::new();
        for case in cases {
            assert!(
                !covered.contains(&case.operation),
                "operacao {:?} duplicada na matriz de Auth",
                case.operation
            );
            covered.push(case.operation);
        }

        for operation in AuthStaticOperation::ALL {
            assert!(
                covered.contains(&operation),
                "operacao {:?} ausente da matriz de contrato de Auth",
                operation
            );
        }
    }

    fn model_operation_contract_temp_dir(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "nexuslang_model_contract_matrix_{}_{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn model_operation_matrix_storage(label: &str) -> Storage {
        let data_dir = model_operation_contract_temp_dir(label);
        fs::create_dir_all(&data_dir).unwrap();
        fs::write(
            data_dir.join("customer.json"),
            MODEL_OPERATION_MATRIX_RECORDS,
        )
        .unwrap();
        Storage::new_json(&data_dir)
    }

    fn auth_operation_contract_temp_dir(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "nexuslang_auth_contract_matrix_{}_{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn auth_operation_matrix_storage(label: &str) -> Storage {
        let data_dir = auth_operation_contract_temp_dir(label);
        fs::create_dir_all(&data_dir).unwrap();
        Storage::new_json(&data_dir)
    }

    fn assert_model_operation_route_matches_case(
        program: &Program,
        case: &ModelOperationContractCase,
    ) {
        let route = checked_routes(program)
            .into_iter()
            .find(|route| method_name(route.method) == case.method && route.path == case.route_path)
            .unwrap_or_else(|| panic!("route {} {} ausente", case.method, case.route_path));

        match route.return_expr {
            Some(CheckedRouteExpr::ModelOperation(operation)) => {
                assert_eq!(
                    operation.model, "Customer",
                    "model inesperado em {}",
                    case.label
                );
                assert_eq!(
                    operation.operation, case.operation,
                    "operacao de route inesperada em {}",
                    case.label
                );
            }
            Some(other) => panic!(
                "route {} deveria retornar operacao de model, encontrado {:?}",
                case.label, other
            ),
            None => panic!("route {} deveria ter return", case.label),
        }
    }

    fn assert_auth_operation_route_matches_case(
        program: &Program,
        case: &AuthOperationContractCase,
    ) {
        let route = checked_routes(program)
            .into_iter()
            .find(|route| method_name(route.method) == case.method && route.path == case.route_path)
            .unwrap_or_else(|| panic!("route {} {} ausente", case.method, case.route_path));

        match route.return_expr {
            Some(CheckedRouteExpr::AuthOperation(operation)) => {
                assert_eq!(
                    operation.operation, case.operation,
                    "operacao de Auth inesperada em {}",
                    case.label
                );
                let checked_args = operation
                    .checked_args
                    .unwrap_or_else(|| panic!("Auth args nao normalizados em {}", case.label));
                assert_eq!(
                    checked_args.auth_config_name(),
                    case.auth_config,
                    "auth config normalizado inesperado em {}",
                    case.label
                );
            }
            Some(other) => panic!(
                "route {} deveria retornar operacao de Auth, encontrado {:?}",
                case.label, other
            ),
            None => panic!("route {} deveria ter return", case.label),
        }
    }

    fn openapi_document_from_http(program: &Program) -> JsonValue {
        let storage = model_operation_matrix_storage("openapi");
        let response = super::router::handle_request(program, &storage, "GET", "/openapi.json", "");

        assert_eq!(response.status, 200);
        parse_openapi_document(&response.body)
    }

    fn auth_openapi_document_from_http(program: &Program) -> JsonValue {
        let storage = auth_operation_matrix_storage("openapi");
        let response = super::router::handle_request(program, &storage, "GET", "/openapi.json", "");

        assert_eq!(response.status, 200);
        parse_openapi_document(&response.body)
    }

    fn openapi_operation_for_case<'a>(
        document: &'a JsonValue,
        case: &ModelOperationContractCase,
    ) -> &'a JsonValue {
        let paths = expect_object_field(document, "paths");
        let path_item = json_object_field(paths, case.openapi_path)
            .unwrap_or_else(|| panic!("OpenAPI path '{}' ausente", case.openapi_path));
        let method = case.method.to_lowercase();
        json_object_field(path_item, &method).unwrap_or_else(|| {
            panic!(
                "OpenAPI operation '{} {}' ausente",
                method, case.openapi_path
            )
        })
    }

    fn openapi_operation_for_auth_case<'a>(
        document: &'a JsonValue,
        case: &AuthOperationContractCase,
    ) -> &'a JsonValue {
        let paths = expect_object_field(document, "paths");
        let path_item = json_object_field(paths, case.openapi_path)
            .unwrap_or_else(|| panic!("OpenAPI path '{}' ausente", case.openapi_path));
        let method = case.method.to_lowercase();
        json_object_field(path_item, &method).unwrap_or_else(|| {
            panic!(
                "OpenAPI operation '{} {}' ausente",
                method, case.openapi_path
            )
        })
    }

    fn assert_model_operation_openapi_case(
        document: &JsonValue,
        case: &ModelOperationContractCase,
    ) {
        let operation = openapi_operation_for_case(document, case);
        let context = format!("{} {}", case.method, case.route_path);

        let request_body = json_object_field(operation, "requestBody");
        assert_eq!(
            request_body.is_some(),
            case.request_body,
            "{} requestBody inesperado",
            context
        );
        if let Some(request_body) = request_body {
            assert_eq!(
                expect_ref_value(request_body, &format!("{}.requestBody", context)),
                "#/components/requestBodies/NexusRequestBody_Customer"
            );
        }

        for flag in MODEL_OPERATION_OPENAPI_FLAGS {
            let actual = json_object_field(operation, flag).is_some();
            let expected = case.openapi_flags.contains(&flag);
            assert_eq!(
                actual, expected,
                "{} flag OpenAPI '{}' inesperada",
                context, flag
            );
            if actual {
                assert!(expect_bool_field_value(operation, flag));
            }
        }

        let responses = expect_object_field(operation, "responses");
        let success = expect_json_field(responses, case.success_status);
        let expected_success_ref = format!(
            "#/components/responses/NexusResponse{}_{}",
            case.success_status, case.response_component
        );
        assert_eq!(
            expect_ref_value(success, &format!("{} response", context)),
            expected_success_ref
        );

        for (status, expected) in [
            ("400", case.bad_request),
            ("404", case.not_found),
            ("409", case.conflict),
        ] {
            assert_eq!(
                json_object_field(responses, status).is_some(),
                expected,
                "{} response {} inesperado",
                context,
                status
            );
        }
    }

    fn assert_auth_operation_openapi_case(document: &JsonValue, case: &AuthOperationContractCase) {
        let operation = openapi_operation_for_auth_case(document, case);
        let context = format!("{} {}", case.method, case.route_path);

        let request_body = json_object_field(operation, "requestBody");
        assert_eq!(
            request_body.is_some(),
            case.request_body,
            "{} requestBody inesperado",
            context
        );
        if let Some(request_body) = request_body {
            let expected_ref = format!(
                "#/components/requestBodies/{}",
                openapi_auth_request_body_component_name(case.operation, "ContractAuth")
            );
            assert_eq!(
                expect_ref_value(request_body, &format!("{}.requestBody", context)),
                expected_ref
            );
            assert_auth_request_body_component_schema(document, request_body, case, &context);
        }
        assert_eq!(
            json_object_field(operation, "security").is_some(),
            case.requires_auth,
            "{} security inesperado",
            context
        );
        assert_eq!(
            operation_has_csrf_header_parameter(operation),
            case.csrf_header,
            "{} parametro CSRF inesperado",
            context
        );

        let responses = expect_object_field(operation, "responses");
        expect_json_field(responses, case.success_status);
        for (status, expected) in [
            ("400", case.bad_request),
            ("401", case.requires_auth),
            ("403", case.forbidden),
            ("429", case.rate_limit),
        ] {
            assert_eq!(
                json_object_field(responses, status).is_some(),
                expected,
                "{} response {} inesperado",
                context,
                status
            );
        }
    }

    fn operation_has_csrf_header_parameter(operation: &JsonValue) -> bool {
        expect_array_field(operation, "parameters")
            .iter()
            .any(|param| {
                matches!(
                    json_object_field(param, "name"),
                    Some(JsonValue::String(name)) if name == "X-Nexus-CSRF-Token"
                )
            })
    }

    fn assert_auth_request_body_component_schema(
        document: &JsonValue,
        request_body: &JsonValue,
        case: &AuthOperationContractCase,
        context: &str,
    ) {
        let request_body_ref = expect_ref_value(request_body, &format!("{}.requestBody", context));
        let component = component_ref_target(document, request_body_ref);
        assert!(expect_bool_field_value(component, "required"));
        let schema = openapi_json_content_schema(component, &format!("{}.requestBody", context));
        expect_string_field(schema, "type", "object");
        let properties = expect_object_field(schema, "properties");
        expect_object_field(properties, "password");

        match case.operation {
            AuthStaticOperation::Register => {
                expect_object_field(properties, "email");
                expect_object_field(properties, "name");
                expect_object_field(properties, "role");
                let password = expect_object_field(properties, "password");
                assert_eq!(expect_number_field_value(password, "minLength"), 15.0);
                assert_string_array_field(schema, "required", &["email", "name", "password"]);
            }
            AuthStaticOperation::Login => {
                expect_object_field(properties, "email");
                assert_json_field_absent(properties, "name", context);
                assert_json_field_absent(properties, "role", context);
                assert_string_array_field(schema, "required", &["email", "password"]);
            }
            AuthStaticOperation::Logout | AuthStaticOperation::User => {}
        }
    }

    fn assert_model_operation_http_case(program: &Program, case: &ModelOperationContractCase) {
        let storage = model_operation_matrix_storage(case.label);
        let response = super::router::handle_request(
            program,
            &storage,
            case.method,
            case.http_path,
            case.body.unwrap_or(""),
        );

        assert_eq!(
            response.status, case.expected_status,
            "status HTTP inesperado em {}",
            case.label
        );
        assert_eq!(
            response.body, case.expected_body,
            "body HTTP inesperado em {}",
            case.label
        );
    }

    fn assert_auth_operation_http_case(program: &Program, case: &AuthOperationContractCase) {
        let storage = auth_operation_matrix_storage(case.label);
        let response = match case.operation {
            AuthStaticOperation::Register => super::router::handle_request(
                program,
                &storage,
                case.method,
                case.http_path,
                r#"{"email":"matrix-register@example.com","name":"Matrix Register","role":"admin","password":"strong-password-123"}"#,
            ),
            AuthStaticOperation::Login => {
                register_auth_matrix_user(
                    program,
                    &storage,
                    "matrix-login@example.com",
                    "Matrix Login",
                    "user",
                );
                super::router::handle_request(
                    program,
                    &storage,
                    case.method,
                    case.http_path,
                    r#"{"email":"matrix-login@example.com","password":"strong-password-123"}"#,
                )
            }
            AuthStaticOperation::Logout => {
                let token = register_auth_matrix_user(
                    program,
                    &storage,
                    "matrix-logout@example.com",
                    "Matrix Logout",
                    "admin",
                );
                let headers = bearer_headers(&token);
                let response = super::router::handle_request_with_headers(
                    program,
                    &storage,
                    case.method,
                    case.http_path,
                    &headers,
                    "",
                );
                let after_logout = super::router::handle_request_with_headers(
                    program,
                    &storage,
                    "GET",
                    "/contract/auth/user",
                    &headers,
                    "",
                );
                assert_eq!(after_logout.status, 401, "logout deveria revogar token");
                response
            }
            AuthStaticOperation::User => {
                let token = register_auth_matrix_user(
                    program,
                    &storage,
                    "matrix-user@example.com",
                    "Matrix User",
                    "admin",
                );
                let headers = bearer_headers(&token);
                super::router::handle_request_with_headers(
                    program,
                    &storage,
                    case.method,
                    case.http_path,
                    &headers,
                    "",
                )
            }
        };

        assert_eq!(
            response.status, case.expected_http_status,
            "status HTTP inesperado em {}",
            case.label
        );
        assert!(
            response.body.contains(case.expected_body_fragment),
            "body HTTP inesperado em {}: {}",
            case.label,
            response.body
        );
    }

    fn register_auth_matrix_user(
        program: &Program,
        storage: &Storage,
        email: &str,
        name: &str,
        role: &str,
    ) -> String {
        let body = format!(
            r#"{{"email":"{}","name":"{}","role":"{}","password":"strong-password-123"}}"#,
            email, name, role
        );
        let response = super::router::handle_request(
            program,
            storage,
            "POST",
            "/contract/auth/register",
            &body,
        );
        assert_eq!(response.status, 201, "precondition register failed");
        json_response_string_field(&response.body, "token")
    }

    fn bearer_headers(token: &str) -> Vec<(String, String)> {
        vec![("Authorization".to_string(), format!("Bearer {}", token))]
    }

    fn json_response_string_field(body: &str, field: &str) -> String {
        let value = parse_json(body)
            .unwrap_or_else(|err| panic!("response body nao e JSON parseavel: {}\n{}", err, body));
        expect_string_field_value(&value, field).to_string()
    }

    #[test]
    fn model_operation_contract_matrix_validates_checker_openapi_and_http() {
        let cases = model_operation_contract_cases();
        assert_model_operation_matrix_covers_all_operations(&cases);

        let program = parse_checked_source(MODEL_OPERATION_MATRIX_SOURCE).unwrap_or_else(|err| {
            panic!(
                "checker rejeitou a matriz de contrato de operacoes de model: {}",
                err
            )
        });
        let document = openapi_document_from_http(&program);
        assert_openapi_operations_match_route_contracts_and_components(&document, &program);

        for case in &cases {
            assert_model_operation_route_matches_case(&program, case);
            assert_model_operation_openapi_case(&document, case);
            assert_model_operation_http_case(&program, case);
        }
    }

    #[test]
    fn auth_operation_contract_matrix_validates_checker_hir_openapi_and_http() {
        let cases = auth_operation_contract_cases();
        assert_auth_operation_matrix_covers_all_operations(&cases);

        let program = parse_checked_source(AUTH_OPERATION_MATRIX_SOURCE).unwrap_or_else(|err| {
            panic!(
                "checker rejeitou a matriz de contrato de operacoes de Auth: {}",
                err
            )
        });
        let document = auth_openapi_document_from_http(&program);
        assert_openapi_operations_match_route_contracts_and_components(&document, &program);

        for case in &cases {
            assert_auth_operation_route_matches_case(&program, case);
            assert_auth_operation_openapi_case(&document, case);
            assert_auth_operation_http_case(&program, case);
        }
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
