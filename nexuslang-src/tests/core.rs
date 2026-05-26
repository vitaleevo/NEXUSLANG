use nexuslang::diagnostic::DiagnosticStage;
use nexuslang::{check_source, parse_source_diagnostic, run_source};
use std::fs;

#[test]
fn top_level_bindings_survive_between_statements() {
    let source = r#"
let salario_base = 300000 kz
let bonus = salario_base * 0.1
let total = salario_base + bonus
print(total)
"#;

    assert!(run_source(source).is_ok());
}

#[test]
fn const_reassignment_is_rejected() {
    let source = r#"
const x = 1
x = 2
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Constante 'x'"));
}

#[test]
fn type_annotations_are_checked() {
    let source = r#"
let salario: money = "alto"
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Tipo inválido"));
}

#[test]
fn function_argument_count_is_checked() {
    let source = r#"
fn dobrar(x: int) -> int {
    return x * 2
}

let valor = dobrar(1, 2)
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("espera 1 argumento"));
}

#[test]
fn function_return_type_is_checked() {
    let source = r#"
fn total() -> money {
    return "cem"
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Tipo de retorno inválido"));
}

#[test]
fn typed_function_must_return_on_all_paths() {
    let source = r#"
fn total(aprovado: bool) -> money {
    if aprovado {
        return 1000 kz
    }
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("deve retornar money em todos os caminhos"));
}

#[test]
fn void_function_cannot_return_value() {
    let source = r#"
fn registrar() {
    return "ok"
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("nao pode retornar valor"));
}

#[test]
fn arrays_must_be_homogeneous() {
    let source = r#"
let valores = [1, "dois"]
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Array com tipos incompatíveis"));
}

#[test]
fn unknown_static_model_is_rejected() {
    let source = r#"
let empregados = Employee::all()
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Model 'Employee' não encontrado"));
}

#[test]
fn erp_example_checks_and_runs() {
    let source = include_str!("../examples/erp_basico.nx");

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn functions_can_read_top_level_bindings() {
    let source = r#"
fn dobro_global() -> int {
    return taxa * 2
}

let taxa = 2
print(dobro_global())
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn explicit_array_type_annotations_are_checked() {
    let source = r#"
let nomes: [string] = ["Ana", "Beto"]
let total: int = len(nomes)
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn explicit_array_type_rejects_wrong_items() {
    let source = r#"
let nomes: [string] = ["Ana", 42]
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Array com tipos incompatíveis") || err.contains("Tipo inválido"));
}

#[test]
fn function_can_accept_array_parameter() {
    let source = r#"
fn contar(nomes: [string]) -> int {
    return len(nomes)
}

let nomes: [string] = ["TI", "RH"]
let total = contar(nomes)
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn model_array_type_matches_static_all() {
    let source = r#"
model Employee {
    name: string
}

let empregados: [Employee] = Employee::all()
"#;

    assert!(check_source(source).is_ok());
}

#[test]
fn model_array_type_rejects_wrong_model() {
    let source = r#"
model Employee {
    name: string
}

model Department {
    name: string
}

let departamentos: [Department] = Employee::all()
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Tipo inválido"));
}

#[test]
fn unknown_model_type_is_rejected() {
    let source = r#"
let empregados: [Employee] = []
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Model type 'Employee' não encontrado"));
}

#[test]
fn reserved_openapi_component_model_names_are_rejected() {
    for name in ["NexusError", "NexusPage_Customer", "NexusList_Customer"] {
        let source = format!(
            r#"
model {} {{
    name: string
}}
"#,
            name
        );

        let err = check_source(&source).unwrap_err();
        assert!(err.contains("nome reservado"));
        assert!(err.contains(name));
        assert!(err.contains("NexusError"));
        assert!(err.contains("NexusPage_*"));
        assert!(err.contains("NexusList_*"));
    }
}

#[test]
fn model_instance_checks_and_runs() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

let customer: Customer = Customer { name: "Ana", balance: 1000 kz }
print(customer)
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn model_instance_rejects_unknown_field() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

let customer = Customer { name: "Ana", email: "ana@example.com", balance: 1000 kz }
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Campo 'Customer.email' nao existe"));
}

#[test]
fn model_instance_rejects_missing_required_field() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

let customer = Customer { name: "Ana" }
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Campo 'Customer.balance' obrigatorio ausente"));
}

#[test]
fn model_instance_rejects_wrong_field_type() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

let customer = Customer { name: "Ana", balance: "alto" }
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Campo 'Customer.balance': esperado money, encontrado string"));
}

#[test]
fn model_instance_fields_require_commas() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

let customer = Customer { name: "Ana" balance: 1000 kz }
"#;

    let diagnostic = parse_source_diagnostic(source).unwrap_err();
    assert_eq!(diagnostic.stage, DiagnosticStage::Parser);
    assert!(diagnostic.message.contains("esperado ',' ou '}'"));
}

#[test]
fn model_field_access_checks_and_runs() {
    let source = r#"
model Customer {
    name: string
    balance: money
    active: bool
}

fn customer_name(customer: Customer) -> string {
    return customer.name
}

let customer: Customer = Customer { name: "Ana", balance: 1000 kz, active: true }
let name: string = customer.name
let balance: money = customer.balance
let active: bool = customer.active
print(customer_name(customer))
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn model_field_access_rejects_unknown_field() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

let customer = Customer { name: "Ana", balance: 1000 kz }
let email = customer.email
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Campo 'Customer.email' nao existe"));
}

#[test]
fn model_field_access_rejects_non_model_value() {
    let source = r#"
let name = "Ana"
let value = name.email
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Acesso a campo 'email' espera model instance, encontrado string"));
}

#[test]
fn dot_field_access_does_not_break_float_literals() {
    let source = r#"
let rate: float = 1.5
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn optional_type_accepts_value_and_nil() {
    let source = r#"
let email: string? = nil
let phone: string? = "999"
let score: float? = 1
print(email)
print(phone)
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn nil_is_rejected_for_non_optional_type() {
    let source = r#"
let email: string = nil
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("esperado string, encontrado nil"));
}

#[test]
fn optional_model_fields_can_be_omitted_and_read_as_nil() {
    let source = r#"
model Customer {
    name: string
    email: string?
    balance: money?
}

let customer: Customer = Customer { name: "Ana" }
let email: string? = customer.email
let balance: money? = customer.balance
print(customer.email)
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn optional_model_field_accepts_value_and_nil() {
    let source = r#"
model Customer {
    name: string
    email: string?
    balance: money?
}

let with_email: Customer = Customer { name: "Ana", email: "ana@example.com", balance: nil }
let without_email: Customer = Customer { name: "Beto", email: nil }
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn optional_model_field_rejects_wrong_value_type() {
    let source = r#"
model Customer {
    name: string
    email: string?
}

let customer = Customer { name: "Ana", email: 42 }
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Campo 'Customer.email': esperado string, encontrado int"));
}

#[test]
fn optional_values_are_rejected_in_arithmetic_and_concat() {
    let source = r#"
let email: string? = nil
let label = email + "!"
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("operacao com opcional invalida"));
}

#[test]
fn model_field_defaults_fill_missing_required_fields_and_run() {
    let source = r#"
model Customer {
    name: string
    status: string = "active"
    active: bool = true
    balance: money = 0 kz
    tags: [string] = ["new"]
    email: string? = nil
}

let customer: Customer = Customer { name: "Ana" }
let status: string = customer.status
let active: bool = customer.active
let balance: money = customer.balance
let tags: [string] = customer.tags
let email: string? = customer.email
print(customer.status)
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn model_field_defaults_can_be_overridden() {
    let source = r#"
model Customer {
    name: string
    status: string = "active"
}

let customer: Customer = Customer { name: "Ana", status: "blocked" }
let status: string = customer.status
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn model_field_default_must_match_field_type() {
    let source = r#"
model Customer {
    status: string = 1
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Campo 'Customer.status' default invalido"));
}

#[test]
fn model_field_default_rejects_non_static_expression() {
    let source = r#"
model Customer {
    status: string = str("active")
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("default de model field"));
}

#[test]
fn model_field_unique_rejects_non_scalar_type() {
    let source = r#"
model Customer {
    tags: [string] unique
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("unique so suporta"));
}

#[test]
fn model_field_index_accepts_scalars_and_rejects_invalid_constraints() {
    let source = r#"
model Customer {
    email: string index
    tier: int index
    balance: money index
    birthday: date? index
    status: string = "active" index
    nif: string unique index
}
"#;

    assert!(check_source(source).is_ok());

    let source = r#"
model Customer {
    tags: [string] index
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("index so suporta"));

    let source = r#"
model Customer {
    email: string index index
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("constraint 'index' duplicada"));
}

#[test]
fn model_field_min_max_accepts_supported_scalars_and_rejects_invalid_constraints() {
    let source = r#"
model Product {
    name: string min 2 max 80
    stock: int min 0 max 999
    rating: float min 0 max 5.0
    price: money min 100 kz max 5000 kz
    launch: date? min "2026-01-01" max "2026-12-31"
    status: string = "active" min 1 max 20
}
"#;

    assert!(check_source(source).is_ok());

    let source = r#"
model Product {
    active: bool min true
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("min/max so suporta"));

    let source = r#"
model Product {
    name: string min "A"
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("min em string espera int"));

    let source = r#"
model Product {
    stock: int min 10 max 5
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("min nao pode ser maior que max"));

    let source = r#"
model Product {
    price: money min 100 kz max 200 usd
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("min/max money devem usar a mesma moeda"));

    let source = r#"
model Product {
    stock: int min 0 min 1
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("constraint 'min' duplicada"));
}

#[test]
fn model_field_default_is_checked_against_min_max_constraints() {
    let source = r#"
model Product {
    name: string = "Valid" min 2 max 80
    stock: int = 5 min 0 max 10
    rating: float = 4 min 0 max 5.0
    price: money = 200 kz min 100 kz max 500 kz
    optional_name: string? = nil min 2 max 80
}
"#;

    assert!(check_source(source).is_ok());

    let source = r#"
model Product {
    name: string = "A" min 2 max 80
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("default viola min: tamanho deve ser >= 2"));

    let source = r#"
model Product {
    stock: int = 11 min 0 max 10
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("default viola max: valor deve ser <= 10"));

    let source = r#"
model Product {
    price: money = 50 kz min 100 kz max 500 kz
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("default viola min: valor deve ser >= 100 kz"));

    let source = r#"
model Product {
    price: money = 200 usd min 100 kz max 500 kz
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("default usa moeda usd, mas min usa kz"));
}

#[test]
fn len_accepts_strings() {
    let source = r#"
let nome = "Nexus"
let total: int = len(nome)
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn len_rejects_numbers() {
    let source = r#"
let total = len(42)
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("len() não aceita int"));
}

#[test]
fn structured_invoice_with_items_checks_and_runs() {
    let source = r#"
invoice {
    customer: "Empresa SARL"
    currency: "AOA"
    tax: 14
    discount: 10000 kz
    item "Consultoria" qty 2 price 150000 kz
    item "Suporte" qty 1 price 50000 kz
}
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn invoice_item_price_must_be_money() {
    let source = r#"
invoice {
    customer: "Empresa SARL"
    currency: "AOA"
    item "Consultoria" qty 2 price 150000
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Invoice item price"));
}

#[test]
fn invoice_requires_customer_currency_and_amount_source() {
    let source = r#"
invoice {
    customer: "Empresa SARL"
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Invoice deve declarar currency"));

    let source = r#"
invoice {
    customer: "Empresa SARL"
    currency: "AOA"
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Invoice deve declarar item ou total"));
}

#[test]
fn duplicate_invoice_fields_are_rejected() {
    let source = r#"
invoice {
    customer: "Empresa SARL"
    customer: "Outra"
    currency: "AOA"
    total: 1000 kz
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Invoice field 'customer' declarado mais de uma vez"));
}

#[test]
fn executable_workflow_steps_check_and_run() {
    let source = r#"
workflow Billing {
    step preparar {
        print("Preparar fatura")
    }
    step finalizar {
        print("Finalizar fatura")
    }
}

run_workflow("Billing")
"#;

    assert!(check_source(source).is_ok());
    assert!(run_source(source).is_ok());
}

#[test]
fn unknown_workflow_call_is_rejected() {
    let source = r#"
run_workflow("Missing")
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Workflow 'Missing' não encontrado"));
}

#[test]
fn route_params_are_available_in_route_body() {
    let source = r#"
route GET /employees/:id {
    return "employee " + id
}
"#;

    assert!(check_source(source).is_ok());
}

#[test]
fn route_paths_allow_hyphenated_static_segments() {
    let source = r#"
route GET /customers/search-not-in {
    return "ok"
}
"#;

    assert!(check_source(source).is_ok());

    let data_dir = temp_data_dir("hyphenated-route-path");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/search-not-in",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, r#""ok""#);
}

#[test]
fn static_route_paths_take_precedence_over_path_params() {
    let source = r#"
route GET /customers/:name {
    return "dynamic " + name
}

route GET /customers/search-not-in {
    return "static"
}
"#;

    assert!(check_source(source).is_ok());

    let data_dir = temp_data_dir("static-route-precedence");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/search-not-in",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, r#""static""#);
}

#[test]
fn http_decodes_percent_encoded_path_and_query_params() {
    let source = r#"
route GET /echo/:name ?(email: string, note: string) {
    return name + "|" + email + "|" + note
}

route GET /tags ?(tags: [string]) {
    return tags
}

route GET /reports/search-page {
    return "static"
}
"#;

    let data_dir = temp_data_dir("percent-decoding");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/echo/Ana%20Silva?em%61il=ana%40example.com&note=Ol%C3%A1+Mundo",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, r#""Ana Silva|ana@example.com|Olá Mundo""#);

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/echo/Ana%2FSilva?email=a%2Bb%40example.com&note=pre%C3%A7o%20100%25",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, r#""Ana/Silva|a+b@example.com|preço 100%""#);

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/tags?tags=ativo%20premium,lead%2Bvip",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, r#"["ativo premium","lead+vip"]"#);

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/reports/search%2Dpage",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, r#""static""#);
}

#[test]
fn http_rejects_invalid_percent_encoding_in_path_and_query() {
    let source = r#"
route GET /echo/:name ?(email: string) {
    return name + "|" + email
}
"#;

    let data_dir = temp_data_dir("percent-decoding-invalid");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/echo/Ana%ZZ?email=ana@example.com",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 400);
    assert!(response.body.contains("escape de path invalido"));

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/missing/Ana%ZZ/extra?email=ana@example.com",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 400);
    assert!(response.body.contains("escape de path invalido"));

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/echo/%C3%28?email=ana@example.com",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 400);
    assert!(response.body.contains("escape de path invalido"));

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/echo/Ana?email=ana%ZZexample.com",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 400);
    assert!(response.body.contains("escape de query invalido"));
}

#[test]
fn openapi_qa_example_checks() {
    let source = std::fs::read_to_string("examples/openapi_qa.nx").unwrap();

    assert!(check_source(&source).is_ok());
}

#[test]
fn duplicate_http_routes_by_method_and_path_are_rejected() {
    let source = r#"
route GET /customers/:name {
    return "first " + name
}

route GET /customers/:name {
    return "second " + name
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Route GET '/customers/:name' declarada mais de uma vez"));
}

#[test]
fn http_routes_with_same_path_and_different_methods_are_allowed() {
    let source = r#"
route GET /customers/:name {
    return "get " + name
}

route PUT /customers/:name {
    return "put " + name
}
"#;

    assert!(check_source(source).is_ok());
}

#[test]
fn route_query_params_are_typed_and_available_in_route_body() {
    let source = r#"
model Customer {
    name: string
    status: string
}

route GET /customers ?(status: string, limit: int, offset: int) {
    return Customer::where("status", status, limit, offset)
}
"#;

    assert!(check_source(source).is_ok());
}

#[test]
fn route_query_params_allow_optional_types_and_defaults() {
    let source = r#"
model Customer {
    name: string
}

route GET /customers ?(limit: int = 20, offset: int = 0) {
    return Customer::all(limit, offset)
}

route GET /customers/status ?(status: string?) {
    return status
}

route GET /reports ?(day: date = "2026-01-01") {
    return "ok"
}

route GET /payments ?(amount: money, maybe_amount: money?, default_amount: money = 1000 kz) {
    return amount
}

route GET /tags ?(tags: [string], maybe_tags: [string]?, default_tags: [string] = ["active", "blocked"]) {
    return tags
}

route GET /amounts ?(amounts: [money] = [1000 kz, 2000 usd]) {
    return amounts
}
"#;

    assert!(check_source(source).is_ok());
}

#[test]
fn route_query_params_reject_duplicates_and_unsupported_types() {
    let source = r#"
route GET /customers/:status ?(status: string) {
    return status
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("parâmetro 'status' mais de uma vez"));

    let source = r#"
route GET /tags ?(tags: [[string]]) {
    return "ok"
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("query param 'tags' usa tipo nao suportado"));

    let source = r#"
route GET /tags ?(tags: [string?]) {
    return "ok"
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("query param 'tags' usa tipo nao suportado"));
}

#[test]
fn route_query_params_reject_invalid_defaults() {
    let source = r#"
route GET /customers ?(limit: int = "many") {
    return limit
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("query param 'limit' default invalido"));

    let source = r#"
route GET /customers ?(limit: int = missing) {
    return limit
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("default de query param"));
}

#[test]
fn route_requires_single_direct_return() {
    let source = r#"
route GET /health {
    print("ok")
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("deve conter um unico return direto"));
}

#[test]
fn route_rejects_unsupported_return_calls() {
    let source = r#"
fn status() -> string {
    return "ok"
}

route GET /health {
    return status()
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("nao suporta chamada 'status()'"));
}

#[test]
fn route_model_create_requires_post() {
    let source = r#"
model Customer {
    name: string
}

route GET /customers {
    return Customer::create()
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::create() so pode ser usado em route POST"));
}

#[test]
fn route_model_find_validates_field_name_and_value_type() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

route GET /customers/:name {
    return Customer::find("missing", name)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Campo 'Customer.missing' nao existe"));

    let source = r#"
model Customer {
    name: string
    balance: money
}

route GET /customers/:name {
    return Customer::find("balance", name)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::find() valor invalido para 'balance'"));
}

#[test]
fn route_model_where_requires_get_and_validates_lookup() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

route POST /customers/:name {
    return Customer::where("name", name)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::where() so pode ser usado em route GET"));

    let source = r#"
model Customer {
    name: string
    balance: money
}

route GET /customers/:name {
    return Customer::where("balance", name)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::where() valor invalido para 'balance'"));
}

#[test]
fn route_model_where_not_requires_get_and_validates_lookup() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

route POST /customers/:name {
    return Customer::where_not("name", name)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::where_not() so pode ser usado em route GET"));

    let source = r#"
model Customer {
    name: string
    balance: money
}

route GET /customers/:name {
    return Customer::where_not("balance", name)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::where_not() valor invalido para 'balance'"));

    let source = r#"
model Customer {
    name: string
    status: string
}

route GET /customers/search ?(status: string, limit: int, offset: int) {
    return Customer::where_not("status", status, "name", "asc", limit, offset)
}
"#;

    assert!(check_source(source).is_ok());
}

#[test]
fn route_model_where_optional_requires_get_and_optional_lookup() {
    let source = r#"
model Customer {
    name: string
    status: string
}

route POST /customers/search ?(status: string?) {
    return Customer::where_optional("status", status)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::where_optional() so pode ser usado em route GET"));

    let source = r#"
model Customer {
    name: string
    status: string
}

route GET /customers/search ?(status: string) {
    return Customer::where_optional("status", status)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::where_optional() valor para 'status' deve ser opcional"));

    let source = r#"
model Customer {
    name: string
    status: string
}

route GET /customers/search ?(status: int?) {
    return Customer::where_optional("status", status)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::where_optional() valor invalido para 'status'"));

    let source = r#"
model Customer {
    name: string
    status: string
}

route GET /customers/search ?(status: string?, limit: int, offset: int) {
    return Customer::where_optional("status", status, "name", "asc", limit, offset)
}
"#;

    assert!(check_source(source).is_ok());
}

#[test]
fn route_model_where_compare_requires_get_and_validates_operator_type() {
    let source = r#"
model Customer {
    name: string
    balance: float
}

route POST /customers/search ?(min: float) {
    return Customer::where_compare("balance", ">=", min)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::where_compare() so pode ser usado em route GET"));

    let source = r#"
model Customer {
    name: string
    balance: float
}

route GET /customers/search ?(min: float) {
    return Customer::where_compare("balance", "between", min)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::where_compare() operador deve ser"));

    let source = r#"
model Customer {
    name: string
    active: bool
}

route GET /customers/search ?(active: bool) {
    return Customer::where_compare("active", ">", active)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("campo do tipo bool nao suporta operador '>'"));

    let source = r#"
model Customer {
    name: string
    balance: float
}

route GET /customers/search ?(name: string) {
    return Customer::where_compare("balance", ">=", name)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("Customer::where_compare() valor invalido para 'balance'"));

    let source = r#"
model Customer {
    name: string
    balance: float
}

route GET /customers/search ?(min: float, limit: int, offset: int) {
    return Customer::where_compare("balance", ">=", min, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_compare_query_filter");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Dina","balance":200},{"name":"Ana","balance":50},{"name":"Cris","balance":300},{"name":"Bia","balance":150}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/search?min=100&limit=2&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Bia","balance":150},{"name":"Cris","balance":300}]"#
    );
}

#[test]
fn http_get_model_where_compare_supports_date_ordering_comparison() {
    let source = r#"
model Invoice {
    code: string
    due: date
}

route GET /invoices/due ?(after: date) {
    return Invoice::where_compare("due", ">", after, "due", "asc")
}
"#;
    let data_dir = temp_data_dir("model_where_compare_date_filter");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("invoice.json"),
        r#"[{"code":"A","due":"2026-01-10"},{"code":"B","due":"2026-02-05"},{"code":"C","due":"2026-01-20"}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/invoices/due?after=2026-01-15",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"code":"C","due":"2026-01-20"},{"code":"B","due":"2026-02-05"}]"#
    );
}

#[test]
fn http_get_model_where_compare_uses_money_query_param_filter() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

route GET /customers/balance ?(min: money, limit: int = 10, offset: int = 0) {
    return Customer::where_compare_page("balance", ">=", min, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_compare_money_query_filter");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Dina","balance":{"amount":200,"currency":"kz"}},{"name":"Ana","balance":{"amount":50,"currency":"kz"}},{"name":"Cris","balance":{"amount":300,"currency":"kz"}},{"name":"Bia","balance":{"amount":150,"currency":"kz"}}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/balance?min=100:kz&limit=2&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":3,"items":[{"name":"Bia","balance":{"amount":150,"currency":"kz"}},{"name":"Cris","balance":{"amount":300,"currency":"kz"}}]}"#
    );
}

#[test]
fn http_get_model_where_text_filters_before_ordering_and_pagination() {
    let source = r#"
model Customer {
    name: string
    email: string?
}

route GET /customers/search ?(term: string, limit: int = 10, offset: int = 0) {
    return Customer::where_text("name", "contains", term, "name", "asc", limit, offset)
}

route GET /customers/email_prefix ?(term: string) {
    return Customer::where_text("email", "starts_with", term, "name", "asc")
}

route GET /customers/email_domain ?(term: string) {
    return Customer::where_text("email", "ends_with", term, "name", "asc")
}
"#;
    let data_dir = temp_data_dir("model_where_text_query_filter");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Dina Silva","email":"dina@example.com"},{"name":"Ana Santos","email":"ana@sales.example.com"},{"name":"Bia Silva","email":"bia@example.org"},{"name":"Cris Lima","email":null}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/search?term=Silva&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Dina Silva","email":"dina@example.com"}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/email_prefix?term=ana@",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana Santos","email":"ana@sales.example.com"}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/email_domain?term=example.com",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana Santos","email":"ana@sales.example.com"},{"name":"Dina Silva","email":"dina@example.com"}]"#
    );
}

#[test]
fn http_get_model_where_text_case_insensitive_filters() {
    let source = r#"
model Customer {
    name: string
    email: string?
}

route GET /customers/search ?(term: string, limit: int = 10, offset: int = 0) {
    return Customer::where_text("name", "icontains", term, "name", "asc", limit, offset)
}

route GET /customers/email_prefix ?(term: string) {
    return Customer::where_text("email", "istarts_with", term, "name", "asc")
}

route GET /customers/email_domain ?(term: string) {
    return Customer::where_text("email", "iends_with", term, "name", "asc")
}
"#;
    let data_dir = temp_data_dir("model_where_text_case_insensitive_filter");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Dina Silva","email":"DINA@EXAMPLE.COM"},{"name":"Ana Santos","email":"Ana@Sales.Example.Com"},{"name":"Bia Silva","email":"bia@example.org"},{"name":"Cris Lima","email":null}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/search?term=silva&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Dina Silva","email":"DINA@EXAMPLE.COM"}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/email_prefix?term=ana@",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana Santos","email":"Ana@Sales.Example.Com"}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/email_domain?term=example.com",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana Santos","email":"Ana@Sales.Example.Com"},{"name":"Dina Silva","email":"DINA@EXAMPLE.COM"}]"#
    );
}

#[test]
fn http_get_model_where_text_case_insensitive_page_returns_total_before_slice() {
    let source = r#"
model Customer {
    name: string
}

route GET /customers/text ?(term: string, limit: int = 10, offset: int = 0) {
    return Customer::where_text_page("name", "icontains", term, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_text_case_insensitive_page");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Dina Silva"},{"name":"ana santos"},{"name":"BIA SILVA"},{"name":"Cris Lima"}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/text?term=silva&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":2,"items":[{"name":"Dina Silva"}]}"#
    );
}

#[test]
fn http_get_model_where_between_filters_inclusive_before_ordering_and_pagination() {
    let source = r#"
model Customer {
    name: string
    balance: float
}

route GET /customers/range ?(min: float, max: float, limit: int = 10, offset: int = 0) {
    return Customer::where_between("balance", min, max, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_between_query_filter");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Dina","balance":200},{"name":"Ana","balance":50},{"name":"Cris","balance":300},{"name":"Bia","balance":150}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/range?min=150&max=300&limit=2&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Bia","balance":150},{"name":"Cris","balance":300}]"#
    );
}

#[test]
fn http_get_model_where_between_supports_date_ranges() {
    let source = r#"
model Invoice {
    code: string
    due: date
}

route GET /invoices/range ?(start: date, end: date) {
    return Invoice::where_between("due", start, end, "due", "asc")
}
"#;
    let data_dir = temp_data_dir("model_where_between_date_filter");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("invoice.json"),
        r#"[{"code":"A","due":"2026-01-10"},{"code":"B","due":"2026-02-05"},{"code":"C","due":"2026-01-20"}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/invoices/range?start=2026-01-10&end=2026-01-31",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"code":"A","due":"2026-01-10"},{"code":"C","due":"2026-01-20"}]"#
    );
}

#[test]
fn http_get_model_where_with_pagination_returns_matching_slice() {
    let source = r#"
model Customer {
    name: string
    status: string
}

route GET /customers/status/:status/page {
    return Customer::where("status", status, 1, 1)
}
"#;
    let data_dir = temp_data_dir("model_where_page");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Ana","status":"active"},{"name":"Bia","status":"active"},{"name":"Cris","status":"active"}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status/active/page",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, r#"[{"name":"Bia","status":"active"}]"#);
}

#[test]
fn http_get_model_where_with_ordering_and_pagination_sorts_before_slice() {
    let source = r#"
model Customer {
    name: string
    status: string
}

route GET /customers/status/:status/order {
    return Customer::where("status", status, "name", "asc", 1, 1)
}
"#;
    let data_dir = temp_data_dir("model_where_order_page");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"active"},{"name":"Ana","status":"active"},{"name":"Bia","status":"active"}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status/active/order",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, r#"[{"name":"Bia","status":"active"}]"#);
}

#[test]
fn http_get_model_where_page_returns_filtered_total_before_slice() {
    let source = r#"
model Customer {
    name: string
    status: string
}

route GET /customers/status ?(status: string, limit: int = 10, offset: int = 0) {
    return Customer::where_page("status", status, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_page_total");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"active"},{"name":"Dina","status":"blocked"},{"name":"Ana","status":"active"},{"name":"Bia","status":"active"}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?status=active&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":3,"items":[{"name":"Bia","status":"active"}]}"#
    );
}

#[test]
fn http_get_model_where_not_page_returns_filtered_total_before_slice() {
    let source = r#"
model Customer {
    name: string
    status: string
}

route GET /customers/status ?(status: string, limit: int = 10, offset: int = 0) {
    return Customer::where_not_page("status", status, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_not_page_total");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"active"},{"name":"Dina","status":"blocked"},{"name":"Ana","status":"pending"},{"name":"Bia","status":"active"}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?status=active&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":2,"items":[{"name":"Dina","status":"blocked"}]}"#
    );
}

#[test]
fn http_get_model_advanced_page_filters_return_total_before_slice() {
    let source = r#"
model Customer {
    name: string
    status: string
    tenant: string
    balance: float
}

route GET /customers/optional ?(status: string?, limit: int = 10, offset: int = 0) {
    return Customer::where_optional_page("status", status, "name", "asc", limit, offset)
}

route GET /customers/compare ?(min: float, limit: int = 10, offset: int = 0) {
    return Customer::where_compare_page("balance", ">=", min, "name", "asc", limit, offset)
}

route GET /customers/text ?(term: string, limit: int = 10, offset: int = 0) {
    return Customer::where_text_page("name", "contains", term, "name", "asc", limit, offset)
}

route GET /customers/range ?(min: float, max: float, limit: int = 10, offset: int = 0) {
    return Customer::where_between_page("balance", min, max, "name", "asc", limit, offset)
}

route GET /customers/all ?(status: string, tenant: string, limit: int = 10, offset: int = 0) {
    return Customer::where_all_page("status", status, "tenant", tenant, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_advanced_page_totals");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"active","tenant":"other","balance":250},{"name":"Dina","status":"blocked","tenant":"main","balance":350},{"name":"Ana","status":"active","tenant":"main","balance":50},{"name":"Bia","status":"active","tenant":"main","balance":150}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/optional?limit=2&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":4,"items":[{"name":"Bia","status":"active","tenant":"main","balance":150},{"name":"Cris","status":"active","tenant":"other","balance":250}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/compare?min=100&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":3,"items":[{"name":"Cris","status":"active","tenant":"other","balance":250}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/text?term=i&limit=2&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":3,"items":[{"name":"Bia","status":"active","tenant":"main","balance":150},{"name":"Cris","status":"active","tenant":"other","balance":250}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/range?min=100&max=300&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":2,"items":[{"name":"Cris","status":"active","tenant":"other","balance":250}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/all?status=active&tenant=main&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":2,"items":[{"name":"Bia","status":"active","tenant":"main","balance":150}]}"#
    );
}

#[test]
fn http_get_model_where_all_uses_typed_query_param_filters() {
    let source = r#"
model Customer {
    name: string
    status: string
    tenant: string
}

route GET /customers/search ?(status: string, tenant: string) {
    return Customer::where_all("status", status, "tenant", tenant)
}
"#;
    let data_dir = temp_data_dir("model_where_all_query_filters");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Ana","status":"active","tenant":"main"},{"name":"Bia","status":"active","tenant":"other"},{"name":"Cris","status":"blocked","tenant":"main"},{"name":"Dina","status":"active","tenant":"main"}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/search?status=active&tenant=main",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana","status":"active","tenant":"main"},{"name":"Dina","status":"active","tenant":"main"}]"#
    );
}

#[test]
fn http_get_model_where_all_filters_before_ordering_and_pagination() {
    let source = r#"
model Customer {
    name: string
    status: string
    tenant: string
}

route GET /customers/search ?(status: string, tenant: string, limit: int, offset: int) {
    return Customer::where_all("status", status, "tenant", tenant, "name", "asc", limit, offset)
}

route GET /customers/page ?(status: string, tenant: string, limit: int, offset: int) {
    return Customer::where_all("status", status, "tenant", tenant, limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_all_order_page");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"active","tenant":"main"},{"name":"Ana","status":"active","tenant":"main"},{"name":"Bia","status":"active","tenant":"main"},{"name":"Zara","status":"blocked","tenant":"main"}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/search?status=active&tenant=main&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Bia","status":"active","tenant":"main"}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/page?status=active&tenant=main&limit=2&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana","status":"active","tenant":"main"},{"name":"Bia","status":"active","tenant":"main"}]"#
    );
}

#[test]
fn http_get_model_where_any_filters_before_ordering_and_pagination() {
    let source = r#"
model Customer {
    name: string
    status: string
    tenant: string
}

route GET /customers/search ?(status: string, tenant: string, limit: int, offset: int) {
    return Customer::where_any("status", status, "tenant", tenant, "name", "asc", limit, offset)
}

route GET /customers/page ?(status: string, tenant: string, limit: int, offset: int) {
    return Customer::where_any("status", status, "tenant", tenant, limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_any_order_page");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"active","tenant":"main"},{"name":"Ana","status":"blocked","tenant":"main"},{"name":"Bia","status":"active","tenant":"other"},{"name":"Zara","status":"blocked","tenant":"other"}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/search?status=active&tenant=main&limit=2&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Bia","status":"active","tenant":"other"},{"name":"Cris","status":"active","tenant":"main"}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/page?status=active&tenant=main&limit=2&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana","status":"blocked","tenant":"main"},{"name":"Bia","status":"active","tenant":"other"}]"#
    );
}

#[test]
fn http_get_model_where_any_page_returns_total_before_slice() {
    let source = r#"
model Customer {
    name: string
    status: string
    tenant: string
}

route GET /customers/search ?(status: string, tenant: string, limit: int, offset: int) {
    return Customer::where_any_page("status", status, "tenant", tenant, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_any_page_total");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"active","tenant":"main"},{"name":"Ana","status":"blocked","tenant":"main"},{"name":"Bia","status":"active","tenant":"other"},{"name":"Zara","status":"blocked","tenant":"other"}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/search?status=active&tenant=main&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":3,"items":[{"name":"Bia","status":"active","tenant":"other"}]}"#
    );
}

#[test]
fn http_get_model_where_in_uses_array_query_param_filters() {
    let source = r#"
model Customer {
    name: string
    status: string
    tier: int
    balance: money
}

route GET /customers/status ?(statuses: [string]) {
    return Customer::where_in("status", statuses, "name", "asc")
}

route GET /customers/tiers ?(tiers: [int], limit: int, offset: int) {
    return Customer::where_in("tier", tiers, "name", "asc", limit, offset)
}

route GET /customers/balances ?(balances: [money]) {
    return Customer::where_in("balance", balances, "name", "asc")
}
"#;
    let data_dir = temp_data_dir("model_where_in_query_filters");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"blocked","tier":2,"balance":{"amount":200,"currency":"kz"}},{"name":"Ana","status":"active","tier":1,"balance":{"amount":100,"currency":"kz"}},{"name":"Bia","status":"pending","tier":2,"balance":{"amount":200,"currency":"kz"}},{"name":"Dina","status":"active","tier":3,"balance":{"amount":300,"currency":"usd"}}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=active,pending",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana","status":"active","tier":1,"balance":{"amount":100,"currency":"kz"}},{"name":"Bia","status":"pending","tier":2,"balance":{"amount":200,"currency":"kz"}},{"name":"Dina","status":"active","tier":3,"balance":{"amount":300,"currency":"usd"}}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/tiers?tiers=1,2&limit=2&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Bia","status":"pending","tier":2,"balance":{"amount":200,"currency":"kz"}},{"name":"Cris","status":"blocked","tier":2,"balance":{"amount":200,"currency":"kz"}}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/balances?balances=200:kz,300:usd",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Bia","status":"pending","tier":2,"balance":{"amount":200,"currency":"kz"}},{"name":"Cris","status":"blocked","tier":2,"balance":{"amount":200,"currency":"kz"}},{"name":"Dina","status":"active","tier":3,"balance":{"amount":300,"currency":"usd"}}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, "[]");
}

#[test]
fn http_get_model_where_in_page_returns_total_before_slice() {
    let source = r#"
model Customer {
    name: string
    status: string
    tier: int
}

route GET /customers/status ?(statuses: [string], limit: int, offset: int) {
    return Customer::where_in_page("status", statuses, "name", "asc", limit, offset)
}

route GET /customers/tiers ?(tiers: [int], limit: int, offset: int) {
    return Customer::where_in_page("tier", tiers, limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_in_page");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"blocked","tier":2},{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Dina","status":"active","tier":3}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=active,pending&limit=2&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":3,"items":[{"name":"Bia","status":"pending","tier":2},{"name":"Dina","status":"active","tier":3}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/tiers?tiers=2,3&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":3,"items":[{"name":"Bia","status":"pending","tier":2}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=&limit=10&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, r#"{"total":0,"items":[]}"#);
}

#[test]
fn http_get_model_where_not_in_uses_array_query_param_filters() {
    let source = r#"
model Customer {
    name: string
    status: string?
    tier: int
}

route GET /customers/status ?(statuses: [string]) {
    return Customer::where_not_in("status", statuses, "name", "asc")
}

route GET /customers/tiers ?(tiers: [int], limit: int, offset: int) {
    return Customer::where_not_in("tier", tiers, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_not_in_query_filters");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"blocked","tier":2},{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Dina","status":"active","tier":3},{"name":"Eli","tier":4}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=active,pending",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Cris","status":"blocked","tier":2}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/tiers?tiers=1,3&limit=2&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Bia","status":"pending","tier":2},{"name":"Cris","status":"blocked","tier":2}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Cris","status":"blocked","tier":2},{"name":"Dina","status":"active","tier":3}]"#
    );
}

#[test]
fn http_get_model_where_not_in_page_returns_total_before_slice() {
    let source = r#"
model Customer {
    name: string
    status: string?
    tier: int
}

route GET /customers/status ?(statuses: [string], limit: int, offset: int) {
    return Customer::where_not_in_page("status", statuses, "name", "asc", limit, offset)
}

route GET /customers/tiers ?(tiers: [int], limit: int, offset: int) {
    return Customer::where_not_in_page("tier", tiers, limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_not_in_page");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"blocked","tier":2},{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Dina","status":"active","tier":3},{"name":"Eva","status":"suspended","tier":4},{"name":"NoStatus","tier":5}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=active,pending&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":2,"items":[{"name":"Eva","status":"suspended","tier":4}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/tiers?tiers=2,3&limit=2&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":3,"items":[{"name":"Ana","status":"active","tier":1},{"name":"Eva","status":"suspended","tier":4}]}"#
    );
}

#[test]
fn http_get_model_where_not_in_optional_ignores_absent_query_param_and_filters_present() {
    let source = r#"
model Customer {
    name: string
    status: string?
    tier: int
}

route GET /customers/status ?(statuses: [string]?, limit: int, offset: int) {
    return Customer::where_not_in_optional("status", statuses, "name", "asc", limit, offset)
}

route GET /customers/tiers ?(tiers: [int]?) {
    return Customer::where_not_in_optional("tier", tiers, "name", "asc")
}
"#;
    let data_dir = temp_data_dir("model_where_not_in_optional");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"blocked","tier":2},{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Dina","status":"active","tier":3},{"name":"Eli","tier":4}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?limit=3&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Bia","status":"pending","tier":2},{"name":"Cris","status":"blocked","tier":2},{"name":"Dina","status":"active","tier":3}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=active,pending&limit=10&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Cris","status":"blocked","tier":2}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=&limit=10&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Cris","status":"blocked","tier":2},{"name":"Dina","status":"active","tier":3}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/tiers?tiers=2,3",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana","status":"active","tier":1},{"name":"Eli","status":null,"tier":4}]"#
    );
}

#[test]
fn http_get_model_where_not_in_optional_page_returns_total_before_slice() {
    let source = r#"
model Customer {
    name: string
    status: string?
    tier: int
}

route GET /customers/status ?(statuses: [string]?, limit: int, offset: int) {
    return Customer::where_not_in_optional_page("status", statuses, "name", "asc", limit, offset)
}

route GET /customers/tiers ?(tiers: [int]?, limit: int, offset: int) {
    return Customer::where_not_in_optional_page("tier", tiers, limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_not_in_optional_page");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"blocked","tier":2},{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Dina","status":"active","tier":3},{"name":"Eva","status":"suspended","tier":4},{"name":"NoStatus","tier":5}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?limit=2&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":6,"items":[{"name":"Bia","status":"pending","tier":2},{"name":"Cris","status":"blocked","tier":2}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=active,pending&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":2,"items":[{"name":"Eva","status":"suspended","tier":4}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=&limit=10&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":5,"items":[{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Cris","status":"blocked","tier":2},{"name":"Dina","status":"active","tier":3},{"name":"Eva","status":"suspended","tier":4}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/tiers?tiers=2,3&limit=2&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":3,"items":[{"name":"Ana","status":"active","tier":1},{"name":"Eva","status":"suspended","tier":4}]}"#
    );
}

#[test]
fn http_get_model_where_in_optional_ignores_absent_query_param_and_filters_present() {
    let source = r#"
model Customer {
    name: string
    status: string
    tier: int
}

route GET /customers/status ?(statuses: [string]?, limit: int, offset: int) {
    return Customer::where_in_optional("status", statuses, "name", "asc", limit, offset)
}

route GET /customers/tiers ?(tiers: [int]?) {
    return Customer::where_in_optional("tier", tiers, "name", "asc")
}
"#;
    let data_dir = temp_data_dir("model_where_in_optional");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"blocked","tier":2},{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Dina","status":"active","tier":3}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?limit=2&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Bia","status":"pending","tier":2},{"name":"Cris","status":"blocked","tier":2}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=active,pending&limit=10&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Dina","status":"active","tier":3}]"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=&limit=10&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, "[]");

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/customers/tiers", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"[{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Cris","status":"blocked","tier":2},{"name":"Dina","status":"active","tier":3}]"#
    );
}

#[test]
fn http_get_model_where_in_optional_page_returns_total_before_slice() {
    let source = r#"
model Customer {
    name: string
    status: string
    tier: int
}

route GET /customers/status ?(statuses: [string]?, limit: int, offset: int) {
    return Customer::where_in_optional_page("status", statuses, "name", "asc", limit, offset)
}

route GET /customers/tiers ?(tiers: [int]?, limit: int, offset: int) {
    return Customer::where_in_optional_page("tier", tiers, limit, offset)
}
"#;
    let data_dir = temp_data_dir("model_where_in_optional_page");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Cris","status":"blocked","tier":2},{"name":"Ana","status":"active","tier":1},{"name":"Bia","status":"pending","tier":2},{"name":"Dina","status":"active","tier":3}]"#,
    )
    .unwrap();

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?limit=2&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":4,"items":[{"name":"Bia","status":"pending","tier":2},{"name":"Cris","status":"blocked","tier":2}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=active,pending&limit=2&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":3,"items":[{"name":"Bia","status":"pending","tier":2},{"name":"Dina","status":"active","tier":3}]}"#
    );

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/status?statuses=&limit=10&offset=0",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body, r#"{"total":0,"items":[]}"#);

    let response = nexuslang::server::handle_request_for_test(
        source,
        "GET",
        "/customers/tiers?tiers=2,3&limit=1&offset=1",
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"total":3,"items":[{"name":"Bia","status":"pending","tier":2}]}"#
    );
}

#[test]
fn http_put_model_update_replaces_record_and_fills_defaults() {
    let source = r#"
model Customer {
    name: string
    status: string = "active"
    email: string?
    balance: money
    active: bool = true
}

route PUT /customers/:name {
    return Customer::update("name", name)
}

route GET /customers {
    return Customer::all()
}
"#;
    let data_dir = temp_data_dir("model_update");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Ana","status":"old","balance":{"amount":1000,"currency":"kz"}},{"name":"Bia","balance":{"amount":2000,"currency":"kz"}}]"#,
    )
    .unwrap();
    let request_body = r#"{"name":"Ana","balance":{"amount":1500,"currency":"kz"}}"#;

    let update_response = nexuslang::server::handle_request_with_body_for_test(
        source,
        "PUT",
        "/customers/Ana",
        request_body,
        &storage,
    )
    .unwrap();

    assert_eq!(update_response.status, 200);
    assert_eq!(
        update_response.body,
        r#"{"name":"Ana","status":"active","email":null,"balance":{"amount":1500,"currency":"kz"},"active":true}"#
    );

    let list_response =
        nexuslang::server::handle_request_for_test(source, "GET", "/customers", &storage).unwrap();

    assert_eq!(list_response.status, 200);
    assert_eq!(
        list_response.body,
        r#"[{"name":"Ana","status":"active","email":null,"balance":{"amount":1500,"currency":"kz"},"active":true},{"name":"Bia","balance":{"amount":2000,"currency":"kz"}}]"#
    );

    let stored = fs::read_to_string(data_dir.join("customer.json")).unwrap();
    assert_eq!(stored.trim(), list_response.body);
}

#[test]
fn http_put_model_update_returns_404_without_modifying_storage() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

route PUT /customers/:name {
    return Customer::update("name", name)
}
"#;
    let data_dir = temp_data_dir("model_update_missing");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    let original = r#"[{"name":"Ana","balance":{"amount":1000,"currency":"kz"}}]"#;
    fs::write(data_dir.join("customer.json"), original).unwrap();
    let request_body = r#"{"name":"Bia","balance":{"amount":2000,"currency":"kz"}}"#;

    let response = nexuslang::server::handle_request_with_body_for_test(
        source,
        "PUT",
        "/customers/Bia",
        request_body,
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 404);
    assert!(response.body.contains("Nao encontrado"));
    assert_eq!(
        fs::read_to_string(data_dir.join("customer.json"))
            .unwrap()
            .trim(),
        original
    );
}

#[test]
fn http_put_model_update_rejects_invalid_request_body_without_modifying_storage() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

route PUT /customers/:name {
    return Customer::update("name", name)
}
"#;
    let data_dir = temp_data_dir("model_update_invalid");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    let original = r#"[{"name":"Ana","balance":{"amount":1000,"currency":"kz"}}]"#;
    fs::write(data_dir.join("customer.json"), original).unwrap();
    let request_body = r#"{"name":"Ana"}"#;

    let response = nexuslang::server::handle_request_with_body_for_test(
        source,
        "PUT",
        "/customers/Ana",
        request_body,
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 400);
    assert!(response
        .body
        .contains("campo 'balance' obrigatorio ausente"));
    assert_eq!(
        fs::read_to_string(data_dir.join("customer.json"))
            .unwrap()
            .trim(),
        original
    );
}

#[test]
fn http_put_model_update_rejects_duplicate_unique_field_without_modifying_storage() {
    let source = r#"
model Customer {
    name: string
    email: string unique
    balance: money
}

route PUT /customers/:name {
    return Customer::update("name", name)
}
"#;
    let data_dir = temp_data_dir("model_update_unique_conflict");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    let original = r#"[{"name":"Ana","email":"ana@example.com","balance":{"amount":1000,"currency":"kz"}},{"name":"Bia","email":"bia@example.com","balance":{"amount":2000,"currency":"kz"}}]"#;
    fs::write(data_dir.join("customer.json"), original).unwrap();
    let request_body =
        r#"{"name":"Ana","email":"bia@example.com","balance":{"amount":1500,"currency":"kz"}}"#;

    let response = nexuslang::server::handle_request_with_body_for_test(
        source,
        "PUT",
        "/customers/Ana",
        request_body,
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 409);
    assert!(response.body.contains("unique ja existe"));
    assert_eq!(
        fs::read_to_string(data_dir.join("customer.json"))
            .unwrap()
            .trim(),
        original
    );
}

#[test]
fn http_put_model_update_allows_unchanged_unique_field() {
    let source = r#"
model Customer {
    name: string
    email: string unique
    balance: money
}

route PUT /customers/:name {
    return Customer::update("name", name)
}
"#;
    let data_dir = temp_data_dir("model_update_unique_same");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Ana","email":"ana@example.com","balance":{"amount":1000,"currency":"kz"}}]"#,
    )
    .unwrap();
    let request_body =
        r#"{"name":"Ana","email":"ana@example.com","balance":{"amount":1500,"currency":"kz"}}"#;

    let response = nexuslang::server::handle_request_with_body_for_test(
        source,
        "PUT",
        "/customers/Ana",
        request_body,
        &storage,
    )
    .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        r#"{"name":"Ana","email":"ana@example.com","balance":{"amount":1500,"currency":"kz"}}"#
    );
}

#[test]
fn http_delete_model_delete_removes_record_and_returns_deleted_record() {
    let source = r#"
model Customer {
    name: string
    status: string = "active"
    email: string?
    balance: money
    active: bool = true
}

route DELETE /customers/:name {
    return Customer::delete("name", name)
}

route GET /customers {
    return Customer::all()
}
"#;
    let data_dir = temp_data_dir("model_delete");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(
        data_dir.join("customer.json"),
        r#"[{"name":"Ana","balance":{"amount":1000,"currency":"kz"}},{"name":"Bia","balance":{"amount":2000,"currency":"kz"}}]"#,
    )
    .unwrap();

    let delete_response =
        nexuslang::server::handle_request_for_test(source, "DELETE", "/customers/Ana", &storage)
            .unwrap();

    assert_eq!(delete_response.status, 200);
    assert_eq!(
        delete_response.body,
        r#"{"name":"Ana","status":"active","email":null,"balance":{"amount":1000,"currency":"kz"},"active":true}"#
    );

    let list_response =
        nexuslang::server::handle_request_for_test(source, "GET", "/customers", &storage).unwrap();

    assert_eq!(list_response.status, 200);
    assert_eq!(
        list_response.body,
        r#"[{"name":"Bia","balance":{"amount":2000,"currency":"kz"}}]"#
    );

    let stored = fs::read_to_string(data_dir.join("customer.json")).unwrap();
    assert_eq!(stored.trim(), list_response.body);
}

#[test]
fn http_delete_model_delete_returns_404_without_modifying_storage() {
    let source = r#"
model Customer {
    name: string
    balance: money
}

route DELETE /customers/:name {
    return Customer::delete("name", name)
}
"#;
    let data_dir = temp_data_dir("model_delete_missing");
    let storage = nexuslang::server::Storage::new_json(&data_dir);
    fs::create_dir_all(&data_dir).unwrap();
    let original = r#"[{"name":"Ana","balance":{"amount":1000,"currency":"kz"}}]"#;
    fs::write(data_dir.join("customer.json"), original).unwrap();

    let response =
        nexuslang::server::handle_request_for_test(source, "DELETE", "/customers/Bia", &storage)
            .unwrap();

    assert_eq!(response.status, 404);
    assert!(response.body.contains("Nao encontrado"));
    assert_eq!(
        fs::read_to_string(data_dir.join("customer.json"))
            .unwrap()
            .trim(),
        original
    );
}

#[test]
fn sqlite_storage_matches_json_storage_for_crud_and_critical_filters() {
    let source = r#"
model Customer {
    name: string unique
    email: string unique
    status: string = "active" index
    tenant: string = "main"
    balance: money
    score: int
    active: bool = true
}

route POST /customers {
    return Customer::create()
}

route GET /customers/order {
    return Customer::all("name", "asc")
}

route GET /customers/page ?(limit: int, offset: int) {
    return Customer::page("name", "asc", limit, offset)
}

route GET /customers/by-status ?(status: string) {
    return Customer::where("status", status, "name", "asc")
}

route GET /customers/not-status ?(status: string) {
    return Customer::where_not("status", status, "name", "asc")
}

route GET /customers/statuses ?(statuses: [string]) {
    return Customer::where_in("status", statuses, "name", "asc")
}

route GET /customers/not-statuses ?(statuses: [string]) {
    return Customer::where_not_in("status", statuses, "name", "asc")
}

route GET /customers/min-score ?(min_score: int) {
    return Customer::where_compare("score", ">=", min_score, "score", "asc")
}

route GET /customers/name-contains ?(term: string) {
    return Customer::where_text("name", "contains", term, "name", "asc")
}

route GET /customers/score-range ?(min: int, max: int) {
    return Customer::where_between("score", min, max, "name", "asc")
}

route GET /customers/all-filter ?(status: string, tenant: string, limit: int, offset: int) {
    return Customer::where_all("status", status, "tenant", tenant, "name", "asc", limit, offset)
}

route GET /customers/any-filter ?(status: string, tenant: string, limit: int, offset: int) {
    return Customer::where_any("status", status, "tenant", tenant, "name", "asc", limit, offset)
}

route GET /customers/:name {
    return Customer::find("name", name)
}

route PUT /customers/:name {
    return Customer::update("name", name)
}

route DELETE /customers/:name {
    return Customer::delete("name", name)
}
"#;

    let requests = [
        ParityRequest {
            label: "create ana",
            method: "POST",
            path: "/customers",
            body: Some(
                r#"{"name":"AnaSilva","email":"ana@example.com","status":"active","tenant":"main","balance":{"amount":100,"currency":"kz"},"score":80}"#,
            ),
        },
        ParityRequest {
            label: "create bia",
            method: "POST",
            path: "/customers",
            body: Some(
                r#"{"name":"BiaRocha","email":"bia@example.com","status":"pending","tenant":"main","balance":{"amount":250,"currency":"kz"},"score":65}"#,
            ),
        },
        ParityRequest {
            label: "create cris",
            method: "POST",
            path: "/customers",
            body: Some(
                r#"{"name":"CrisSilva","email":"cris@example.com","status":"blocked","tenant":"other","balance":{"amount":500,"currency":"kz"},"score":40}"#,
            ),
        },
        ParityRequest {
            label: "create dina",
            method: "POST",
            path: "/customers",
            body: Some(
                r#"{"name":"DinaCosta","email":"dina@example.com","status":"active","tenant":"main","balance":{"amount":300,"currency":"usd"},"score":90,"active":false}"#,
            ),
        },
        ParityRequest {
            label: "list ordered after create",
            method: "GET",
            path: "/customers/order",
            body: None,
        },
        ParityRequest {
            label: "find ana",
            method: "GET",
            path: "/customers/AnaSilva",
            body: None,
        },
        ParityRequest {
            label: "filter where",
            method: "GET",
            path: "/customers/by-status?status=active",
            body: None,
        },
        ParityRequest {
            label: "filter where_not",
            method: "GET",
            path: "/customers/not-status?status=blocked",
            body: None,
        },
        ParityRequest {
            label: "filter where_in",
            method: "GET",
            path: "/customers/statuses?statuses=active,pending",
            body: None,
        },
        ParityRequest {
            label: "filter where_not_in",
            method: "GET",
            path: "/customers/not-statuses?statuses=active,pending",
            body: None,
        },
        ParityRequest {
            label: "filter compare",
            method: "GET",
            path: "/customers/min-score?min_score=70",
            body: None,
        },
        ParityRequest {
            label: "filter text",
            method: "GET",
            path: "/customers/name-contains?term=Silva",
            body: None,
        },
        ParityRequest {
            label: "filter range",
            method: "GET",
            path: "/customers/score-range?min=60&max=85",
            body: None,
        },
        ParityRequest {
            label: "filter all",
            method: "GET",
            path: "/customers/all-filter?status=active&tenant=main&limit=10&offset=0",
            body: None,
        },
        ParityRequest {
            label: "filter any",
            method: "GET",
            path: "/customers/any-filter?status=pending&tenant=other&limit=10&offset=0",
            body: None,
        },
        ParityRequest {
            label: "page ordered",
            method: "GET",
            path: "/customers/page?limit=2&offset=1",
            body: None,
        },
        ParityRequest {
            label: "duplicate unique email",
            method: "POST",
            path: "/customers",
            body: Some(
                r#"{"name":"Eva Nova","email":"ana@example.com","status":"active","tenant":"main","balance":{"amount":900,"currency":"kz"},"score":50}"#,
            ),
        },
        ParityRequest {
            label: "delete bia",
            method: "DELETE",
            path: "/customers/BiaRocha",
            body: None,
        },
        ParityRequest {
            label: "update cris after delete",
            method: "PUT",
            path: "/customers/CrisSilva",
            body: Some(
                r#"{"name":"CrisSilva","email":"cris@example.com","status":"active","tenant":"other","balance":{"amount":700,"currency":"kz"},"score":72}"#,
            ),
        },
        ParityRequest {
            label: "list ordered after mutations",
            method: "GET",
            path: "/customers/order",
            body: None,
        },
        ParityRequest {
            label: "find deleted bia",
            method: "GET",
            path: "/customers/BiaRocha",
            body: None,
        },
    ];

    let json_responses = run_parity_requests(
        "storage_parity_crud_filters",
        source,
        ParityBackend::Json,
        &requests,
    );
    let sqlite_responses = run_parity_requests(
        "storage_parity_crud_filters",
        source,
        ParityBackend::Sqlite,
        &requests,
    );

    assert_eq!(sqlite_responses, json_responses);
    assert_recorded_response(
        &json_responses,
        "filter where_in",
        200,
        r#"[{"name":"AnaSilva","email":"ana@example.com","status":"active","tenant":"main","balance":{"amount":100,"currency":"kz"},"score":80,"active":true},{"name":"BiaRocha","email":"bia@example.com","status":"pending","tenant":"main","balance":{"amount":250,"currency":"kz"},"score":65,"active":true},{"name":"DinaCosta","email":"dina@example.com","status":"active","tenant":"main","balance":{"amount":300,"currency":"usd"},"score":90,"active":false}]"#,
    );
    assert_recorded_response(
        &json_responses,
        "filter any",
        200,
        r#"[{"name":"BiaRocha","email":"bia@example.com","status":"pending","tenant":"main","balance":{"amount":250,"currency":"kz"},"score":65,"active":true},{"name":"CrisSilva","email":"cris@example.com","status":"blocked","tenant":"other","balance":{"amount":500,"currency":"kz"},"score":40,"active":true}]"#,
    );
    assert_recorded_response_contains(
        &json_responses,
        "duplicate unique email",
        409,
        "unique ja existe",
    );
    assert_recorded_response(
        &json_responses,
        "update cris after delete",
        200,
        r#"{"name":"CrisSilva","email":"cris@example.com","status":"active","tenant":"other","balance":{"amount":700,"currency":"kz"},"score":72,"active":true}"#,
    );
    assert_recorded_response(
        &json_responses,
        "list ordered after mutations",
        200,
        r#"[{"name":"AnaSilva","email":"ana@example.com","status":"active","tenant":"main","balance":{"amount":100,"currency":"kz"},"score":80,"active":true},{"name":"CrisSilva","email":"cris@example.com","status":"active","tenant":"other","balance":{"amount":700,"currency":"kz"},"score":72,"active":true},{"name":"DinaCosta","email":"dina@example.com","status":"active","tenant":"main","balance":{"amount":300,"currency":"usd"},"score":90,"active":false}]"#,
    );
    assert_recorded_response_contains(&json_responses, "find deleted bia", 404, "Nao encontrado");
}

#[test]
fn storage_schema_evolution_allows_additive_optional_and_defaulted_fields() {
    let v1_source = r#"
model Customer {
    name: string unique
    balance: money
}

route POST /customers {
    return Customer::create()
}

route GET /customers/:name {
    return Customer::find("name", name)
}
"#;

    let v2_source = r#"
model Customer {
    name: string unique
    balance: money
    status: string = "active"
    email: string?
}

route GET /customers/:name {
    return Customer::find("name", name)
}
"#;

    let requests_v1 = [ParityRequest {
        label: "create old customer",
        method: "POST",
        path: "/customers",
        body: Some(r#"{"name":"AnaSilva","balance":{"amount":100,"currency":"kz"}}"#),
    }];

    for backend in [ParityBackend::Json, ParityBackend::Sqlite] {
        let storage = parity_storage("storage_schema_evolution_additive", backend);
        let create_response = run_requests_with_storage(v1_source, &storage, &requests_v1)
            .into_iter()
            .next()
            .expect("create response should be recorded");

        assert_eq!(
            create_response.status,
            201,
            "{} create should succeed before schema evolution",
            backend.label()
        );

        let read_response = nexuslang::server::handle_request_for_test(
            v2_source,
            "GET",
            "/customers/AnaSilva",
            &storage,
        )
        .unwrap_or_else(|err| {
            panic!(
                "{} read after schema evolution failed: {}",
                backend.label(),
                err
            )
        });

        assert_eq!(
            read_response.status,
            200,
            "{} read should succeed after additive schema evolution",
            backend.label()
        );
        assert_eq!(
            read_response.body,
            r#"{"name":"AnaSilva","balance":{"amount":100,"currency":"kz"},"status":"active","email":null}"#,
            "{} should materialize defaults and optional nulls for older records",
            backend.label()
        );
    }
}

#[test]
fn openapi_endpoint_lists_route_params() {
    let source = r#"
route GET /employees/:id {
    return "employee " + id
}
"#;
    let data_dir = temp_data_dir("openapi");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#"/employees/{id}"#));
    assert!(response.body.contains(r#""name":"id""#));
}

#[test]
fn openapi_endpoint_generates_stable_operation_ids() {
    let source = r#"
route GET /employees/:id {
    return "employee " + id
}

route GET /employees/by/id {
    return "employee static"
}

route POST /customers {
    return "created"
}

route PUT /customers/:name/profile {
    return "updated " + name
}

route DELETE /orders/:order/items/:item {
    return "deleted " + order + item
}
"#;
    let data_dir = temp_data_dir("openapi_operation_ids");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""operationId":"get_employees_by_id""#));
    assert!(response
        .body
        .contains(r#""operationId":"get_employees_by_id_2""#));
    assert!(response.body.contains(r#""operationId":"post_customers""#));
    assert!(response
        .body
        .contains(r#""operationId":"put_customers_by_name_profile""#));
    assert!(response
        .body
        .contains(r#""operationId":"delete_orders_by_order_items_by_item""#));
}

#[test]
fn openapi_endpoint_generates_stable_resource_tags() {
    let source = r#"
route GET /customers/:name {
    return "customer " + name
}

route POST /customers {
    return "created"
}

route GET /orders/:order/items/:item {
    return "item " + order + item
}

route GET /:tenant/reports {
    return "reports " + tenant
}

route GET /:tenant {
    return tenant
}
"#;
    let data_dir = temp_data_dir("openapi_resource_tags");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(
        r#""tags":[{"name":"customers"},{"name":"orders"},{"name":"reports"},{"name":"routes"}]"#
    ));
    assert!(response
        .body
        .contains(r#""operationId":"get_customers_by_name","tags":["customers"]"#));
    assert!(response
        .body
        .contains(r#""operationId":"post_customers","tags":["customers"]"#));
    assert!(response
        .body
        .contains(r#""operationId":"get_orders_by_order_items_by_item","tags":["orders"]"#));
    assert!(response
        .body
        .contains(r#""operationId":"get_by_tenant_reports","tags":["reports"]"#));
    assert!(response
        .body
        .contains(r#""operationId":"get_by_tenant","tags":["routes"]"#));
}

#[test]
fn openapi_endpoint_groups_methods_under_same_path() {
    let source = r#"
route GET /customers/:name {
    return "get " + name
}

route PUT /customers/:name {
    return "put " + name
}
"#;
    let data_dir = temp_data_dir("openapi_grouped_path_methods");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(response.body.matches(r#""/customers/{name}":{"#).count(), 1);
    assert!(response.body.contains(
        r##""/customers/{name}":{"get":{"summary":"GET /customers/:name","operationId":"get_customers_by_name","tags":["customers"],"parameters":[{"$ref":"#/components/parameters/NexusPathParam_name"}],"responses":{"200":{"description":"OK","content":{"application/json":{"schema":{"type":"string"}}}}}},"put":{"summary":"PUT /customers/:name","operationId":"put_customers_by_name","tags":["customers"],"parameters":[{"$ref":"#/components/parameters/NexusPathParam_name"}],"responses":{"200":{"description":"OK","content":{"application/json":{"schema":{"type":"string"}}}}}}}"##
    ));
    assert_eq!(
        response
            .body
            .matches(r##""$ref":"#/components/parameters/NexusPathParam_name""##)
            .count(),
        2
    );
}

#[test]
fn openapi_endpoint_generates_model_schema_with_optional_and_default_fields() {
    let source = r#"
model Customer {
    name: string
    status: string = "active"
    email: string?
    active: bool = true
}

route GET /customers/:name {
    return Customer { name: name }
}
"#;
    let data_dir = temp_data_dir("openapi_model_schema");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#"/customers/{name}"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/Customer"}"##));
    assert!(response.body.contains(r#""Customer":{"type":"object""#));
    assert!(response.body.contains(r#""name":{"type":"string"}"#));
    assert!(response
        .body
        .contains(r#""status":{"type":"string","default":"active"}"#));
    assert!(response
        .body
        .contains(r#""email":{"type":"string","nullable":true}"#));
    assert!(response
        .body
        .contains(r#""active":{"type":"boolean","default":true}"#));
    assert!(response.body.contains(r#""required":["name"]"#));
}

#[test]
fn openapi_endpoint_describes_model_arrays_and_optional_field_access() {
    let source = r#"
model Customer {
    name: string
    email: string?
    balance: money
}

route GET /customers {
    return Customer::all()
}

route GET /customers/:name/email {
    return Customer { name: name, balance: 1000 kz }.email
}
"#;
    let data_dir = temp_data_dir("openapi_route_schemas");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
    assert!(response.body.contains(
        r##""NexusList_Customer":{"type":"array","items":{"$ref":"#/components/schemas/Customer"}}"##
    ));
    assert!(response
        .body
        .contains(r#""schema":{"type":"string","nullable":true}"#));
    assert!(
        response.body.contains(
            r#""balance":{"type":"object","properties":{"amount":{"type":"number"},"currency":{"type":"string"}},"required":["amount","currency"]}"#
        )
    );
}

#[test]
fn openapi_endpoint_describes_model_create_request_body() {
    let source = r#"
model Customer {
    name: string
    status: string = "active"
}

route POST /customers {
    return Customer::create()
}
"#;
    let data_dir = temp_data_dir("openapi_create_request_body");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""post":{"summary":"POST /customers""#));
    assert!(response.body.contains(
        r##""requestBody":{"$ref":"#/components/requestBodies/NexusRequestBody_Customer"}"##
    ));
    assert!(
        response
            .body
            .contains(r##""NexusRequestBody_Customer":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Customer"}}}}"##)
    );
    assert!(response.body.contains(
        r##""responses":{"201":{"$ref":"#/components/responses/NexusResponse201_Customer"}"##
    ));
    assert!(response
        .body
        .contains(r#""400":{"description":"Bad Request""#));
}

#[test]
fn openapi_endpoint_uses_reusable_success_response_components() {
    let source = r#"
model Customer {
    name: string
}

route GET /customers/:name {
    return Customer::find("name", name)
}

route POST /customers {
    return Customer::create()
}

route GET /customers {
    return Customer::all()
}

route GET /customers/page {
    return Customer::page(10, 0)
}
"#;
    let data_dir = temp_data_dir("openapi_success_response_components");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r##""200":{"$ref":"#/components/responses/NexusResponse200_Customer"}"##));
    assert!(response
        .body
        .contains(r##""201":{"$ref":"#/components/responses/NexusResponse201_Customer"}"##));
    assert!(response.body.contains(
        r##""200":{"$ref":"#/components/responses/NexusResponse200_NexusList_Customer"}"##
    ));
    assert!(response.body.contains(
        r##""200":{"$ref":"#/components/responses/NexusResponse200_NexusPage_Customer"}"##
    ));
    assert!(response.body.contains(
        r##""NexusResponse200_Customer":{"description":"OK","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Customer"}}}}"##
    ));
    assert!(response.body.contains(
        r##""NexusResponse201_Customer":{"description":"Created","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Customer"}}}}"##
    ));
    assert!(response.body.contains(
        r##""NexusResponse200_NexusList_Customer":{"description":"OK","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusList_Customer"}}}}"##
    ));
    assert!(response.body.contains(
        r##""NexusResponse200_NexusPage_Customer":{"description":"OK","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusPage_Customer"}}}}"##
    ));
}

#[test]
fn openapi_1_0_contract_snapshot_covers_reusable_components() {
    let source = r#"
model Customer {
    name: string unique
    status: string = "active" index
    balance: money min 100 kz max 5000 kz
}

route GET /customers/:name ?(active: bool = true) {
    return Customer::find("name", name)
}

route POST /customers {
    return Customer::create()
}

route PUT /customers/:name/update {
    return Customer::update("name", name)
}

route GET /customers/search ?(statuses: [string]) {
    return Customer::where_in("status", statuses)
}

route GET /customers/page ?(status: string?, limit: int = 10, offset: int = 0) {
    return Customer::where_optional_page("status", status, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("openapi_1_0_contract_snapshot");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.starts_with(
        r#"{"openapi":"3.0.0","info":{"title":"NexusLang API","version":"0.1.0"},"tags":[{"name":"customers"}],"paths":{"#
    ));

    let golden_fragments = [
        r##""/customers/{name}":{"get":{"summary":"GET /customers/:name","operationId":"get_customers_by_name","tags":["customers"],"parameters":[{"$ref":"#/components/parameters/NexusPathParam_name"},{"$ref":"#/components/parameters/NexusQueryParam_active"}],"responses":{"200":{"$ref":"#/components/responses/NexusResponse200_Customer"},"400":{"description":"Bad Request","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}},"404":{"description":"Not Found","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}}}}}"##,
        r##""/customers":{"post":{"summary":"POST /customers","operationId":"post_customers","tags":["customers"],"parameters":[],"requestBody":{"$ref":"#/components/requestBodies/NexusRequestBody_Customer"},"responses":{"201":{"$ref":"#/components/responses/NexusResponse201_Customer"},"400":{"description":"Bad Request","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}},"409":{"description":"Conflict","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}}}}}"##,
        r##""/customers/{name}/update":{"put":{"summary":"PUT /customers/:name/update","operationId":"put_customers_by_name_update","tags":["customers"],"parameters":[{"$ref":"#/components/parameters/NexusPathParam_name"}],"requestBody":{"$ref":"#/components/requestBodies/NexusRequestBody_Customer"},"responses":{"200":{"$ref":"#/components/responses/NexusResponse200_Customer"},"400":{"description":"Bad Request","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}},"404":{"description":"Not Found","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}},"409":{"description":"Conflict","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}}}}}"##,
        r##""/customers/search":{"get":{"summary":"GET /customers/search","operationId":"get_customers_search","tags":["customers"],"parameters":[{"$ref":"#/components/parameters/NexusQueryParam_statuses"}],"x-nexus-in-filters":true,"responses":{"200":{"$ref":"#/components/responses/NexusResponse200_NexusList_Customer"},"400":{"description":"Bad Request","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}}}}}"##,
        r##""/customers/page":{"get":{"summary":"GET /customers/page","operationId":"get_customers_page","tags":["customers"],"parameters":[{"$ref":"#/components/parameters/NexusQueryParam_status"},{"$ref":"#/components/parameters/NexusQueryParam_limit"},{"$ref":"#/components/parameters/NexusQueryParam_offset"}],"x-nexus-pagination":true,"x-nexus-total-count":true,"x-nexus-ordering":true,"x-nexus-optional-filters":true,"responses":{"200":{"$ref":"#/components/responses/NexusResponse200_NexusPage_Customer"},"400":{"description":"Bad Request","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}}}}}"##,
        r##""Customer":{"type":"object","properties":{"name":{"type":"string","x-nexus-unique":true},"status":{"type":"string","x-nexus-index":true,"default":"active"},"balance":{"type":"object","properties":{"amount":{"type":"number"},"currency":{"type":"string"}},"required":["amount","currency"],"x-nexus-min":{"amount":100,"currency":"kz"},"x-nexus-max":{"amount":5000,"currency":"kz"}}},"required":["name","balance"]}"##,
        r##""NexusPage_Customer":{"type":"object","properties":{"total":{"type":"integer"},"items":{"type":"array","items":{"$ref":"#/components/schemas/Customer"}}},"required":["total","items"]}"##,
        r##""NexusList_Customer":{"type":"array","items":{"$ref":"#/components/schemas/Customer"}}"##,
        r##""NexusError":{"type":"object","properties":{"error":{"type":"string"}},"required":["error"]}"##,
        r##""NexusPathParam_name":{"name":"name","in":"path","required":true,"schema":{"type":"string"}}"##,
        r##""NexusQueryParam_active":{"name":"active","in":"query","required":false,"schema":{"type":"boolean","default":true}}"##,
        r##""NexusQueryParam_statuses":{"name":"statuses","in":"query","required":true,"schema":{"type":"array","items":{"type":"string"}},"style":"form","explode":false}"##,
        r##""NexusQueryParam_status":{"name":"status","in":"query","required":false,"schema":{"type":"string","nullable":true}}"##,
        r##""NexusQueryParam_limit":{"name":"limit","in":"query","required":false,"schema":{"type":"integer","default":10}}"##,
        r##""NexusQueryParam_offset":{"name":"offset","in":"query","required":false,"schema":{"type":"integer","default":0}}"##,
        r##""NexusRequestBody_Customer":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Customer"}}}}"##,
        r##""NexusResponse200_Customer":{"description":"OK","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Customer"}}}}"##,
        r##""NexusResponse201_Customer":{"description":"Created","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Customer"}}}}"##,
        r##""NexusResponse200_NexusList_Customer":{"description":"OK","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusList_Customer"}}}}"##,
        r##""NexusResponse200_NexusPage_Customer":{"description":"OK","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusPage_Customer"}}}}"##,
    ];

    for fragment in golden_fragments {
        assert!(
            response.body.contains(fragment),
            "missing OpenAPI 1.0 golden fragment:\n{}\n\nOpenAPI:\n{}",
            fragment,
            response.body
        );
    }

    assert_eq!(
        response
            .body
            .matches(r##""$ref":"#/components/requestBodies/NexusRequestBody_Customer""##)
            .count(),
        2
    );
    assert_eq!(
        response
            .body
            .matches(r##""$ref":"#/components/responses/NexusResponse200_Customer""##)
            .count(),
        2
    );
    assert_eq!(
        response
            .body
            .matches(r##""$ref":"#/components/responses/NexusResponse201_Customer""##)
            .count(),
        1
    );
    assert_eq!(
        response
            .body
            .matches(r##""$ref":"#/components/responses/NexusResponse200_NexusList_Customer""##)
            .count(),
        1
    );
    assert_eq!(
        response
            .body
            .matches(r##""$ref":"#/components/responses/NexusResponse200_NexusPage_Customer""##)
            .count(),
        1
    );
}

#[test]
fn openapi_endpoint_uses_reusable_request_body_components() {
    let source = r#"
model Customer {
    name: string
}

route POST /customers {
    return Customer::create()
}

route PUT /customers/:name {
    return Customer::update("name", name)
}
"#;
    let data_dir = temp_data_dir("openapi_request_body_components");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert_eq!(
        response
            .body
            .matches(r##""$ref":"#/components/requestBodies/NexusRequestBody_Customer""##)
            .count(),
        2
    );
    assert_eq!(
        response
            .body
            .matches(r#""NexusRequestBody_Customer":"#)
            .count(),
        1
    );
    assert!(response.body.contains(
        r##""requestBodies":{"NexusRequestBody_Customer":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Customer"}}}}"##
    ));
}

#[test]
fn openapi_endpoint_describes_unique_fields_and_conflict_response() {
    let source = r#"
model Customer {
    email: string unique
}

route POST /customers {
    return Customer::create()
}
"#;
    let data_dir = temp_data_dir("openapi_unique_constraint");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""email":{"type":"string","x-nexus-unique":true}"#));
    assert!(response.body.contains(r#""409":{"description":"Conflict""#));
}

#[test]
fn openapi_endpoint_uses_reusable_error_schema_for_error_responses() {
    let source = r#"
model Customer {
    email: string unique
}

route POST /customers {
    return Customer::create()
}

route GET /customers/:email ?(active: bool) {
    return Customer::find("email", email)
}
"#;
    let data_dir = temp_data_dir("openapi_reusable_error_schema");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(
        r#""NexusError":{"type":"object","properties":{"error":{"type":"string"}},"required":["error"]}"#
    ));
    assert!(response.body.contains(
        r##""400":{"description":"Bad Request","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}}"##
    ));
    assert!(response.body.contains(
        r##""404":{"description":"Not Found","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}}"##
    ));
    assert!(response.body.contains(
        r##""409":{"description":"Conflict","content":{"application/json":{"schema":{"$ref":"#/components/schemas/NexusError"}}}}"##
    ));
}

#[test]
fn openapi_endpoint_describes_indexed_model_fields() {
    let source = r#"
model Customer {
    email: string unique index
    status: string = "active" index
    birthday: date? index
}

route GET /customers {
    return Customer::all()
}
"#;
    let data_dir = temp_data_dir("openapi_index_constraint");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""email":{"type":"string","x-nexus-unique":true,"x-nexus-index":true}"#));
    assert!(response
        .body
        .contains(r#""status":{"type":"string","x-nexus-index":true,"default":"active"}"#));
    assert!(response.body.contains(
        r#""birthday":{"type":"string","format":"date","nullable":true,"x-nexus-index":true}"#
    ));
    assert!(!response.body.contains(r#""409":{"description":"Conflict""#));
}

#[test]
fn openapi_endpoint_describes_min_max_model_fields() {
    let source = r#"
model Product {
    name: string min 2 max 80
    stock: int min 0 max 100
    price: money min 100 kz max 5000 kz
    launch: date? min "2026-01-01" max "2026-12-31"
}

route GET /products {
    return Product::all()
}
"#;
    let data_dir = temp_data_dir("openapi_min_max_constraint");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""name":{"type":"string","minLength":2,"maxLength":80}"#));
    assert!(response
        .body
        .contains(r#""stock":{"type":"integer","minimum":0,"maximum":100}"#));
    assert!(response.body.contains(r#""price":{"type":"object","properties":{"amount":{"type":"number"},"currency":{"type":"string"}},"required":["amount","currency"],"x-nexus-min":{"amount":100,"currency":"kz"},"x-nexus-max":{"amount":5000,"currency":"kz"}}"#));
    assert!(response.body.contains(
        r#""launch":{"type":"string","format":"date","nullable":true,"x-nexus-min":"2026-01-01","x-nexus-max":"2026-12-31"}"#
    ));
}

#[test]
fn openapi_endpoint_describes_model_find_response_and_404() {
    let source = r#"
model Customer {
    name: string
}

route GET /customers/:name {
    return Customer::find("name", name)
}
"#;
    let data_dir = temp_data_dir("openapi_find_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""get":{"summary":"GET /customers/:name""#));
    assert!(response
        .body
        .contains(r##""200":{"$ref":"#/components/responses/NexusResponse200_Customer"}"##));
    assert!(response
        .body
        .contains(r#""404":{"description":"Not Found""#));
}

#[test]
fn openapi_endpoint_describes_model_where_array_response() {
    let source = r#"
model Customer {
    status: string
}

route GET /customers/status/:status {
    return Customer::where("status", status)
}
"#;
    let data_dir = temp_data_dir("openapi_where_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""get":{"summary":"GET /customers/status/:status""#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
    assert!(!response
        .body
        .contains(r#""404":{"description":"Not Found""#));
}

#[test]
fn openapi_endpoint_marks_exclusion_filtered_model_array_response() {
    let source = r#"
model Customer {
    status: string
}

route GET /customers/status/:status {
    return Customer::where_not("status", status)
}
"#;
    let data_dir = temp_data_dir("openapi_where_not_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""x-nexus-exclusion-filters":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_exclusion_total_count_page_response() {
    let source = r#"
model Customer {
    name: string
    status: string
}

route GET /customers/status ?(status: string, limit: int = 10, offset: int = 0) {
    return Customer::where_not_page("status", status, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("openapi_where_not_page_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""x-nexus-exclusion-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-total-count":true"#));
    assert!(response.body.contains(r#""x-nexus-pagination":true"#));
    assert!(response.body.contains(r#""x-nexus-ordering":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusPage_Customer"}"##));
    assert!(response.body.contains(
        r##""NexusPage_Customer":{"type":"object","properties":{"total":{"type":"integer"},"items":{"type":"array","items":{"$ref":"#/components/schemas/Customer"}}},"required":["total","items"]}"##
    ));
}

#[test]
fn openapi_endpoint_marks_composite_filtered_model_array_response() {
    let source = r#"
model Customer {
    status: string
    tenant: string
}

route GET /customers/search ?(status: string, tenant: string) {
    return Customer::where_all("status", status, "tenant", tenant)
}
"#;
    let data_dir = temp_data_dir("openapi_where_all_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""x-nexus-composite-filters":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_or_filtered_model_array_response() {
    let source = r#"
model Customer {
    status: string
    tenant: string
}

route GET /customers/search ?(status: string, tenant: string) {
    return Customer::where_any("status", status, "tenant", tenant)
}
"#;
    let data_dir = temp_data_dir("openapi_where_any_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-or-filters":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_or_total_count_page_response() {
    let source = r#"
model Customer {
    status: string
    tenant: string
}

route GET /customers/search ?(status: string, tenant: string, limit: int = 10, offset: int = 0) {
    return Customer::where_any_page("status", status, "tenant", tenant, "status", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("openapi_where_any_page_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-or-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-total-count":true"#));
    assert!(response.body.contains(r#""x-nexus-pagination":true"#));
    assert!(response.body.contains(r#""x-nexus-ordering":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusPage_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_optional_filtered_model_array_response() {
    let source = r#"
model Customer {
    status: string
}

route GET /customers/search ?(status: string?) {
    return Customer::where_optional("status", status)
}
"#;
    let data_dir = temp_data_dir("openapi_where_optional_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-optional-filters":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_in_filtered_model_array_response() {
    let source = r#"
model Customer {
    status: string
}

route GET /customers/search ?(statuses: [string]) {
    return Customer::where_in("status", statuses)
}
"#;
    let data_dir = temp_data_dir("openapi_where_in_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-in-filters":true"#));
    assert!(response.body.contains(
        r#""name":"statuses","in":"query","required":true,"schema":{"type":"array","items":{"type":"string"}},"style":"form","explode":false"#
    ));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_not_in_filtered_model_array_response() {
    let source = r#"
model Customer {
    status: string
}

route GET /customers/excluded ?(statuses: [string]) {
    return Customer::where_not_in("status", statuses)
}
"#;
    let data_dir = temp_data_dir("openapi_where_not_in_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""x-nexus-exclusion-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-in-filters":true"#));
    assert!(response.body.contains(
        r#""name":"statuses","in":"query","required":true,"schema":{"type":"array","items":{"type":"string"}},"style":"form","explode":false"#
    ));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_not_in_total_count_page_response() {
    let source = r#"
model Customer {
    status: string
}

route GET /customers/excluded ?(statuses: [string], limit: int = 10, offset: int = 0) {
    return Customer::where_not_in_page("status", statuses, "status", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("openapi_where_not_in_page_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-total-count":true"#));
    assert!(response.body.contains(r#""x-nexus-pagination":true"#));
    assert!(response.body.contains(r#""x-nexus-ordering":true"#));
    assert!(response
        .body
        .contains(r#""x-nexus-exclusion-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-in-filters":true"#));
    assert!(response.body.contains(
        r#""name":"statuses","in":"query","required":true,"schema":{"type":"array","items":{"type":"string"}},"style":"form","explode":false"#
    ));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusPage_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_optional_not_in_filtered_model_array_response() {
    let source = r#"
model Customer {
    status: string
}

route GET /customers/excluded ?(statuses: [string]?) {
    return Customer::where_not_in_optional("status", statuses)
}
"#;
    let data_dir = temp_data_dir("openapi_where_not_in_optional_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""x-nexus-exclusion-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-in-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-optional-filters":true"#));
    assert!(response.body.contains(
        r#""name":"statuses","in":"query","required":false,"schema":{"type":"array","items":{"type":"string"},"nullable":true},"style":"form","explode":false"#
    ));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_optional_not_in_total_count_page_response() {
    let source = r#"
model Customer {
    status: string
}

route GET /customers/excluded ?(statuses: [string]?, limit: int = 10, offset: int = 0) {
    return Customer::where_not_in_optional_page("status", statuses, "status", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("openapi_where_not_in_optional_page_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-total-count":true"#));
    assert!(response.body.contains(r#""x-nexus-pagination":true"#));
    assert!(response.body.contains(r#""x-nexus-ordering":true"#));
    assert!(response
        .body
        .contains(r#""x-nexus-exclusion-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-in-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-optional-filters":true"#));
    assert!(response.body.contains(
        r#""name":"statuses","in":"query","required":false,"schema":{"type":"array","items":{"type":"string"},"nullable":true},"style":"form","explode":false"#
    ));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusPage_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_optional_in_filtered_model_array_response() {
    let source = r#"
model Customer {
    status: string
}

route GET /customers/search ?(statuses: [string]?) {
    return Customer::where_in_optional("status", statuses)
}
"#;
    let data_dir = temp_data_dir("openapi_where_in_optional_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-in-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-optional-filters":true"#));
    assert!(response.body.contains(
        r#""name":"statuses","in":"query","required":false,"schema":{"type":"array","items":{"type":"string"},"nullable":true},"style":"form","explode":false"#
    ));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_optional_in_total_count_page_response() {
    let source = r#"
model Customer {
    status: string
}

route GET /customers/search ?(statuses: [string]?, limit: int = 10, offset: int = 0) {
    return Customer::where_in_optional_page("status", statuses, "status", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("openapi_where_in_optional_page_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-total-count":true"#));
    assert!(response.body.contains(r#""x-nexus-pagination":true"#));
    assert!(response.body.contains(r#""x-nexus-ordering":true"#));
    assert!(response.body.contains(r#""x-nexus-in-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-optional-filters":true"#));
    assert!(response.body.contains(
        r#""name":"statuses","in":"query","required":false,"schema":{"type":"array","items":{"type":"string"},"nullable":true},"style":"form","explode":false"#
    ));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusPage_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_comparison_filtered_model_array_response() {
    let source = r#"
model Customer {
    balance: float
}

route GET /customers/search ?(min: float) {
    return Customer::where_compare("balance", ">=", min)
}
"#;
    let data_dir = temp_data_dir("openapi_where_compare_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""x-nexus-comparison-filters":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_text_filtered_model_array_response() {
    let source = r#"
model Customer {
    name: string
}

route GET /customers/search ?(term: string) {
    return Customer::where_text("name", "contains", term)
}
"#;
    let data_dir = temp_data_dir("openapi_where_text_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-text-filters":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_range_filtered_model_array_response() {
    let source = r#"
model Customer {
    balance: float
}

route GET /customers/range ?(min: float, max: float) {
    return Customer::where_between("balance", min, max)
}
"#;
    let data_dir = temp_data_dir("openapi_where_between_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-range-filters":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_paginated_model_array_response() {
    let source = r#"
model Customer {
    status: string
}

route GET /customers/status/:status/page {
    return Customer::where("status", status, 10, 20)
}
"#;
    let data_dir = temp_data_dir("openapi_pagination");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-pagination":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_total_count_page_response() {
    let source = r#"
model Customer {
    name: string
    status: string
}

route GET /customers/status/:status/page {
    return Customer::where_page("status", status, "name", "asc", 10, 0)
}
"#;
    let data_dir = temp_data_dir("openapi_total_count_page");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-total-count":true"#));
    assert!(response.body.contains(r#""x-nexus-pagination":true"#));
    assert!(response.body.contains(r#""x-nexus-ordering":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusPage_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_advanced_total_count_page_responses() {
    let source = r#"
model Customer {
    name: string
    status: string
    tenant: string
    balance: float
}

route GET /customers/optional ?(status: string?, limit: int = 10, offset: int = 0) {
    return Customer::where_optional_page("status", status, "name", "asc", limit, offset)
}

route GET /customers/includes ?(statuses: [string], limit: int = 10, offset: int = 0) {
    return Customer::where_in_page("status", statuses, "name", "asc", limit, offset)
}

route GET /customers/compare ?(min: float, limit: int = 10, offset: int = 0) {
    return Customer::where_compare_page("balance", ">=", min, "name", "asc", limit, offset)
}

route GET /customers/text ?(term: string, limit: int = 10, offset: int = 0) {
    return Customer::where_text_page("name", "contains", term, "name", "asc", limit, offset)
}

route GET /customers/range ?(min: float, max: float, limit: int = 10, offset: int = 0) {
    return Customer::where_between_page("balance", min, max, "name", "asc", limit, offset)
}

route GET /customers/all ?(status: string, tenant: string, limit: int = 10, offset: int = 0) {
    return Customer::where_all_page("status", status, "tenant", tenant, "name", "asc", limit, offset)
}
"#;
    let data_dir = temp_data_dir("openapi_advanced_total_count_page");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-total-count":true"#));
    assert!(response.body.contains(r#""x-nexus-pagination":true"#));
    assert!(response.body.contains(r#""x-nexus-ordering":true"#));
    assert!(response.body.contains(r#""x-nexus-optional-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-in-filters":true"#));
    assert!(response
        .body
        .contains(r#""x-nexus-comparison-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-text-filters":true"#));
    assert!(response.body.contains(r#""x-nexus-range-filters":true"#));
    assert!(response
        .body
        .contains(r#""x-nexus-composite-filters":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusPage_Customer"}"##));
}

#[test]
fn openapi_endpoint_marks_ordered_paginated_model_array_response() {
    let source = r#"
model Customer {
    name: string
}

route GET /customers/order {
    return Customer::all("name", "asc", 10, 0)
}
"#;
    let data_dir = temp_data_dir("openapi_ordering");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""x-nexus-ordering":true"#));
    assert!(response.body.contains(r#""x-nexus-pagination":true"#));
    assert!(response
        .body
        .contains(r##""schema":{"$ref":"#/components/schemas/NexusList_Customer"}"##));
}

#[test]
fn openapi_endpoint_describes_typed_query_params() {
    let source = r#"
model Customer {
    name: string
    active: bool
}

route GET /customers/:tenant ?(limit: int, active: bool) {
    return Customer::all(limit, 0)
}
"#;
    let data_dir = temp_data_dir("openapi_query_params");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""name":"tenant","in":"path","required":true,"schema":{"type":"string"}"#));
    assert!(response
        .body
        .contains(r#""name":"limit","in":"query","required":true,"schema":{"type":"integer"}"#));
    assert!(response
        .body
        .contains(r#""name":"active","in":"query","required":true,"schema":{"type":"boolean"}"#));
    assert!(response
        .body
        .contains(r#""400":{"description":"Bad Request""#));
}

#[test]
fn openapi_endpoint_uses_reusable_parameter_components() {
    let source = r#"
route GET /customers/:tenant ?(limit: int, active: bool) {
    return tenant
}

route GET /orders/:tenant ?(limit: int, tags: [string]) {
    return tags
}

route GET /customers/defaults ?(limit: int = 20) {
    return "defaults"
}
"#;
    let data_dir = temp_data_dir("openapi_parameter_components");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(
        r##""parameters":[{"$ref":"#/components/parameters/NexusPathParam_tenant"},{"$ref":"#/components/parameters/NexusQueryParam_limit"},{"$ref":"#/components/parameters/NexusQueryParam_active"}]"##
    ));
    assert_eq!(
        response
            .body
            .matches(r##""$ref":"#/components/parameters/NexusPathParam_tenant""##)
            .count(),
        2
    );
    assert_eq!(
        response
            .body
            .matches(r##""$ref":"#/components/parameters/NexusQueryParam_limit""##)
            .count(),
        2
    );
    assert!(response.body.contains(
        r#""NexusPathParam_tenant":{"name":"tenant","in":"path","required":true,"schema":{"type":"string"}}"#
    ));
    assert!(response.body.contains(
        r#""NexusQueryParam_limit":{"name":"limit","in":"query","required":true,"schema":{"type":"integer"}}"#
    ));
    assert!(response.body.contains(
        r#""NexusQueryParam_active":{"name":"active","in":"query","required":true,"schema":{"type":"boolean"}}"#
    ));
    assert!(response.body.contains(
        r#""NexusQueryParam_tags":{"name":"tags","in":"query","required":true,"schema":{"type":"array","items":{"type":"string"}},"style":"form","explode":false}"#
    ));
    assert!(response.body.contains(
        r#""NexusQueryParam_limit_2":{"name":"limit","in":"query","required":false,"schema":{"type":"integer","default":20}}"#
    ));
}

#[test]
fn openapi_endpoint_describes_money_query_params() {
    let source = r#"
route GET /payments ?(amount: money, maybe_amount: money?, default_amount: money = 1000 kz) {
    return amount
}
"#;
    let data_dir = temp_data_dir("openapi_money_query_params");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(
        r#""name":"amount","in":"query","required":true,"schema":{"type":"string","format":"nexus-money","example":"1000:kz"}"#
    ));
    assert!(response.body.contains(
        r#""name":"maybe_amount","in":"query","required":false,"schema":{"type":"string","format":"nexus-money","example":"1000:kz","nullable":true}"#
    ));
    assert!(response.body.contains(
        r#""name":"default_amount","in":"query","required":false,"schema":{"type":"string","format":"nexus-money","example":"1000:kz","default":"1000:kz"}"#
    ));
}

#[test]
fn openapi_endpoint_describes_array_query_params() {
    let source = r#"
route GET /search ?(tags: [string], maybe_tags: [string]?, amounts: [money] = [1000 kz]) {
    return tags
}
"#;
    let data_dir = temp_data_dir("openapi_array_query_params");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(
        r#""name":"tags","in":"query","required":true,"schema":{"type":"array","items":{"type":"string"}},"style":"form","explode":false"#
    ));
    assert!(response.body.contains(
        r#""name":"maybe_tags","in":"query","required":false,"schema":{"type":"array","items":{"type":"string"},"nullable":true},"style":"form","explode":false"#
    ));
    assert!(response.body.contains(
        r#""name":"amounts","in":"query","required":false,"schema":{"type":"array","items":{"type":"string","format":"nexus-money","example":"1000:kz"},"default":["1000:kz"]},"style":"form","explode":false"#
    ));
}

#[test]
fn openapi_endpoint_describes_optional_and_default_query_params() {
    let source = r#"
model Customer {
    name: string
}

route GET /customers ?(limit: int = 20, status: string?) {
    return Customer::all(limit, 0)
}
"#;
    let data_dir = temp_data_dir("openapi_optional_query_params");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(
        r#""name":"limit","in":"query","required":false,"schema":{"type":"integer","default":20}"#
    ));
    assert!(response.body.contains(
        r#""name":"status","in":"query","required":false,"schema":{"type":"string","nullable":true}"#
    ));
    assert!(response
        .body
        .contains(r#""400":{"description":"Bad Request""#));
}

#[test]
fn openapi_endpoint_describes_model_update_request_body_and_404() {
    let source = r#"
model Customer {
    name: string
}

route PUT /customers/:name {
    return Customer::update("name", name)
}
"#;
    let data_dir = temp_data_dir("openapi_update_request_body");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""put":{"summary":"PUT /customers/:name""#));
    assert!(response.body.contains(
        r##""requestBody":{"$ref":"#/components/requestBodies/NexusRequestBody_Customer"}"##
    ));
    assert!(
        response
            .body
            .contains(r##""NexusRequestBody_Customer":{"required":true,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/Customer"}}}}"##)
    );
    assert!(response
        .body
        .contains(r##""200":{"$ref":"#/components/responses/NexusResponse200_Customer"}"##));
    assert!(response
        .body
        .contains(r#""400":{"description":"Bad Request""#));
    assert!(response
        .body
        .contains(r#""404":{"description":"Not Found""#));
}

#[test]
fn openapi_endpoint_describes_model_delete_response_and_404() {
    let source = r#"
model Customer {
    name: string
}

route DELETE /customers/:name {
    return Customer::delete("name", name)
}
"#;
    let data_dir = temp_data_dir("openapi_delete_response");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response
        .body
        .contains(r#""delete":{"summary":"DELETE /customers/:name""#));
    assert!(response
        .body
        .contains(r##""200":{"$ref":"#/components/responses/NexusResponse200_Customer"}"##));
    assert!(response
        .body
        .contains(r#""404":{"description":"Not Found""#));
    assert!(!response.body.contains(r#""requestBody""#));
}

#[test]
fn missing_http_route_returns_404() {
    let source = r#"
route GET /employees {
    return "ok"
}
"#;
    let data_dir = temp_data_dir("missing_route");
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(source, "GET", "/missing", &storage).unwrap();

    assert_eq!(response.status, 404);
}

fn temp_data_dir(name: &str) -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("nexuslang_test_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[derive(Clone, Copy)]
enum ParityBackend {
    Json,
    Sqlite,
}

impl ParityBackend {
    fn label(self) -> &'static str {
        match self {
            ParityBackend::Json => "json",
            ParityBackend::Sqlite => "sqlite",
        }
    }
}

struct ParityRequest {
    label: &'static str,
    method: &'static str,
    path: &'static str,
    body: Option<&'static str>,
}

#[derive(Debug, PartialEq, Eq)]
struct RecordedResponse {
    label: &'static str,
    status: u16,
    body: String,
}

fn run_parity_requests(
    test_name: &str,
    source: &str,
    backend: ParityBackend,
    requests: &[ParityRequest],
) -> Vec<RecordedResponse> {
    let storage = parity_storage(test_name, backend);
    run_requests_with_storage(source, &storage, requests)
}

fn run_requests_with_storage(
    source: &str,
    storage: &nexuslang::server::Storage,
    requests: &[ParityRequest],
) -> Vec<RecordedResponse> {
    requests
        .iter()
        .map(|request| {
            let response = match request.body {
                Some(body) => nexuslang::server::handle_request_with_body_for_test(
                    source,
                    request.method,
                    request.path,
                    body,
                    storage,
                ),
                None => nexuslang::server::handle_request_for_test(
                    source,
                    request.method,
                    request.path,
                    storage,
                ),
            }
            .unwrap_or_else(|err| panic!("request '{}' failed: {}", request.label, err));

            RecordedResponse {
                label: request.label,
                status: response.status,
                body: response.body,
            }
        })
        .collect()
}

fn parity_storage(test_name: &str, backend: ParityBackend) -> nexuslang::server::Storage {
    let dir_name = format!("{}_{}", test_name, backend.label());
    let data_dir = temp_data_dir(&dir_name);
    fs::create_dir_all(&data_dir).unwrap();
    match backend {
        ParityBackend::Json => nexuslang::server::Storage::new_json(&data_dir),
        ParityBackend::Sqlite => {
            nexuslang::server::Storage::new_sqlite(&data_dir.join("nexus.db")).unwrap()
        }
    }
}

fn assert_recorded_response(responses: &[RecordedResponse], label: &str, status: u16, body: &str) {
    let response = responses
        .iter()
        .find(|response| response.label == label)
        .unwrap_or_else(|| panic!("response '{}' not recorded", label));
    assert_eq!(response.status, status, "unexpected status for '{}'", label);
    assert_eq!(response.body, body, "unexpected body for '{}'", label);
}

fn assert_recorded_response_contains(
    responses: &[RecordedResponse],
    label: &str,
    status: u16,
    body_fragment: &str,
) {
    let response = responses
        .iter()
        .find(|response| response.label == label)
        .unwrap_or_else(|| panic!("response '{}' not recorded", label));
    assert_eq!(response.status, status, "unexpected status for '{}'", label);
    assert!(
        response.body.contains(body_fragment),
        "body for '{}' did not contain '{}': {}",
        label,
        body_fragment,
        response.body
    );
}
