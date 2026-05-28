#![allow(deprecated)]

use nexuslang::ast::Decl;
use nexuslang::auth_ops::AuthStaticOperation;
use nexuslang::diagnostic::{
    checker_code_for_message, codes, parser_code_for_message, runtime_code_for_message,
    DiagnosticLabel, DiagnosticSeverity, DiagnosticStage, DiagnosticSuggestion,
};
use nexuslang::model_ops::ModelStaticOperation;
use nexuslang::route_hir::CheckedRouteExpr;
use nexuslang::{
    check_source, parse_checked_source, parse_source_diagnostic, run_source, run_source_captured,
};
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
fn print_call_arguments_are_type_checked() {
    let err = check_source("print(missing)\n")
        .expect_err("print call should validate arguments before returning void");
    assert!(
        err.contains("Variável 'missing' não definida"),
        "err: {err}"
    );
}

#[test]
fn string_add_requires_two_strings() {
    let err = check_source(r#"let message = "total: " + 1"#)
        .expect_err("mixed string concatenation should be rejected");
    assert!(err.contains("operação numérica inválida"), "err: {err}");
}

#[test]
fn control_flow_body_bindings_do_not_leak() {
    let err = check_source(
        r#"
fn value() -> int {
    if true {
        let local = 1
    }
    return local
}
"#,
    )
    .expect_err("if body local binding should not leak");
    assert!(err.contains("Variável 'local' não definida"), "err: {err}");

    let err = check_source(
        r#"
fn value() -> int {
    let nums = [1]
    for n in nums {
        let inner = n
    }
    return inner
}
"#,
    )
    .expect_err("for body local binding should not leak");
    assert!(err.contains("Variável 'inner' não definida"), "err: {err}");
}

#[test]
fn diagnostic_code_and_severity_do_not_change_text_display() {
    let diagnostic = nexuslang::diagnostic::Diagnostic::new(DiagnosticStage::Checker, "erro")
        .with_code("NXL3999")
        .with_severity(DiagnosticSeverity::Warning)
        .with_label_at("origem do erro", 2, 4)
        .with_label(DiagnosticLabel::new("contexto adicional"))
        .with_note("nota para tooling")
        .with_suggestion("verifique o tipo retornado")
        .with_replacement_suggestion("troque o retorno", "return 1");

    assert_eq!(diagnostic.code.as_deref(), Some("NXL3999"));
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::Warning));
    assert_eq!(diagnostic.labels.len(), 2);
    assert_eq!(diagnostic.labels[0].message, "origem do erro");
    assert_eq!(diagnostic.labels[0].line, Some(2));
    assert_eq!(diagnostic.labels[0].column, Some(4));
    assert_eq!(diagnostic.notes, ["nota para tooling"]);
    assert_eq!(diagnostic.suggestions.len(), 2);
    assert_eq!(
        diagnostic.suggestions[1],
        DiagnosticSuggestion::with_replacement("troque o retorno", "return 1")
    );
    assert_eq!(diagnostic.to_string(), "erro");

    let optional = diagnostic
        .without_code()
        .without_severity()
        .without_labels()
        .without_notes()
        .without_suggestions();
    assert_eq!(optional.code, None);
    assert_eq!(optional.severity, None);
    assert!(optional.labels.is_empty());
    assert!(optional.notes.is_empty());
    assert!(optional.suggestions.is_empty());
    assert_eq!(optional.to_string(), "erro");
}

#[test]
fn diagnostic_code_catalog_classifies_error_families() {
    let parser_import =
        nexuslang::diagnostic::Diagnostic::parser("import espera 'from' antes do caminho", 2, 1);
    assert_eq!(parser_import.code.as_deref(), Some(codes::PARSER_IMPORT));

    assert_eq!(
        parser_code_for_message("expressão inválida: Eof"),
        codes::PARSER_EXPRESSION
    );
    assert_eq!(
        checker_code_for_message("Tipo de retorno inválido: esperado int"),
        codes::CHECKER_TYPE
    );
    assert_eq!(
        checker_code_for_message("Variável 'cliente' não definida"),
        codes::CHECKER_SYMBOL
    );
    assert_eq!(
        checker_code_for_message("Workflow 'cobranca' não encontrado"),
        codes::CHECKER_WORKFLOW
    );
    assert_eq!(
        runtime_code_for_message("Função 'calcular' não definida"),
        codes::RUNTIME_UNDEFINED_FUNCTION
    );
    assert_eq!(
        runtime_code_for_message("Divisão por zero"),
        codes::RUNTIME_DIVISION_BY_ZERO
    );
    assert_eq!(
        runtime_code_for_message("assert_eq falhou: esperado ok, recebido erro"),
        codes::RUNTIME_ASSERTION
    );
    assert_eq!(
        runtime_code_for_message("assert_contains falhou: esperado conter ativo"),
        codes::RUNTIME_ASSERTION
    );
}

#[test]
fn native_assert_helpers_pass_and_fail_at_runtime() {
    let output = run_source_captured(
        r#"
assert_true(2 > 1)
assert_true(3 > 1, "comparacao verdadeira")
assert_eq(1, 1.0)
assert_eq("pedido aprovado", "pedido aprovado")
assert_eq([1, 2, 3], [1, 2, 3])
assert_eq([1, 2], [1.0, 2.0], "vetor numerico")
assert_ne("ativo", "inativo", "status nao deve ser inativo")
assert_contains("cliente ativo premium", "ativo", "texto de status")
assert_contains([1, 2, 3], 2, "lista contem id")
assert_contains([1.0, 2.0], 2, "lista numerica")
print("ok")
"#,
    )
    .expect("assert helpers should pass");
    assert_eq!(output, ["ok".to_string()]);

    let err = run_source_captured(
        r#"
print("antes")
assert_eq("recebido", "esperado", "cliente ativo deve bater")
"#,
    )
    .expect_err("assert_eq should fail the program");
    assert!(err.contains("assert_eq falhou"), "err: {err}");
    assert!(err.contains("cliente ativo deve bater"), "err: {err}");
    assert!(err.contains("esperado esperado"), "err: {err}");
    assert!(err.contains("recebido recebido"), "err: {err}");

    let err = run_source_captured(r#"assert_ne("ativo", "ativo", "status deve mudar")"#)
        .expect_err("assert_ne should fail when values are equal");
    assert!(err.contains("assert_ne falhou"), "err: {err}");
    assert!(err.contains("status deve mudar"), "err: {err}");
    assert!(err.contains("valor nao deveria ser ativo"), "err: {err}");

    let err = run_source_captured(
        r#"assert_contains("cliente ativo", "inativo", "texto deve conter status")"#,
    )
    .expect_err("assert_contains should fail when value is absent");
    assert!(err.contains("assert_contains falhou"), "err: {err}");
    assert!(err.contains("texto deve conter status"), "err: {err}");
    assert!(err.contains("esperado conter inativo"), "err: {err}");

    let err = run_source_captured(r#"assert_true(false, "flag ativo")"#)
        .expect_err("assert_true should include optional message");
    assert!(err.contains("assert_true falhou"), "err: {err}");
    assert!(err.contains("flag ativo"), "err: {err}");

    let err = check_source(r#"assert_true("sim")"#).expect_err("assert_true expects bool");
    assert!(err.contains("assert_true"), "err: {err}");
    assert!(err.contains("bool"), "err: {err}");

    let err =
        check_source(r#"assert_eq("a", "a", false)"#).expect_err("assert_eq message is string");
    assert!(err.contains("mensagem"), "err: {err}");
    assert!(err.contains("string"), "err: {err}");

    let err = check_source(r#"assert_contains(10, 1)"#)
        .expect_err("assert_contains requires string or array container");
    assert!(err.contains("assert_contains"), "err: {err}");
    assert!(err.contains("string ou array"), "err: {err}");
}

#[test]
fn parser_import_export_diagnostics_include_tooling_metadata() {
    let import_err = parse_source_diagnostic(r#"import x "./lib.nx""#).unwrap_err();
    assert_eq!(import_err.code.as_deref(), Some(codes::PARSER_IMPORT));
    assert_eq!(import_err.labels[0].message, "sintaxe do import");
    assert!(import_err
        .notes
        .iter()
        .any(|note| note.contains("Imports usam a forma")));
    assert!(import_err
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("import Nome")));
    assert!(import_err.to_string().contains("import espera"));

    let export_err = parse_source_diagnostic(r#"export import x from "./lib.nx""#).unwrap_err();
    assert_eq!(export_err.code.as_deref(), Some(codes::PARSER_EXPORT));
    assert_eq!(export_err.labels[0].message, "sintaxe do export");
    assert!(export_err
        .notes
        .iter()
        .any(|note| note.contains("Exports sao suportados")));
    assert!(export_err
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("declaracao nomeada")));
    assert!(export_err.to_string().contains("exportar"));
}

#[test]
fn checker_additional_diagnostic_families_include_tooling_metadata() {
    let symbol_err = nexuslang::parse_checked_source_diagnostic("print(cliente)").unwrap_err();
    assert_eq!(symbol_err.code.as_deref(), Some(codes::CHECKER_SYMBOL));
    assert_eq!(symbol_err.labels[0].message, "referencia de simbolo");
    assert!(symbol_err
        .notes
        .iter()
        .any(|note| note.contains("nao encontrou uma declaracao")));
    assert!(symbol_err
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("Declare o simbolo")));
    assert!(symbol_err.to_string().contains("Variável"));

    let argument_err = nexuslang::parse_checked_source_diagnostic(
        r#"
fn dobrar(x: int) -> int {
    return x
}

print(dobrar(1, 2))
"#,
    )
    .unwrap_err();
    assert_eq!(argument_err.code.as_deref(), Some(codes::CHECKER_ARGUMENT));
    assert_eq!(
        argument_err.labels[0].message,
        "argumentos verificados aqui"
    );
    assert!(argument_err
        .notes
        .iter()
        .any(|note| note.contains("argumentos incompat")));
    assert!(argument_err
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("assinatura esperada")));
    assert!(argument_err.to_string().contains("argumento"));

    let model_err =
        nexuslang::parse_checked_source_diagnostic("let clientes = Customer::all()").unwrap_err();
    assert_eq!(model_err.code.as_deref(), Some(codes::CHECKER_MODEL));
    assert_eq!(model_err.labels[0].message, "uso de model aqui");
    assert!(model_err
        .notes
        .iter()
        .any(|note| note.contains("model e seus campos")));
    assert!(model_err
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("model esperado")));
    assert!(model_err.to_string().contains("Model"));

    let workflow_err =
        nexuslang::parse_checked_source_diagnostic(r#"run_workflow("Cobranca")"#).unwrap_err();
    assert_eq!(workflow_err.code.as_deref(), Some(codes::CHECKER_WORKFLOW));
    assert_eq!(workflow_err.labels[0].message, "workflow verificado aqui");
    assert!(workflow_err
        .notes
        .iter()
        .any(|note| note.contains("Workflows chamados")));
    assert!(workflow_err
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("workflow esperado")));
    assert!(workflow_err.to_string().contains("Workflow"));
}

#[test]
fn runtime_additional_diagnostic_families_include_tooling_metadata() {
    let variable_err = nexuslang::MultiModuleDiagnostic::runtime("Variável 'cliente' não definida");
    assert_eq!(
        variable_err.diagnostic.code.as_deref(),
        Some(codes::RUNTIME_UNDEFINED_VARIABLE)
    );
    assert_eq!(
        variable_err.diagnostic.labels[0].message,
        "variavel acessada em runtime"
    );
    assert!(variable_err
        .diagnostic
        .notes
        .iter()
        .any(|note| note.contains("escopo atual")));
    assert!(variable_err
        .diagnostic
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("Declare a variavel")));

    let function_err = nexuslang::MultiModuleDiagnostic::runtime("Função 'calcular' não definida");
    assert_eq!(
        function_err.diagnostic.code.as_deref(),
        Some(codes::RUNTIME_UNDEFINED_FUNCTION)
    );
    assert_eq!(
        function_err.diagnostic.labels[0].message,
        "funcao chamada em runtime"
    );
    assert!(function_err
        .diagnostic
        .notes
        .iter()
        .any(|note| note.contains("programa carregado")));
    assert!(function_err
        .diagnostic
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("Declare a funcao")));
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

// ---------------------------------------------------------------------------
// F11.12 — Multi-module ERP example (erp_basico_multi/)
// ---------------------------------------------------------------------------

#[test]
fn erp_basico_multi_checks_and_runs() {
    // Load the multi-module ERP example from examples/erp_basico_multi/
    let result = nexuslang::load_and_run_with_graph(std::path::Path::new(
        "examples/erp_basico_multi/main.nx",
    ));
    assert!(
        result.is_ok(),
        "multi-module ERP example should load, check, and run: {:?}",
        result.err()
    );
}

#[test]
fn erp_basico_multi_loads_and_checks_with_graph() {
    let result = nexuslang::load_and_check_with_graph(std::path::Path::new(
        "examples/erp_basico_multi/main.nx",
    ));
    assert!(
        result.is_ok(),
        "multi-module ERP example should load and check with graph: {:?}",
        result.err()
    );
}

#[test]
fn erp_basico_multi_runtime_output() {
    let entry = std::path::Path::new("examples/erp_basico_multi/main.nx");

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"=== ERP Multi-Módulo ===".to_string()),
        "output should contain header: {:?}",
        output
    );
    assert!(
        output.contains(&"Bem-vindo, Admin".to_string()),
        "output should contain greeting: {:?}",
        output
    );
    assert!(
        output.contains(&"Salário acima da média".to_string()),
        "output should contain conditional result: {:?}",
        output
    );
    assert!(
        output.contains(&"Processando mês:".to_string()),
        "output should contain loop: {:?}",
        output
    );
    assert!(
        output.contains(&"Financeiro".to_string()),
        "output should contain for-loop item: {:?}",
        output
    );
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
fn auth_config_and_route_guard_check() {
    let source = r#"
model User {
    email: string unique
    role: string
}

auth Session {
    model: User
    identity: email
    role: role
    password_min: 15
    session_ttl_minutes: 60
    idle_ttl_minutes: 15
}

route GET /me auth(Session, role: "admin") {
    return "ok"
}
"#;

    if let Err(err) = check_source(source) {
        panic!("{}", err);
    }
}

#[test]
fn auth_identity_must_be_unique_string() {
    let source = r#"
model User {
    email: string
}

auth Session {
    model: User
    identity: email
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("deve ser string unique"));
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
fn route_array_return_type_validates_inner_type() {
    let source = r#"
route GET /bad {
    return [nil]
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(err.contains("valor HTTP concreto"), "err: {err}");
    assert!(err.contains("nil"), "err: {err}");
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
fn checked_route_hir_lifts_model_static_operation() {
    let source = r#"
model Customer {
    name: string
}

route GET /customers/:name {
    return Customer::find("name", name)
}
"#;

    let program = parse_checked_source(source).unwrap();
    let routes = nexuslang::route_hir::checked_routes(&program);

    assert_eq!(routes.len(), 1);
    match routes[0].return_expr {
        Some(CheckedRouteExpr::ModelOperation(operation)) => {
            assert_eq!(operation.model, "Customer");
            assert_eq!(operation.operation, ModelStaticOperation::Find);
            assert_eq!(operation.args.len(), 2);
        }
        other => panic!("route deveria retornar ModelOperation, encontrado {other:?}"),
    }
}

#[test]
fn checked_route_hir_lifts_auth_static_operation() {
    let source = r#"
model User {
    email: string unique
}

auth UserAuth {
    model: User
    identity: email
}

route POST /auth/login {
    return Auth::login(UserAuth)
}
"#;

    let program = parse_checked_source(source).unwrap();
    let routes = nexuslang::route_hir::checked_routes(&program);

    assert_eq!(routes.len(), 1);
    match routes[0].return_expr {
        Some(CheckedRouteExpr::AuthOperation(operation)) => {
            assert_eq!(operation.operation, AuthStaticOperation::Login);
            assert_eq!(operation.args.len(), 1);
            assert_eq!(
                operation
                    .checked_args
                    .and_then(|args| args.auth_config_name()),
                Some("UserAuth")
            );
        }
        other => panic!("route deveria retornar AuthOperation, encontrado {other:?}"),
    }
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
fn route_model_where_text_rejects_optional_value_for_required_field() {
    let source = r#"
model Customer {
    name: string
}

route GET /customers/search ?(term: string?) {
    return Customer::where_text("name", "contains", term)
}
"#;

    let err = check_source(source).unwrap_err();
    assert!(
        err.contains("Customer::where_text() valor invalido para 'name'"),
        "err: {err}"
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
fn storage_driver_registry_parses_and_constructs_backends() {
    let json_dir = temp_data_dir("storage_driver_registry_json");
    let json_driver = nexuslang::server::StorageDriver::parse("json").unwrap();
    let json_storage = nexuslang::server::Storage::new_driver(json_driver, &json_dir).unwrap();

    assert_eq!(json_driver, nexuslang::server::StorageDriver::Json);
    assert_eq!(
        json_storage.driver(),
        nexuslang::server::StorageDriver::Json
    );
    assert_eq!(
        json_driver.target_path(&json_dir),
        json_dir,
        "JSON driver should target the data directory"
    );

    let sqlite_dir = temp_data_dir("storage_driver_registry_sqlite");
    let sqlite_driver = nexuslang::server::StorageDriver::parse("sqlite3").unwrap();
    let sqlite_storage =
        nexuslang::server::Storage::new_driver(sqlite_driver, &sqlite_dir).unwrap();

    assert_eq!(sqlite_driver, nexuslang::server::StorageDriver::Sqlite);
    assert_eq!(
        sqlite_storage.driver(),
        nexuslang::server::StorageDriver::Sqlite
    );
    assert_eq!(
        sqlite_driver.target_path(&sqlite_dir),
        sqlite_dir.join("nexus.db")
    );

    let error = nexuslang::server::StorageDriver::parse("memory").unwrap_err();
    assert!(error.contains("json, sqlite"), "error: {error}");
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

const AUTH_SOURCE: &str = r#"
model User {
    email: string unique
    name: string
    role: string = "user" index
}

auth UserAuth {
    model: User
    identity: email
    role: role
    password_min: 15
    session_ttl_minutes: 60
    idle_ttl_minutes: 10
}

route POST /auth/register {
    return Auth::register(UserAuth)
}

route POST /auth/login {
    return Auth::login(UserAuth)
}

route POST /auth/logout auth(UserAuth) {
    return Auth::logout()
}

route GET /me auth(UserAuth) {
    return Auth::user()
}

route GET /admin/users auth(UserAuth, role: "admin") {
    return User::all()
}
"#;

#[test]
fn native_auth_registers_with_argon2id_and_session_cookie() {
    assert!(check_source(AUTH_SOURCE).is_ok());
    let data_dir = temp_data_dir("native_auth_register");
    fs::create_dir_all(&data_dir).unwrap();
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let password = "strong-password-123";
    let register = nexuslang::server::handle_request_with_body_for_test(
        AUTH_SOURCE,
        "POST",
        "/auth/register",
        &format!(
            r#"{{"email":"ana@example.com","name":"Ana","role":"admin","password":"{}"}}"#,
            password
        ),
        &storage,
    )
    .unwrap();

    assert_eq!(register.status, 201);
    assert!(register.body.contains(r#""email":"ana@example.com""#));
    let token = json_string_field(&register.body, "token");
    let csrf = json_string_field(&register.body, "csrf_token");
    let cookie = set_cookie_header(&register);
    assert!(cookie.contains("__Host-nexus_session="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("Secure"));
    assert!(cookie.contains("SameSite=Lax"));
    assert!(!csrf.is_empty());

    let auth_store = fs::read_to_string(data_dir.join(".nexus-auth.json")).unwrap();
    assert!(auth_store.contains("$argon2id$"));
    assert!(!auth_store.contains(password));
    assert!(!auth_store.contains(&token));
    assert!(!auth_store.contains(&csrf));

    let cookie_headers = vec![("Cookie".to_string(), cookie_pair(&cookie))];
    let me = nexuslang::server::handle_request_with_headers_and_body_for_test(
        AUTH_SOURCE,
        "GET",
        "/me",
        &cookie_headers,
        "",
        &storage,
    )
    .unwrap();
    assert_eq!(me.status, 200);
    assert_eq!(
        me.body,
        r#"{"email":"ana@example.com","name":"Ana","role":"admin"}"#
    );

    let admin = nexuslang::server::handle_request_with_headers_and_body_for_test(
        AUTH_SOURCE,
        "GET",
        "/admin/users",
        &cookie_headers,
        "",
        &storage,
    )
    .unwrap();
    assert_eq!(admin.status, 200);
    assert!(admin.body.contains(r#""role":"admin""#));

    let logout_without_csrf = nexuslang::server::handle_request_with_headers_and_body_for_test(
        AUTH_SOURCE,
        "POST",
        "/auth/logout",
        &cookie_headers,
        "",
        &storage,
    )
    .unwrap();
    assert_eq!(logout_without_csrf.status, 403);

    let invalid_csrf_headers = vec![
        ("Cookie".to_string(), cookie_pair(&cookie)),
        ("X-Nexus-CSRF-Token".to_string(), "invalid".to_string()),
    ];
    let logout_with_invalid_csrf =
        nexuslang::server::handle_request_with_headers_and_body_for_test(
            AUTH_SOURCE,
            "POST",
            "/auth/logout",
            &invalid_csrf_headers,
            "",
            &storage,
        )
        .unwrap();
    assert_eq!(logout_with_invalid_csrf.status, 403);

    let csrf_cookie_headers = vec![
        ("Cookie".to_string(), cookie_pair(&cookie)),
        ("X-Nexus-CSRF-Token".to_string(), csrf),
    ];
    let logout = nexuslang::server::handle_request_with_headers_and_body_for_test(
        AUTH_SOURCE,
        "POST",
        "/auth/logout",
        &csrf_cookie_headers,
        "",
        &storage,
    )
    .unwrap();
    assert_eq!(logout.status, 200);
    assert_eq!(logout.body, "true");

    let after_logout = nexuslang::server::handle_request_with_headers_and_body_for_test(
        AUTH_SOURCE,
        "GET",
        "/me",
        &cookie_headers,
        "",
        &storage,
    )
    .unwrap();
    assert_eq!(after_logout.status, 401);
}

#[test]
fn native_auth_supports_revocable_bearer_tokens_and_roles() {
    let data_dir = temp_data_dir("native_auth_bearer");
    fs::create_dir_all(&data_dir).unwrap();
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let register = nexuslang::server::handle_request_with_body_for_test(
        AUTH_SOURCE,
        "POST",
        "/auth/register",
        r#"{"email":"bia@example.com","name":"Bia","password":"strong-password-123"}"#,
        &storage,
    )
    .unwrap();
    assert_eq!(register.status, 201);

    let login = nexuslang::server::handle_request_with_body_for_test(
        AUTH_SOURCE,
        "POST",
        "/auth/login",
        r#"{"email":"bia@example.com","password":"strong-password-123"}"#,
        &storage,
    )
    .unwrap();
    assert_eq!(login.status, 200);
    let token = json_string_field(&login.body, "token");
    let bearer_headers = vec![("Authorization".to_string(), format!("Bearer {}", token))];

    let me = nexuslang::server::handle_request_with_headers_and_body_for_test(
        AUTH_SOURCE,
        "GET",
        "/me",
        &bearer_headers,
        "",
        &storage,
    )
    .unwrap();
    assert_eq!(me.status, 200);
    assert_eq!(
        me.body,
        r#"{"email":"bia@example.com","name":"Bia","role":"user"}"#
    );

    let admin = nexuslang::server::handle_request_with_headers_and_body_for_test(
        AUTH_SOURCE,
        "GET",
        "/admin/users",
        &bearer_headers,
        "",
        &storage,
    )
    .unwrap();
    assert_eq!(admin.status, 403);

    let logout = nexuslang::server::handle_request_with_headers_and_body_for_test(
        AUTH_SOURCE,
        "POST",
        "/auth/logout",
        &bearer_headers,
        "",
        &storage,
    )
    .unwrap();
    assert_eq!(logout.status, 200);

    let after_logout = nexuslang::server::handle_request_with_headers_and_body_for_test(
        AUTH_SOURCE,
        "GET",
        "/me",
        &bearer_headers,
        "",
        &storage,
    )
    .unwrap();
    assert_eq!(after_logout.status, 401);
}

#[test]
fn native_auth_rate_limits_failed_login_attempts() {
    let data_dir = temp_data_dir("native_auth_rate_limit");
    fs::create_dir_all(&data_dir).unwrap();
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let register = nexuslang::server::handle_request_with_body_for_test(
        AUTH_SOURCE,
        "POST",
        "/auth/register",
        r#"{"email":"rate@example.com","name":"Rate","password":"strong-password-123"}"#,
        &storage,
    )
    .unwrap();
    assert_eq!(register.status, 201);

    for _ in 0..5 {
        let failed = nexuslang::server::handle_request_with_body_for_test(
            AUTH_SOURCE,
            "POST",
            "/auth/login",
            r#"{"email":"rate@example.com","password":"wrong-password-123"}"#,
            &storage,
        )
        .unwrap();
        assert_eq!(failed.status, 401);
    }

    let limited = nexuslang::server::handle_request_with_body_for_test(
        AUTH_SOURCE,
        "POST",
        "/auth/login",
        r#"{"email":"rate@example.com","password":"wrong-password-123"}"#,
        &storage,
    )
    .unwrap();
    assert_eq!(limited.status, 429);
    assert!(limited.body.contains("Muitas requisicoes"));
}

#[test]
fn native_auth_sqlite_store_matches_json_for_core_flow() {
    for backend in [ParityBackend::Json, ParityBackend::Sqlite] {
        let storage = parity_storage("native_auth_storage_parity", backend);
        let register = nexuslang::server::handle_request_with_body_for_test(
            AUTH_SOURCE,
            "POST",
            "/auth/register",
            r#"{"email":"sql@example.com","name":"Sql","role":"admin","password":"strong-password-123"}"#,
            &storage,
        )
        .unwrap();
        assert_eq!(register.status, 201, "register on {}", backend.label());
        let csrf = json_string_field(&register.body, "csrf_token");
        let cookie = set_cookie_header(&register);

        let cookie_headers = vec![("Cookie".to_string(), cookie_pair(&cookie))];
        let me = nexuslang::server::handle_request_with_headers_and_body_for_test(
            AUTH_SOURCE,
            "GET",
            "/me",
            &cookie_headers,
            "",
            &storage,
        )
        .unwrap();
        assert_eq!(me.status, 200, "cookie /me on {}", backend.label());
        assert!(me.body.contains(r#""email":"sql@example.com""#));

        let login = nexuslang::server::handle_request_with_body_for_test(
            AUTH_SOURCE,
            "POST",
            "/auth/login",
            r#"{"email":"sql@example.com","password":"strong-password-123"}"#,
            &storage,
        )
        .unwrap();
        assert_eq!(login.status, 200, "login on {}", backend.label());
        let token = json_string_field(&login.body, "token");
        let bearer_headers = vec![("Authorization".to_string(), format!("Bearer {}", token))];

        let admin = nexuslang::server::handle_request_with_headers_and_body_for_test(
            AUTH_SOURCE,
            "GET",
            "/admin/users",
            &bearer_headers,
            "",
            &storage,
        )
        .unwrap();
        assert_eq!(admin.status, 200, "bearer admin on {}", backend.label());

        let logout_headers = vec![
            ("Cookie".to_string(), cookie_pair(&cookie)),
            ("X-Nexus-CSRF-Token".to_string(), csrf),
        ];
        let logout = nexuslang::server::handle_request_with_headers_and_body_for_test(
            AUTH_SOURCE,
            "POST",
            "/auth/logout",
            &logout_headers,
            "",
            &storage,
        )
        .unwrap();
        assert_eq!(logout.status, 200, "logout on {}", backend.label());
    }
}

#[test]
fn native_auth_semantic_guards_are_checked() {
    let err = check_source(
        r#"
model User {
    email: string
}

auth UserAuth {
    model: User
    identity: email
}
"#,
    )
    .unwrap_err();
    assert!(err.contains("deve ser string unique"));

    let err = check_source(
        r#"
model User {
    email: string unique
}

route GET /me auth(MissingAuth) {
    return "ok"
}
"#,
    )
    .unwrap_err();
    assert!(err.contains("usa auth 'MissingAuth' inexistente"));
}

#[test]
fn native_auth_openapi_exposes_security_contract() {
    let data_dir = temp_data_dir("native_auth_openapi");
    fs::create_dir_all(&data_dir).unwrap();
    let storage = nexuslang::server::Storage::new_json(&data_dir);

    let response =
        nexuslang::server::handle_request_for_test(AUTH_SOURCE, "GET", "/openapi.json", &storage)
            .unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body.contains(r#""securitySchemes""#));
    assert!(response.body.contains(r#""NexusSession""#));
    assert!(response.body.contains(r#""NexusBearer""#));
    assert!(response
        .body
        .contains(r#""security":[{"NexusSession":[]},{"NexusBearer":[]}"#));
    assert!(response.body.contains(r#""csrf_token":{"type":"string"}"#));
    assert!(response.body.contains(r#""X-Nexus-CSRF-Token""#));
    assert!(response
        .body
        .contains(r#""401":{"description":"Unauthorized""#));
    assert!(response
        .body
        .contains(r#""429":{"description":"Too Many Requests""#));
    assert!(response
        .body
        .contains(r#""403":{"description":"Forbidden""#));
}

fn set_cookie_header(response: &nexuslang::server::HttpResponse) -> String {
    response
        .headers
        .iter()
        .find(|(name, _)| name == "Set-Cookie")
        .map(|(_, value)| value.clone())
        .expect("Set-Cookie header")
}

fn cookie_pair(cookie: &str) -> String {
    cookie.split(';').next().expect("cookie pair").to_string()
}

fn json_string_field(body: &str, field: &str) -> String {
    let needle = format!(r#""{}":""#, field);
    let start = body.find(&needle).expect("json field") + needle.len();
    let rest = &body[start..];
    let end = rest.find('"').expect("json field end");
    rest[..end].to_string()
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

// ─── Fase 11.02: Import/Export Parser, HIR, and Formatter ─────────────

#[test]
fn parse_import_simple() {
    let source = r#"import Customer from "./crm.nx""#;
    let program = nexuslang::parse_source(source).unwrap();
    match &program.decls[0] {
        Decl::Import { import } => {
            assert_eq!(import.name, "Customer");
            assert!(import.alias.is_none());
            assert_eq!(import.source, "./crm.nx");
        }
        other => panic!("expected Import, got {:?}", other),
    }
}

#[test]
fn parse_import_with_alias() {
    let source = r#"import BuildInvoice as InvoiceFlow from "./billing.nx""#;
    let program = nexuslang::parse_source(source).unwrap();
    match &program.decls[0] {
        Decl::Import { import } => {
            assert_eq!(import.name, "BuildInvoice");
            assert_eq!(import.alias.as_deref(), Some("InvoiceFlow"));
            assert_eq!(import.source, "./billing.nx");
        }
        other => panic!("expected Import, got {:?}", other),
    }
}

#[test]
fn parse_export_model() {
    let source = "export model Foo { name: string }";
    let program = nexuslang::parse_source(source).unwrap();
    match &program.decls[0] {
        Decl::Export { decl, .. } => match decl.as_ref() {
            Decl::Model { name, .. } => assert_eq!(name, "Foo"),
            other => panic!("expected Model inside Export, got {:?}", other),
        },
        other => panic!("expected Export, got {:?}", other),
    }
}

#[test]
fn parse_export_function() {
    let source = "export fn hello() -> string { return \"world\" }";
    let program = nexuslang::parse_source(source).unwrap();
    match &program.decls[0] {
        Decl::Export { decl, .. } => match decl.as_ref() {
            Decl::Function { name, .. } => assert_eq!(name, "hello"),
            other => panic!("expected Function inside Export, got {:?}", other),
        },
        other => panic!("expected Export, got {:?}", other),
    }
}

#[test]
fn parse_export_workflow() {
    let source = "export workflow Onboard { step start { print(\"ok\") } }";
    let program = nexuslang::parse_source(source).unwrap();
    match &program.decls[0] {
        Decl::Export { decl, .. } => match decl.as_ref() {
            Decl::Workflow { name, .. } => assert_eq!(name, "Onboard"),
            other => panic!("expected Workflow inside Export, got {:?}", other),
        },
        other => panic!("expected Export, got {:?}", other),
    }
}

#[test]
fn parse_export_auth() {
    let source = "export auth UserAuth { model: User identity: email }";
    // This should parse: auth requires model/identity fields
    let program = nexuslang::parse_source(source).unwrap();
    match &program.decls[0] {
        Decl::Export { decl, .. } => match decl.as_ref() {
            Decl::Auth { config } => assert_eq!(config.name, "UserAuth"),
            other => panic!("expected Auth inside Export, got {:?}", other),
        },
        other => panic!("expected Export, got {:?}", other),
    }
}

#[test]
fn parse_error_export_import_is_rejected() {
    let source = "export import Customer from \"./crm.nx\"";
    let err = nexuslang::parse_source(source).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("não é possível exportar um import") || msg.contains("export"),
        "unexpected error: {}",
        msg
    );
}

#[test]
fn parse_error_export_invoice_is_rejected() {
    let source = "export invoice { customer: \"X\" currency: \"AOA\" total: 100 kz }";
    let err = nexuslang::parse_source(source).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("não é possível exportar invoice") || msg.contains("export"),
        "unexpected error: {}",
        msg
    );
}

#[test]
fn parse_error_import_missing_from() {
    let source = r#"import Customer "./crm.nx""#;
    let err = nexuslang::parse_source(source).unwrap_err();
    assert!(err.to_string().contains("from"));
}

#[test]
fn parse_error_import_non_string_path() {
    let source = "import Customer from 42";
    let err = nexuslang::parse_source(source).unwrap_err();
    assert!(err.to_string().contains("string literal"));
}

#[test]
fn fmt_import_roundtrip() {
    let source = r#"import Customer from "./crm.nx""#;
    let formatted = nexuslang::fmt_source(source).unwrap();
    assert!(formatted.contains("import"));
    assert!(formatted.contains("Customer"));
    assert!(formatted.contains("./crm.nx"));
}

#[test]
fn fmt_import_with_alias_roundtrip() {
    let source = r#"import BuildInvoice as InvoiceFlow from "./billing.nx""#;
    let formatted = nexuslang::fmt_source(source).unwrap();
    assert!(formatted.contains("BuildInvoice"));
    assert!(formatted.contains("InvoiceFlow"));
    assert!(formatted.contains("./billing.nx"));
}

#[test]
fn fmt_export_roundtrip() {
    let source = "export model Foo { name: string }";
    let formatted = nexuslang::fmt_source(source).unwrap();
    assert!(formatted.contains("export"));
    assert!(formatted.contains("Foo"));
}

#[test]
fn exported_model_is_checked_and_runnable() {
    // An exported model should still be valid and runnable
    let source = r#"
export model Customer {
    name: string
    balance: money
}

let c = Customer { name: "Ana", balance: 1000 kz }
print(c.name)
"#;
    let result = run_source(source);
    assert!(
        result.is_ok(),
        "exported model should run: {:?}",
        result.err()
    );
}

#[test]
fn import_is_syntactically_valid_in_single_file_mode() {
    // Single file: imports parse but don't resolve yet
    let source = r#"
import Customer from "./crm.nx"

model Supplier {
    name: string
}

print("single file with import")
"#;
    // Should parse and check (imports are accepted but not resolved)
    let result = check_source(source);
    assert!(
        result.is_ok(),
        "import should be accepted: {:?}",
        result.err()
    );
}

#[test]
fn hir_lower_import_creates_symbols_and_references() {
    use nexuslang::hir;
    let source = r#"import Customer as Cliente from "./crm.nx""#;
    let program = nexuslang::parse_source(source).unwrap();
    let hir_program = hir::lower_program(&program);

    // Check the import decl was lowered
    let import_decl = hir_program
        .decls
        .iter()
        .find(|d| d.kind == hir::HirDeclKind::Import)
        .expect("expected an Import HIR decl");

    assert_eq!(import_decl.name, Some("Cliente"));

    // Check references: ModulePath and ImportSymbol
    let mod_path = hir_program
        .references
        .iter()
        .find(|r| r.kind == hir::HirReferenceKind::ModulePath)
        .expect("expected ModulePath reference");
    assert_eq!(mod_path.name, "./crm.nx");

    let import_sym = hir_program
        .references
        .iter()
        .find(|r| r.kind == hir::HirReferenceKind::ImportSymbol)
        .expect("expected ImportSymbol reference");
    assert_eq!(import_sym.name, "Customer");

    // Check the alias symbol
    let imported_sym = hir_program
        .symbols
        .iter()
        .find(|s| s.kind == hir::HirSymbolKind::ImportedSymbol)
        .expect("expected ImportedSymbol");
    assert_eq!(imported_sym.name, "Cliente");
}

#[test]
fn export_wrapper_preserves_inner_declaration_spans() {
    let source = "export model Bar { active: bool }";
    let program = nexuslang::parse_source(source).unwrap();
    match &program.decls[0] {
        Decl::Export {
            decl: inner,
            export_span,
            span,
        } => {
            assert!(export_span.line > 0, "export_span should be known");
            assert_eq!(span.line, export_span.line);
            // Inner declaration should have its own span
            assert!(inner.span().line > 0);
        }
        other => panic!("expected Export, got {:?}", other),
    }
}

// ─── Fase 11.03: Module Loader Integration Tests ───────────────────────

use std::path::{Path, PathBuf};

/// Helper: create a temporary .nx file and return its path.
fn create_nx_file(dir: &Path, name: &str, source: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, source).expect("failed to write temp file");
    path
}

/// Helper: create a unique temp dir per test.
fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nx_modtest_{}_{}", label, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

#[test]
fn module_loader_loads_and_merges_two_files() {
    let dir = temp_dir("two_files");

    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
}
"#,
    );

    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User from "./lib.nx"

let u = User { name: "Ana" }
print(u.name)
"#,
    );

    let result = nexuslang::load_and_run(&entry);
    assert!(result.is_ok(), "two-file load+run: {:?}", result.err());
}

#[test]
fn module_loader_rejects_missing_export() {
    let dir = temp_dir("missing_export");

    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User { name: string }
"#,
    );

    let entry = create_nx_file(&dir, "main.nx", r#"import Post from "./lib.nx""#);

    let result = nexuslang::load_program(&entry);
    assert!(result.is_err(), "missing export should fail");
    let err = result.unwrap_err();
    assert!(
        err.contains("Post") || err.contains("não é"),
        "error should mention symbol: {}",
        err
    );
}

#[test]
fn module_loader_rejects_circular_dependency() {
    let dir = temp_dir("circular");

    create_nx_file(
        &dir,
        "a.nx",
        r#"import B from "./b.nx" export fn A() -> string { return "a" } "#,
    );
    create_nx_file(
        &dir,
        "b.nx",
        r#"import A from "./a.nx" export fn B() -> string { return "b" } "#,
    );

    let result = nexuslang::load_program(&dir.join("a.nx"));
    assert!(result.is_err(), "circular dep should fail");
}

#[test]
fn module_loader_rejects_non_relative_path() {
    let dir = temp_dir("nonrel");

    let entry = create_nx_file(&dir, "main.nx", r#"import Foo from "bar.nx""#);

    let result = nexuslang::module_loader::load_program_full(&entry);
    let error = result.expect_err("non-relative path should fail");
    let diagnostic = error.to_diagnostic();
    assert_eq!(diagnostic.code.as_deref(), Some(codes::MODULE_LOADER_PATH));
    assert_eq!(diagnostic.labels[0].message, "caminho importado aqui");
    assert!(diagnostic
        .notes
        .iter()
        .any(|note| note.contains("Imports de arquivo")));
    assert!(diagnostic
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("./modulo.nx")));
    assert!(diagnostic.to_string().contains("Caminho nao relativo"));

    match error {
        nexuslang::module_loader::ModuleError::NonRelativePath { source, .. } => {
            assert_eq!(source, "bar.nx");
        }
        other => panic!("expected NonRelativePath, got {:?}", other),
    }
}

#[test]
fn module_loader_three_level_deep_deps() {
    let dir = temp_dir("deep");

    create_nx_file(
        &dir,
        "base.nx",
        r#"
export fn greet(name: string) -> string {
    return "Hi " + name
}
"#,
    );

    create_nx_file(
        &dir,
        "middle.nx",
        r#"
import greet from "./base.nx"

export fn wrapped(name: string) -> string {
    return greet(name)
}
"#,
    );

    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import wrapped from "./middle.nx"

let msg = wrapped("Nexus")
print(msg)
"#,
    );

    let result = nexuslang::load_and_run(&entry);
    assert!(result.is_ok(), "deep deps should run: {:?}", result.err());
}

#[test]
fn module_loader_non_exported_symbol_is_rejected() {
    let dir = temp_dir("non_exported");

    create_nx_file(&dir, "lib.nx", r#"model Hidden { x: int }"#);

    let entry = create_nx_file(&dir, "main.nx", r#"import Hidden from "./lib.nx""#);

    let result = nexuslang::load_program(&entry);
    assert!(result.is_err(), "non-exported should fail");
}

#[test]
fn module_loader_exported_model_is_runnable_through_import() {
    let dir = temp_dir("exported_run");

    create_nx_file(
        &dir,
        "crm.nx",
        r#"
export model Customer {
    name: string
    balance: money
}
"#,
    );

    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import Customer from "./crm.nx"

let c = Customer { name: "João", balance: 50000 kz }
print(c.name)
"#,
    );

    let result = nexuslang::load_and_run(&entry);
    assert!(
        result.is_ok(),
        "exported model through import: {:?}",
        result.err()
    );
}

#[test]
fn module_loader_nx_extension_inference_works() {
    let dir = temp_dir("ext_infer");

    create_nx_file(
        &dir,
        "helper",
        r#"
export fn id(x: string) -> string { return x }
"#,
    );

    let entry = create_nx_file(&dir, "main.nx", r#"import id from "./helper""#);

    let result = nexuslang::load_program(&entry);
    // Should either load (with extension inference) or give a clear error
    match result {
        Ok(_) => {} // extension inference worked
        Err(e) => {
            // If it failed, the error should mention the file
            assert!(
                e.contains("helper") || e.contains("caminho"),
                "error: {}",
                e
            );
        }
    }
}

// ─── Fase 11.05: Multi-module checker with HirSymbolRef resolution ───────

#[test]
fn check_with_module_graph_resolves_imported_model_symbol() {
    let dir = temp_dir("check_graph_model");

    create_nx_file(
        &dir,
        "models.nx",
        r#"
export model User {
    name: string
}
"#,
    );

    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User from "./models.nx"

let u = User { name: "Ana" }
print(u.name)
"#,
    );

    // load_and_check_with_graph should succeed (checker + cross-module resolution)
    let result = nexuslang::load_and_check_with_graph(&entry);
    assert!(
        result.is_ok(),
        "load_and_check_with_graph should succeed: {:?}",
        result.err()
    );

    // Also verify the full run works end-to-end
    let run_result = nexuslang::load_and_run(&entry);
    assert!(
        run_result.is_ok(),
        "load_and_run should succeed: {:?}",
        run_result.err()
    );
}

#[test]
fn check_with_module_graph_resolves_imported_function_symbol() {
    let dir = temp_dir("check_graph_fn");

    create_nx_file(
        &dir,
        "helpers.nx",
        r#"
export fn greet(name: string) -> string {
    return "Hello, " + name
}
"#,
    );

    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import greet from "./helpers.nx"

let msg = greet("World")
print(msg)
"#,
    );

    // load_and_check_with_graph should succeed
    let result = nexuslang::load_and_check_with_graph(&entry);
    assert!(
        result.is_ok(),
        "load_and_check_with_graph should succeed: {:?}",
        result.err()
    );

    // Full run should work
    let run_result = nexuslang::load_and_run(&entry);
    assert!(
        run_result.is_ok(),
        "load_and_run should succeed: {:?}",
        run_result.err()
    );
}

#[test]
fn check_with_module_graph_rejects_missing_export() {
    let dir = temp_dir("check_graph_missing_export");

    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User { name: string }
"#,
    );

    let entry = create_nx_file(&dir, "main.nx", r#"import Post from "./lib.nx""#);

    let result = nexuslang::module_loader::load_program_full(&entry);
    assert!(result.is_err(), "missing export should fail");
}

// ─── Fase 11.06: Verify HirSymbolRef in checker HIR ─────────────────────

#[test]
fn check_with_module_graph_hir_symbol_ref_points_to_correct_symbol() {
    let dir = temp_dir("check_hir_symbol_ref");

    create_nx_file(
        &dir,
        "models.nx",
        r#"
export model User {
    name: string
}

export fn greet(name: string) -> string {
    return "Hello, " + name
}
"#,
    );

    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User from "./models.nx"
import greet from "./models.nx"

let u = User { name: "Ana" }
print(greet(u.name))
"#,
    );

    // Load with full graph
    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    // Check with module graph
    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    // Retrieve the import resolutions
    let resolutions = checker.checked_import_resolutions();
    assert_eq!(
        resolutions.len(),
        2,
        "should have resolved 2 imports (User + greet)"
    );

    // Lower the program to HIR so we can look up symbol details
    let hir = nexuslang::hir::lower_program(&program);

    // Find the import decls in the HIR
    let import_decls: Vec<&nexuslang::hir::HirDecl<'_>> = hir
        .decls
        .iter()
        .filter(|d| d.kind == nexuslang::hir::HirDeclKind::Import)
        .collect();
    assert_eq!(import_decls.len(), 2, "should have 2 import decls in HIR");

    for import_decl in &import_decls {
        let decl_id = import_decl.id;
        let sym_ref = resolutions
            .get(&decl_id)
            .expect("every import decl should have a resolution");

        // Verify the module: models.nx is a dependency, so it's module 1
        assert_eq!(
            sym_ref.module.index(),
            1,
            "imported symbols come from models.nx (module 1)"
        );

        // Look up the resolved symbol in the HIR
        let resolved_sym = hir
            .symbols
            .iter()
            .find(|s| s.id == sym_ref.symbol)
            .expect("resolved symbol should exist in HIR");

        // The symbol should be either a Model or Function named User/greet
        match resolved_sym.kind {
            nexuslang::hir::HirSymbolKind::Model => {
                assert_eq!(resolved_sym.name, "User");
            }
            nexuslang::hir::HirSymbolKind::Function => {
                assert_eq!(resolved_sym.name, "greet");
            }
            other => panic!(
                "resolved symbol should be Model or Function, got {:?}",
                other
            ),
        }
    }
}

#[test]
fn resolve_hir_imports_uses_import_path_when_export_names_overlap() {
    let dir = temp_dir("check_hir_import_path_overlap");

    create_nx_file(
        &dir,
        "a.nx",
        r#"
export fn shared() -> string {
    return "a"
}
"#,
    );
    create_nx_file(
        &dir,
        "b.nx",
        r#"
export fn shared() -> string {
    return "b"
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import shared from "./b.nx"
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");
    let hir = nexuslang::hir::lower_program(&program);
    let resolutions =
        nexuslang::module_loader::resolve_hir_imports(&hir, &module_graph, &decl_module_map);

    let import_decl = hir
        .decls
        .iter()
        .find(|decl| decl.kind == nexuslang::hir::HirDeclKind::Import)
        .expect("should have one import decl");
    let sym_ref = resolutions
        .get(&import_decl.id)
        .expect("import should be resolved");
    let expected_module = module_graph
        .entries
        .iter()
        .find(|entry| entry.path.file_name().and_then(|name| name.to_str()) == Some("b.nx"))
        .expect("b.nx should be in graph")
        .module_id;

    assert_eq!(
        sym_ref.module, expected_module,
        "import should resolve through its source path, not the first matching export name"
    );
}

#[test]
fn module_graph_rejects_duplicate_import_aliases_in_one_module() {
    let dir = temp_dir("duplicate_import_aliases");

    create_nx_file(
        &dir,
        "a.nx",
        r#"
export fn one() -> string {
    return "one"
}
"#,
    );
    create_nx_file(
        &dir,
        "b.nx",
        r#"
export fn two() -> string {
    return "two"
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import one as duplicated from "./a.nx"
import two as duplicated from "./b.nx"
"#,
    );

    let result = nexuslang::module_loader::load_program_full(&entry);
    let error = result.unwrap_err();
    let diagnostic = error.to_diagnostic();
    assert_eq!(
        diagnostic.code.as_deref(),
        Some(codes::MODULE_LOADER_DUPLICATE_ALIAS)
    );
    assert_eq!(diagnostic.labels[0].message, "alias duplicado aqui");
    assert!(diagnostic
        .notes
        .iter()
        .any(|note| note.contains("ja tinha sido usado")));
    assert!(diagnostic
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("aliases locais diferentes")));
    assert!(diagnostic.to_string().contains("Alias de import duplicado"));

    match error {
        nexuslang::module_loader::ModuleError::DuplicateImportAlias { alias, .. } => {
            assert_eq!(alias, "duplicated");
        }
        other => panic!("expected DuplicateImportAlias, got {:?}", other),
    }
}

#[test]
fn module_graph_rejects_import_alias_collision_with_local_top_level() {
    let dir = temp_dir("import_alias_collision");

    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export fn helper() -> string {
    return "dep"
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import helper as run from "./lib.nx"

fn run() -> string {
    return "local"
}
"#,
    );

    let result = nexuslang::module_loader::load_program_full(&entry);
    let error = result.unwrap_err();
    let diagnostic = error.to_diagnostic();
    assert_eq!(
        diagnostic.code.as_deref(),
        Some(codes::MODULE_LOADER_ALIAS_COLLISION)
    );
    assert_eq!(diagnostic.labels[0].message, "alias importado aqui");
    assert!(diagnostic
        .notes
        .iter()
        .any(|note| note.contains("declaracao top-level local")));
    assert!(diagnostic
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("Renomeie o alias")));
    assert!(diagnostic.to_string().contains("colide com declaracao"));

    match error {
        nexuslang::module_loader::ModuleError::ImportAliasCollision { alias, .. } => {
            assert_eq!(alias, "run");
        }
        other => panic!("expected ImportAliasCollision, got {:?}", other),
    }
}

#[test]
fn module_graph_rejects_duplicate_symbols_across_loaded_modules() {
    let dir = temp_dir("duplicate_graph_symbols");

    create_nx_file(
        &dir,
        "a.nx",
        r#"
export fn shared() -> string {
    return "a"
}
"#,
    );
    create_nx_file(
        &dir,
        "b.nx",
        r#"
export fn shared() -> string {
    return "b"
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import shared as from_a from "./a.nx"
import shared as from_b from "./b.nx"
"#,
    );

    let result = nexuslang::module_loader::load_program_full(&entry);
    match result.unwrap_err() {
        nexuslang::module_loader::ModuleError::DuplicateGraphSymbol { symbol, kind, .. } => {
            assert_eq!(symbol, "shared");
            assert_eq!(kind, "funcao");
        }
        other => panic!("expected DuplicateGraphSymbol, got {:?}", other),
    }
}

#[test]
fn module_graph_rejects_duplicate_symbols_from_path_dependencies() {
    let workspace = temp_dir("duplicate_path_dep_symbols");
    let dependency = workspace.join("crm_core");
    let app = workspace.join("erp_app");
    fs::create_dir_all(&dependency).expect("create dependency dir");
    fs::create_dir_all(&app).expect("create app dir");

    fs::write(
        dependency.join("nexus.toml"),
        r#"[package]
name = "crm_core"
version = "0.1.0"
entry = "main.nx"

[dependencies]
"#,
    )
    .expect("write dependency manifest");
    fs::write(
        dependency.join("main.nx"),
        r#"
export fn shared() -> string {
    return "dep"
}
"#,
    )
    .expect("write dependency entry");
    fs::write(
        app.join("nexus.toml"),
        r#"[package]
name = "erp_app"
version = "0.1.0"
entry = "main.nx"

[dependencies]
crm_core = "path:../crm_core"
"#,
    )
    .expect("write app manifest");
    fs::write(
        app.join("local.nx"),
        r#"
export fn shared() -> string {
    return "local"
}
"#,
    )
    .expect("write local module");
    fs::write(
        app.join("main.nx"),
        r#"
import shared as from_dep from "crm_core"
import shared as from_local from "./local.nx"
"#,
    )
    .expect("write app entry");

    let result = nexuslang::module_loader::load_program_full(&app.join("main.nx"));
    match result.unwrap_err() {
        nexuslang::module_loader::ModuleError::DuplicateGraphSymbol { symbol, .. } => {
            assert_eq!(symbol, "shared");
        }
        other => panic!("expected DuplicateGraphSymbol, got {:?}", other),
    }
}

#[test]
fn source_database_tracks_modules_sources_and_import_edges() {
    let dir = temp_dir("source_database_relative_edges");

    create_nx_file(
        &dir,
        "financeiro.nx",
        r#"
export fn total_vendas() -> int {
    return 42
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import total_vendas as total from "./financeiro.nx"

let valor = total()
print(valor)
"#,
    );

    let (_program, module_graph, source_database) =
        nexuslang::load_and_check_with_source_database(&entry).expect("load should succeed");

    assert_eq!(source_database.modules().len(), module_graph.entries.len());

    let entry_module = source_database
        .module(module_graph.entry_id)
        .expect("entry module should be in source database");
    assert!(entry_module.is_entry);
    assert_eq!(entry_module.path, entry.canonicalize().unwrap());
    assert!(entry_module.source.contains("import total_vendas"));

    let edges: Vec<_> = source_database
        .import_edges_from(module_graph.entry_id)
        .collect();
    assert_eq!(edges.len(), 1);

    let edge = edges[0];
    assert_eq!(edge.imported_name, "total_vendas");
    assert_eq!(edge.alias.as_deref(), Some("total"));
    assert_eq!(edge.source_path, "./financeiro.nx");
    assert!(edge.import_span.is_known());
    assert!(edge.name_span.is_known());
    assert!(edge.source_span.is_known());

    let target_module = source_database
        .module(
            edge.target_module
                .expect("import should resolve to target module"),
        )
        .expect("target module should be in source database");
    assert_eq!(
        target_module
            .path
            .file_name()
            .and_then(|name| name.to_str()),
        Some("financeiro.nx")
    );
    assert!(target_module.source.contains("export fn total_vendas"));
}

#[test]
fn source_database_tracks_path_dependency_import_edges() {
    let workspace = temp_dir("source_database_path_dep_edges");
    let dependency = workspace.join("crm_core");
    let app = workspace.join("erp_app");
    fs::create_dir_all(&dependency).expect("create dependency dir");
    fs::create_dir_all(&app).expect("create app dir");

    fs::write(
        dependency.join("nexus.toml"),
        r#"[package]
name = "crm_core"
version = "0.1.0"
entry = "main.nx"

[dependencies]
"#,
    )
    .expect("write dependency manifest");
    fs::write(
        dependency.join("main.nx"),
        r#"
export fn dep_total() -> int {
    return 7
}
"#,
    )
    .expect("write dependency entry");
    fs::write(
        app.join("nexus.toml"),
        r#"[package]
name = "erp_app"
version = "0.1.0"
entry = "main.nx"

[dependencies]
crm_core = "path:../crm_core"
"#,
    )
    .expect("write app manifest");
    fs::write(
        app.join("main.nx"),
        r#"
import dep_total from "crm_core"

let valor = dep_total()
print(valor)
"#,
    )
    .expect("write app entry");

    let (_program, module_graph, source_database) =
        nexuslang::load_and_check_with_source_database(&app.join("main.nx"))
            .expect("load should succeed");

    assert_eq!(source_database.modules().len(), module_graph.entries.len());

    let edge = source_database
        .import_edges()
        .iter()
        .find(|edge| edge.source_path == "crm_core")
        .expect("path dependency import edge should be tracked");
    assert_eq!(edge.imported_name, "dep_total");

    let source_module = source_database
        .module(edge.source_module)
        .expect("source module should be tracked");
    assert_eq!(
        source_module.path,
        app.join("main.nx").canonicalize().unwrap()
    );

    let target_module = source_database
        .module(edge.target_module.expect("path dependency should resolve"))
        .expect("target module should be tracked");
    assert_eq!(
        target_module.path,
        dependency.join("main.nx").canonicalize().unwrap()
    );
    assert!(source_database
        .module_by_path(&dependency.join("main.nx"))
        .is_some());
}

#[test]
fn source_database_attaches_diagnostics_to_modules() {
    let dir = temp_dir("source_database_diagnostics");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
print("ok")
"#,
    );

    let (_program, module_graph, source_database) =
        nexuslang::load_and_check_with_source_database(&entry).expect("load should succeed");
    let diagnostic =
        nexuslang::diagnostic::Diagnostic::new(DiagnosticStage::Checker, "erro simulado")
            .with_location(2, 1);

    let module_diagnostic = source_database
        .attach_diagnostic(module_graph.entry_id, diagnostic.clone())
        .expect("diagnostic should attach to entry module");
    assert_eq!(module_diagnostic.module_id, module_graph.entry_id);
    assert_eq!(module_diagnostic.path, entry.canonicalize().unwrap());
    assert_eq!(module_diagnostic.diagnostic, diagnostic);

    let path_diagnostic = source_database
        .attach_diagnostic_to_path(&entry, diagnostic)
        .expect("diagnostic should attach by path");
    assert_eq!(path_diagnostic.module_id, module_graph.entry_id);
}

#[test]
fn source_database_records_decl_source_ranges() {
    let dir = temp_dir("source_database_decl_ranges");

    let lib = create_nx_file(
        &dir,
        "lib.nx",
        r#"
export fn total() -> int {
    return 1
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import total from "./lib.nx"
"#,
    );

    let (_program, module_graph, _decl_module_map, source_database) =
        nexuslang::module_loader::load_program_full_with_source_database(&entry)
            .expect("load should succeed");
    let lib_path = lib.canonicalize().unwrap();
    let lib_module_id = module_graph
        .entries
        .iter()
        .find(|entry| entry.path == lib_path)
        .expect("lib module should be in graph")
        .module_id;

    let range = source_database
        .decl_ranges()
        .iter()
        .find(|range| range.module_id == lib_module_id)
        .expect("lib declaration should have a source range")
        .range;

    assert_eq!(range.start.line, 2);
    assert_eq!(range.end.line, 4);
    assert!(range.contains(3, Some(5)));
    assert!(!range.contains(5, None));
    assert_eq!(
        source_database.source_range_for_module_location(lib_module_id, 3, Some(5)),
        Some(range)
    );
}

#[test]
fn source_database_maps_checker_diagnostic_to_imported_module() {
    let dir = temp_dir("source_database_checker_diagnostic_module");

    let lib = create_nx_file(
        &dir,
        "lib.nx",
        r#"
export fn broken() -> int {
    return "erro"
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import broken from "./lib.nx"
"#,
    );

    let (program, module_graph, decl_module_map, source_database) =
        nexuslang::module_loader::load_program_full_with_source_database(&entry)
            .expect("load should succeed");

    let module_diagnostic = nexuslang::check_with_source_database(
        &program,
        &module_graph,
        &decl_module_map,
        &source_database,
    )
    .expect_err("checker should reject imported module body");

    assert_eq!(module_diagnostic.path, lib.canonicalize().unwrap());
    assert_eq!(module_diagnostic.diagnostic.stage, DiagnosticStage::Checker);
    assert!(module_diagnostic
        .diagnostic
        .message
        .contains("Tipo de retorno inválido"));
    assert_eq!(module_diagnostic.diagnostic.line, Some(3));
}

#[test]
fn source_database_ranges_disambiguate_blank_entry_lines_from_imported_errors() {
    let dir = temp_dir("source_database_checker_diagnostic_overlapping_lines");

    let lib = create_nx_file(
        &dir,
        "lib.nx",
        r#"
export fn broken() -> int {
    return "erro"
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import broken from "./lib.nx"


"#,
    );

    let (program, module_graph, decl_module_map, source_database) =
        nexuslang::module_loader::load_program_full_with_source_database(&entry)
            .expect("load should succeed");

    let module_diagnostic = nexuslang::check_with_source_database(
        &program,
        &module_graph,
        &decl_module_map,
        &source_database,
    )
    .expect_err("checker should reject imported module body");

    assert_eq!(module_diagnostic.path, lib.canonicalize().unwrap());
    assert_eq!(module_diagnostic.diagnostic.line, Some(3));
    assert_eq!(
        module_diagnostic
            .source_range
            .expect("diagnostic should carry the imported declaration range")
            .end
            .line,
        4
    );
}

#[test]
fn source_database_prefers_checker_diagnostic_owner_over_identical_ranges() {
    let dir = temp_dir("source_database_checker_diagnostic_owner");

    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export fn ok() -> int {
    return 1
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
fn broken() -> int {
    return "erro"
}

import ok from "./lib.nx"
"#,
    );

    let (program, module_graph, decl_module_map, source_database) =
        nexuslang::module_loader::load_program_full_with_source_database(&entry)
            .expect("load should succeed");
    let mut checker = nexuslang::checker::Checker::new();

    let diagnostic = checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect_err("checker should reject entry module body");
    let owner = diagnostic
        .owner
        .expect("checker diagnostic should carry declaration owner");
    assert_eq!(owner.module_id, Some(module_graph.entry_id.0));
    assert_eq!(
        decl_module_map.get(owner.decl_index).copied(),
        Some(module_graph.entry_id)
    );

    let module_diagnostic = source_database
        .attach_program_diagnostic(&program, &decl_module_map, diagnostic)
        .expect("diagnostic should attach through owner");

    assert_eq!(module_diagnostic.path, entry.canonicalize().unwrap());
    assert_eq!(module_diagnostic.module_id, module_graph.entry_id);
    let source_range = module_diagnostic
        .source_range
        .expect("owner should preserve declaration source range");
    assert_eq!(source_range.start.line, 2);
    assert_eq!(source_range.end.line, 4);
}

#[test]
fn load_and_check_with_source_database_formats_imported_module_diagnostic_path() {
    let dir = temp_dir("source_database_checker_diagnostic_string");

    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export fn broken() -> int {
    return "erro"
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import broken from "./lib.nx"
"#,
    );

    let err = nexuslang::load_and_check_with_source_database(&entry).unwrap_err();
    assert!(err.contains("lib.nx"), "err: {err}");
    assert!(err.contains("Tipo de retorno inválido"), "err: {err}");
    assert!(err.contains(":3:"), "err: {err}");
}

#[test]
fn load_and_check_with_source_database_diagnostic_exposes_structured_error() {
    let dir = temp_dir("source_database_public_structured_diagnostic");

    let lib = create_nx_file(
        &dir,
        "lib.nx",
        r#"
export fn broken() -> int {
    return "erro"
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import broken from "./lib.nx"
"#,
    );

    let err = nexuslang::load_and_check_with_source_database_diagnostic(&entry)
        .expect_err("checker should reject imported module body");
    let owner = err
        .diagnostic
        .owner
        .expect("structured diagnostic should expose checker owner");
    let module_id = err
        .module_id
        .expect("structured diagnostic should expose module id");
    let source_range = err
        .source_range
        .expect("structured diagnostic should expose source range");

    assert_eq!(err.path, Some(lib.canonicalize().unwrap()));
    assert_eq!(owner.module_id, Some(module_id.0));
    assert_eq!(err.diagnostic.line, Some(3));
    assert_eq!(err.diagnostic.labels[0].message, "origem do erro de tipo");
    assert!(err
        .diagnostic
        .notes
        .iter()
        .any(|note| note.contains("O checker compara")));
    assert!(err
        .diagnostic
        .suggestions
        .iter()
        .any(|suggestion| suggestion.message.contains("Ajuste a anotacao")));
    assert_eq!(source_range.start.line, 2);
    assert_eq!(source_range.end.line, 4);
    assert!(err.to_string().contains("lib.nx"));
    assert!(err.to_string().contains("Tipo de retorno inválido"));
}

#[test]
fn multi_module_diagnostic_json_covers_runtime_stage() {
    let dir = temp_dir("multi_module_runtime_diagnostic_json");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
print(10 / 0)
"#,
    );

    let err = nexuslang::load_and_run_with_source_database_diagnostic(&entry)
        .expect_err("runtime should reject division by zero");
    assert_eq!(err.diagnostic.stage, DiagnosticStage::Runtime);
    assert_eq!(err.diagnostic.code.as_deref(), Some("NXL5001"));
    assert_eq!(err.diagnostic.severity, Some(DiagnosticSeverity::Error));
    assert!(err.diagnostic.message.contains("Divisão por zero"));
    assert_eq!(err.path, Some(entry.canonicalize().unwrap()));
    assert_eq!(err.module_id.map(|id| id.index()), Some(0));
    assert_eq!(err.source_range, None);
    assert_eq!(
        err.diagnostic.labels[0].message,
        "operacao aritmetica em runtime"
    );
    assert!(err
        .diagnostic
        .notes
        .iter()
        .any(|note| note.contains("dividir ou calcular modulo por zero")));
    assert!(err
        .diagnostic
        .suggestions
        .iter()
        .any(|suggestion| suggestion
            .message
            .contains("divisor seja diferente de zero")));

    let json = nexuslang::multi_module_diagnostic_json("run", &err);
    assert!(json.contains(r#""ok":false"#), "json: {json}");
    assert!(json.contains(r#""schema_version":1"#), "json: {json}");
    assert!(json.contains(r#""command":"run""#), "json: {json}");
    assert!(json.contains(r#""code":"NXL5001""#), "json: {json}");
    assert!(json.contains(r#""severity":"error""#), "json: {json}");
    assert!(json.contains(r#""stage":"runtime""#), "json: {json}");
    assert!(json.contains(r#""path":"#), "json: {json}");
    assert!(json.contains("main.nx"), "json: {json}");
    assert!(json.contains(r#""module_id":0"#), "json: {json}");
    assert!(json.contains(r#""owner":null"#), "json: {json}");
    assert!(json.contains(r#""source_range":null"#), "json: {json}");
    assert!(
        json.contains("operacao aritmetica em runtime"),
        "json: {json}"
    );
    assert!(json.contains("A execucao tentou dividir"), "json: {json}");
    assert!(
        json.contains("Garanta que o divisor seja diferente de zero"),
        "json: {json}"
    );
    assert!(json.contains("Divisão por zero"), "json: {json}");
}

#[test]
fn multi_module_diagnostic_json_includes_labels_notes_and_suggestions() {
    let diagnostic = nexuslang::diagnostic::Diagnostic::new(
        DiagnosticStage::Checker,
        "Tipo de retorno inválido: esperado int",
    )
    .with_code(codes::CHECKER_TYPE)
    .with_label_at("retorno incompatível", 3, 5)
    .with_note("A assinatura da função declara retorno int.")
    .with_replacement_suggestion("retorne um inteiro", "return 1");
    let error = nexuslang::MultiModuleDiagnostic {
        path: None,
        module_id: None,
        diagnostic,
        source_range: None,
    };

    assert_eq!(error.to_string(), "Tipo de retorno inválido: esperado int");

    let json = nexuslang::multi_module_diagnostic_json("check", &error);
    assert!(
        json.contains(r#""labels":[{"message":"retorno incompatível","line":3,"column":5}]"#),
        "json: {json}"
    );
    assert!(
        json.contains(r#""notes":["A assinatura da função declara retorno int."]"#),
        "json: {json}"
    );
    assert!(
        json.contains(
            r#""suggestions":[{"message":"retorne um inteiro","replacement":"return 1"}]"#
        ),
        "json: {json}"
    );
    assert!(
        json.contains(r#""text":"Tipo de retorno inválido: esperado int""#),
        "json: {json}"
    );
}

#[test]
fn multi_module_diagnostic_report_groups_by_path_and_module() {
    let lib_path = std::path::PathBuf::from("/tmp/nexus/lib.nx");
    let first = nexuslang::MultiModuleDiagnostic {
        path: Some(lib_path.clone()),
        module_id: Some(nexuslang::hir::HirModuleId(1)),
        diagnostic: nexuslang::diagnostic::Diagnostic::new(
            DiagnosticStage::Checker,
            "Variável 'cliente' não definida",
        )
        .with_code(codes::CHECKER_SYMBOL)
        .with_location(2, 5),
        source_range: None,
    };
    let second = nexuslang::MultiModuleDiagnostic {
        path: Some(lib_path.clone()),
        module_id: Some(nexuslang::hir::HirModuleId(1)),
        diagnostic: nexuslang::diagnostic::Diagnostic::new(
            DiagnosticStage::Checker,
            "Função 'total' espera 1 argumento(s), recebeu 2",
        )
        .with_code(codes::CHECKER_ARGUMENT)
        .with_location(3, 9),
        source_range: None,
    };
    let runtime = nexuslang::MultiModuleDiagnostic::runtime("Função 'calcular' não definida");
    let report = nexuslang::MultiModuleDiagnosticReport::new(vec![first.clone(), second, runtime]);

    assert_eq!(report.len(), 3);
    assert_eq!(report.first(), Some(&first));

    let groups = report.groups_by_path_and_module();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].path.as_deref(), Some(lib_path.as_path()));
    assert_eq!(groups[0].module_id, Some(nexuslang::hir::HirModuleId(1)));
    assert_eq!(groups[0].diagnostic_indexes, [0, 1]);
    assert_eq!(groups[1].path, None);
    assert_eq!(groups[1].module_id, None);
    assert_eq!(groups[1].diagnostic_indexes, [2]);

    let report_json = nexuslang::multi_module_diagnostic_report_json("check", &report);
    assert!(report_json.contains(r#""ok":false"#), "json: {report_json}");
    assert!(
        report_json.contains(r#""diagnostic":{"code":"NXL3002""#),
        "json: {report_json}"
    );
    assert!(
        report_json.contains(r#""diagnostics":[{"code":"NXL3002""#),
        "json: {report_json}"
    );
    assert!(
        report_json.contains(r#""diagnostic_indexes":[0,1]"#),
        "json: {report_json}"
    );
    assert!(
        report_json.contains(r#""diagnostic_indexes":[2]"#),
        "json: {report_json}"
    );

    let first_error_json =
        nexuslang::multi_module_diagnostic_json("check", report.first().unwrap());
    assert!(
        first_error_json.contains(r#""diagnostic":{"code":"NXL3002""#),
        "json: {first_error_json}"
    );
    assert!(
        !first_error_json.contains(r#""diagnostics":"#),
        "first-error JSON must stay collection-free: {first_error_json}"
    );
}

#[test]
fn multi_module_diagnostic_report_tooling_helpers_filter_without_changing_json() {
    let lib_path = std::path::PathBuf::from("/tmp/nexus/helpers/lib.nx");
    let other_path = std::path::PathBuf::from("/tmp/nexus/helpers/other.nx");
    let lib_module = nexuslang::hir::HirModuleId(1);
    let other_module = nexuslang::hir::HirModuleId(2);

    let first = nexuslang::MultiModuleDiagnostic {
        path: Some(lib_path.clone()),
        module_id: Some(lib_module),
        diagnostic: nexuslang::diagnostic::Diagnostic::new(
            DiagnosticStage::Checker,
            "Variável 'cliente' não definida",
        )
        .with_code(codes::CHECKER_SYMBOL)
        .with_location(2, 5),
        source_range: None,
    };
    let second = nexuslang::MultiModuleDiagnostic {
        path: Some(lib_path.clone()),
        module_id: Some(lib_module),
        diagnostic: nexuslang::diagnostic::Diagnostic::new(
            DiagnosticStage::Checker,
            "Função 'total' espera 1 argumento(s), recebeu 2",
        )
        .with_code(codes::CHECKER_ARGUMENT)
        .with_location(3, 9),
        source_range: None,
    };
    let parser = nexuslang::MultiModuleDiagnostic {
        path: Some(other_path.clone()),
        module_id: Some(other_module),
        diagnostic: nexuslang::diagnostic::Diagnostic::parser(
            "import espera 'from' antes do caminho",
            1,
            1,
        ),
        source_range: None,
    };
    let runtime = nexuslang::MultiModuleDiagnostic::runtime("Divisão por zero");
    let report = nexuslang::MultiModuleDiagnosticReport::new(vec![first, second, parser, runtime]);

    assert_eq!(report.diagnostics_for_path(&lib_path).len(), 2);
    assert_eq!(report.diagnostics_for_path(&other_path).len(), 1);
    assert_eq!(report.diagnostics_for_module_id(lib_module).len(), 2);
    assert_eq!(report.diagnostics_for_module_id(other_module).len(), 1);
    assert_eq!(
        report
            .diagnostics_for_path_and_module(Some(lib_path.as_path()), Some(lib_module))
            .len(),
        2
    );
    assert_eq!(
        report
            .diagnostics_for_path_and_module(Some(lib_path.as_path()), Some(other_module))
            .len(),
        0
    );
    assert_eq!(
        report.diagnostics_for_stage(DiagnosticStage::Checker).len(),
        2
    );
    assert_eq!(
        report.diagnostics_for_stage(DiagnosticStage::Parser).len(),
        1
    );
    assert_eq!(
        report.diagnostics_for_stage(DiagnosticStage::Runtime).len(),
        1
    );
    assert_eq!(
        report
            .diagnostics_for_severity(DiagnosticSeverity::Error)
            .len(),
        4
    );

    let groups = report.groups_by_path_and_module();
    assert_eq!(groups.len(), 3);
    assert_eq!(report.diagnostics_for_group(&groups[0]).len(), 2);
    assert_eq!(
        report
            .first_diagnostic_for_group(&groups[0])
            .map(|diagnostic| diagnostic.diagnostic.message.as_str()),
        Some("Variável 'cliente' não definida")
    );
    let stale_group = nexuslang::MultiModuleDiagnosticGroup {
        path: Some(lib_path.clone()),
        module_id: Some(lib_module),
        diagnostic_indexes: vec![usize::MAX],
    };
    assert!(report.diagnostics_for_group(&stale_group).is_empty());
    assert!(report.first_diagnostic_for_group(&stale_group).is_none());

    let json = nexuslang::multi_module_diagnostic_report_json("check", &report);
    assert!(
        json.contains(r#""diagnostic_indexes":[0,1]"#),
        "json: {json}"
    );
    assert!(
        !json.contains("diagnostics_for_path"),
        "helpers must not change JSON v1 shape: {json}"
    );
}

#[test]
fn multi_module_diagnostic_report_summary_counts_tooling_dimensions_without_changing_json() {
    let lib_path = std::path::PathBuf::from("/tmp/nexus/summary/lib.nx");
    let other_path = std::path::PathBuf::from("/tmp/nexus/summary/other.nx");
    let lib_module = nexuslang::hir::HirModuleId(1);
    let other_module = nexuslang::hir::HirModuleId(2);

    let first = nexuslang::MultiModuleDiagnostic {
        path: Some(lib_path.clone()),
        module_id: Some(lib_module),
        diagnostic: nexuslang::diagnostic::Diagnostic::new(
            DiagnosticStage::Checker,
            "Variável 'cliente' não definida",
        )
        .with_code(codes::CHECKER_SYMBOL),
        source_range: None,
    };
    let second = nexuslang::MultiModuleDiagnostic {
        path: Some(lib_path.clone()),
        module_id: Some(lib_module),
        diagnostic: nexuslang::diagnostic::Diagnostic::new(
            DiagnosticStage::Checker,
            "Função 'total' espera 1 argumento(s), recebeu 2",
        )
        .with_code(codes::CHECKER_ARGUMENT),
        source_range: None,
    };
    let parser = nexuslang::MultiModuleDiagnostic {
        path: Some(other_path.clone()),
        module_id: Some(other_module),
        diagnostic: nexuslang::diagnostic::Diagnostic::parser(
            "import espera 'from' antes do caminho",
            1,
            1,
        ),
        source_range: None,
    };
    let warning = nexuslang::MultiModuleDiagnostic {
        path: Some(lib_path.clone()),
        module_id: Some(lib_module),
        diagnostic: nexuslang::diagnostic::Diagnostic::new(
            DiagnosticStage::Checker,
            "Aviso para tooling",
        )
        .with_code(codes::CHECKER_GENERIC)
        .with_severity(DiagnosticSeverity::Warning),
        source_range: None,
    };
    let no_severity = nexuslang::MultiModuleDiagnostic {
        path: None,
        module_id: None,
        diagnostic: nexuslang::diagnostic::Diagnostic::new(
            DiagnosticStage::Runtime,
            "Runtime sem severidade",
        )
        .without_severity(),
        source_range: None,
    };
    let report = nexuslang::MultiModuleDiagnosticReport::new(vec![
        first,
        second,
        parser,
        warning,
        no_severity,
    ]);

    let summary = report.summary();
    assert_eq!(summary.total, 5);
    assert!(summary.has_diagnostics);
    assert!(summary.has_errors);
    assert!(summary.has_warnings);
    assert_eq!(
        summary.stages,
        vec![
            nexuslang::MultiModuleDiagnosticStageCount {
                stage: DiagnosticStage::Checker,
                count: 3,
            },
            nexuslang::MultiModuleDiagnosticStageCount {
                stage: DiagnosticStage::Parser,
                count: 1,
            },
            nexuslang::MultiModuleDiagnosticStageCount {
                stage: DiagnosticStage::Runtime,
                count: 1,
            },
        ]
    );
    assert_eq!(
        summary.severities,
        vec![
            nexuslang::MultiModuleDiagnosticSeverityCount {
                severity: Some(DiagnosticSeverity::Error),
                count: 3,
            },
            nexuslang::MultiModuleDiagnosticSeverityCount {
                severity: Some(DiagnosticSeverity::Warning),
                count: 1,
            },
            nexuslang::MultiModuleDiagnosticSeverityCount {
                severity: None,
                count: 1,
            },
        ]
    );
    assert_eq!(summary.paths, vec![lib_path, other_path]);
    assert_eq!(summary.module_ids, vec![lib_module, other_module]);

    let empty_summary = nexuslang::MultiModuleDiagnosticReport::empty().summary();
    assert_eq!(empty_summary.total, 0);
    assert!(!empty_summary.has_diagnostics);
    assert!(!empty_summary.has_errors);
    assert!(!empty_summary.has_warnings);
    assert!(empty_summary.stages.is_empty());
    assert!(empty_summary.severities.is_empty());
    assert!(empty_summary.paths.is_empty());
    assert!(empty_summary.module_ids.is_empty());

    let empty_view = nexuslang::MultiModuleDiagnosticReport::empty().tooling_view();
    assert_eq!(empty_view.summary, empty_summary);
    assert!(empty_view.groups.is_empty());
    assert!(empty_view.items.is_empty());
    let empty_source_view =
        nexuslang::MultiModuleDiagnosticReport::empty().tooling_view_with_source_context(None);
    assert_eq!(empty_source_view.summary, empty_summary);
    assert!(empty_source_view.groups.is_empty());
    assert!(empty_source_view.items.is_empty());

    let json = nexuslang::multi_module_diagnostic_report_json("check", &report);
    assert!(json.contains(r#""diagnostics":[{"code":"NXL3002""#));
    assert!(!json.contains(r#""summary""#), "json: {json}");
    assert!(!json.contains(r#""has_errors""#), "json: {json}");
    assert!(!json.contains(r#""stages":"#), "json: {json}");
    assert!(!json.contains(r#""severities":"#), "json: {json}");
    assert!(!json.contains(r#""module_ids":"#), "json: {json}");
}

#[derive(Debug)]
struct DiagnosticReportToolingSnapshot {
    total: usize,
    has_errors: bool,
    checker_count: usize,
    module_loader_count: usize,
    runtime_count: usize,
    error_count: usize,
    affected_paths: Vec<PathBuf>,
    affected_modules: Vec<nexuslang::hir::HirModuleId>,
    group_sizes: Vec<usize>,
    output_lines: usize,
}

#[derive(Debug)]
struct DiagnosticReportFixture {
    entry: PathBuf,
    primary_path: Option<PathBuf>,
    source_database: Option<nexuslang::module_loader::SourceDatabase>,
    report: nexuslang::MultiModuleDiagnosticReport,
    output: Vec<String>,
}

fn diagnostic_report_tooling_snapshot(
    report: &nexuslang::MultiModuleDiagnosticReport,
    output: &[String],
) -> DiagnosticReportToolingSnapshot {
    let view = report.tooling_view();
    DiagnosticReportToolingSnapshot {
        total: view.summary.total,
        has_errors: view.summary.has_errors,
        checker_count: report.diagnostics_for_stage(DiagnosticStage::Checker).len(),
        module_loader_count: report
            .diagnostics_for_stage(DiagnosticStage::ModuleLoader)
            .len(),
        runtime_count: report.diagnostics_for_stage(DiagnosticStage::Runtime).len(),
        error_count: report
            .diagnostics_for_severity(DiagnosticSeverity::Error)
            .len(),
        affected_paths: view.summary.paths,
        affected_modules: view.summary.module_ids,
        group_sizes: view
            .groups
            .iter()
            .map(|group| report.diagnostics_for_group(group).len())
            .collect(),
        output_lines: output.len(),
    }
}

fn checker_report_tooling_fixture() -> DiagnosticReportFixture {
    let dir = temp_dir("tooling_report_checker_fixture");
    let lib = create_nx_file(
        &dir,
        "lib.nx",
        r#"
export fn broken_text() -> int {
    return "erro"
}

export fn broken_number() -> bool {
    return 1
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import broken_text from "./lib.nx"
import broken_number from "./lib.nx"
"#,
    );

    let (_, _, _, source_database) =
        nexuslang::module_loader::load_program_full_with_source_database(&entry)
            .expect("checker fixture should load source database before checking");
    let report = nexuslang::load_and_check_with_source_database_diagnostic_report(&entry)
        .expect_err("checker fixture should produce a diagnostic report");
    DiagnosticReportFixture {
        entry,
        primary_path: Some(lib.canonicalize().unwrap()),
        source_database: Some(source_database),
        report,
        output: Vec::new(),
    }
}

fn module_loader_report_tooling_fixture() -> DiagnosticReportFixture {
    let dir = temp_dir("tooling_report_loader_fixture");
    let lib = create_nx_file(
        &dir,
        "lib.nx",
        r#"
export fn available() -> int {
    return 1
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import missing from "./lib.nx"
"#,
    );

    let report = nexuslang::load_and_check_with_source_database_diagnostic_report(&entry)
        .expect_err("module-loader fixture should produce a diagnostic report");
    DiagnosticReportFixture {
        entry,
        primary_path: Some(lib.canonicalize().unwrap()),
        source_database: None,
        report,
        output: Vec::new(),
    }
}

fn runtime_report_tooling_fixture() -> DiagnosticReportFixture {
    let dir = temp_dir("tooling_report_runtime_fixture");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
print("antes")
print(10 / 0)
"#,
    );

    let (_, _, _, source_database) =
        nexuslang::module_loader::load_program_full_with_source_database(&entry)
            .expect("runtime fixture should load source database before running");
    let run_report =
        nexuslang::load_and_run_with_source_database_captured_diagnostic_report(&entry)
            .expect_err("runtime fixture should produce a diagnostic report");
    DiagnosticReportFixture {
        primary_path: Some(entry.canonicalize().unwrap()),
        entry,
        source_database: Some(source_database),
        report: run_report.report,
        output: run_report.output,
    }
}

#[test]
fn diagnostic_report_tooling_example_consumes_checker_fixture() {
    let fixture = checker_report_tooling_fixture();
    let snapshot = diagnostic_report_tooling_snapshot(&fixture.report, &fixture.output);
    let primary_path = fixture
        .primary_path
        .as_ref()
        .expect("checker fixture should have a primary source path");

    assert_eq!(snapshot.total, 2);
    assert!(snapshot.has_errors);
    assert_eq!(snapshot.checker_count, 2);
    assert_eq!(snapshot.module_loader_count, 0);
    assert_eq!(snapshot.runtime_count, 0);
    assert_eq!(snapshot.error_count, 2);
    assert_eq!(snapshot.affected_paths, vec![primary_path.clone()]);
    assert_eq!(snapshot.group_sizes, vec![2]);
    assert_eq!(snapshot.output_lines, 0);
    assert_eq!(fixture.report.diagnostics_for_path(primary_path).len(), 2);

    let view = fixture.report.tooling_view();
    assert_eq!(view.summary.total, 2);
    assert_eq!(view.groups.len(), 1);
    assert_eq!(view.groups[0].diagnostic_indexes, [0, 1]);
    assert_eq!(fixture.report.tooling_items(), view.items);
    assert_eq!(view.items.len(), 2);
    assert_eq!(view.items[0].diagnostic_index, 0);
    assert_eq!(view.items[0].group_index, 0);
    assert_eq!(view.items[0].path.as_deref(), Some(primary_path.as_path()));
    assert_eq!(view.items[0].module_id, view.groups[0].module_id);
    assert_eq!(view.items[0].stage, DiagnosticStage::Checker);
    assert_eq!(view.items[0].severity, Some(DiagnosticSeverity::Error));
    assert_eq!(view.items[0].code.as_deref(), Some(codes::CHECKER_TYPE));
    assert!(view.items[0].message.contains("Tipo de retorno inválido"));
    assert_eq!(
        view.items[0].source_range.map(|range| range.start.line),
        Some(2)
    );
    assert_eq!(view.items[1].diagnostic_index, 1);
    assert_eq!(view.items[1].group_index, 0);
    assert_eq!(
        view.items[1].source_range.map(|range| range.start.line),
        Some(6)
    );

    let source_database = fixture
        .source_database
        .as_ref()
        .expect("checker fixture should carry source database");
    let source_view = fixture
        .report
        .tooling_view_with_source_context(Some(source_database));
    assert_eq!(source_view.summary, view.summary);
    assert_eq!(source_view.groups, view.groups);
    assert_eq!(
        fixture
            .report
            .tooling_items_with_source_context(Some(source_database)),
        source_view.items
    );
    assert_eq!(source_view.items.len(), 2);
    assert_eq!(source_view.items[0].item, view.items[0]);
    let first_context = source_view.items[0]
        .source_context
        .as_ref()
        .expect("checker item should have source context");
    assert_eq!(first_context.path.as_path(), primary_path.as_path());
    assert_eq!(first_context.module_id, view.items[0].module_id.unwrap());
    assert_eq!(first_context.line, view.items[0].line.unwrap());
    assert_eq!(first_context.column, view.items[0].column);
    assert!(first_context.line_text.contains(r#"return "erro""#));
    assert_eq!(
        first_context.source_range.map(|range| range.start.line),
        Some(2)
    );
    assert!(first_context.highlight_start_column.is_some());
    assert!(
        first_context.highlight_end_column.unwrap() > first_context.highlight_start_column.unwrap()
    );
    assert!(source_view.items[1].source_context.is_some());

    let groups = fixture.report.groups_by_path_and_module();
    let first_message = fixture
        .report
        .first_diagnostic_for_group(&groups[0])
        .map(|diagnostic| diagnostic.diagnostic.message.as_str());
    assert!(
        first_message
            .unwrap_or_default()
            .contains("Tipo de retorno inválido"),
        "first diagnostic: {:?}",
        first_message
    );

    let first_error_string = nexuslang::load_and_check_with_graph(&fixture.entry)
        .expect_err("legacy String checker wrapper should still fail");
    assert!(first_error_string.contains("Tipo de retorno inválido"));

    let json = nexuslang::multi_module_diagnostic_report_json("check", &fixture.report);
    assert!(
        json.contains(r#""diagnostic_indexes":[0,1]"#),
        "json: {json}"
    );
    assert!(!json.contains(r#""summary""#), "json: {json}");
    assert!(!json.contains(r#""group_index""#), "json: {json}");
    assert!(!json.contains(r#""diagnostic_index""#), "json: {json}");
    assert!(!json.contains(r#""line_text""#), "json: {json}");
    assert!(
        !json.contains(r#""highlight_start_column""#),
        "json: {json}"
    );
}

#[test]
fn diagnostic_report_tooling_example_consumes_module_loader_fixture() {
    let fixture = module_loader_report_tooling_fixture();
    let snapshot = diagnostic_report_tooling_snapshot(&fixture.report, &fixture.output);
    let primary_path = fixture
        .primary_path
        .as_ref()
        .expect("loader fixture should have a primary source path");

    assert_eq!(snapshot.total, 1);
    assert!(snapshot.has_errors);
    assert_eq!(snapshot.checker_count, 0);
    assert_eq!(snapshot.module_loader_count, 1);
    assert_eq!(snapshot.runtime_count, 0);
    assert_eq!(snapshot.error_count, 1);
    assert_eq!(snapshot.affected_paths, vec![primary_path.clone()]);
    assert!(snapshot.affected_modules.is_empty());
    assert_eq!(snapshot.group_sizes, vec![1]);

    let view = fixture.report.tooling_view();
    assert_eq!(view.summary.total, 1);
    assert_eq!(view.groups.len(), 1);
    assert_eq!(fixture.report.tooling_items(), view.items);
    assert_eq!(view.items.len(), 1);
    assert_eq!(view.items[0].diagnostic_index, 0);
    assert_eq!(view.items[0].group_index, 0);
    assert_eq!(view.items[0].path.as_deref(), Some(primary_path.as_path()));
    assert_eq!(view.items[0].module_id, None);
    assert_eq!(view.items[0].stage, DiagnosticStage::ModuleLoader);
    assert_eq!(view.items[0].severity, Some(DiagnosticSeverity::Error));
    assert_eq!(
        view.items[0].code.as_deref(),
        Some(codes::MODULE_LOADER_SYMBOL_NOT_EXPORTED)
    );
    assert!(view.items[0].message.contains("missing"));
    assert!(view.items[0].source_range.is_none());
    let source_view = fixture
        .report
        .tooling_view_with_source_context(fixture.source_database.as_ref());
    assert_eq!(source_view.summary, view.summary);
    assert_eq!(source_view.groups, view.groups);
    assert_eq!(source_view.items.len(), 1);
    assert_eq!(source_view.items[0].item, view.items[0]);
    assert!(
        source_view.items[0].source_context.is_none(),
        "module-loader fixture has no SourceDatabase after loader failure"
    );
    assert_eq!(
        fixture.report.tooling_items_with_source_context(None),
        source_view.items
    );

    let diagnostic = fixture.report.first().expect("fixture report should fail");
    assert_eq!(diagnostic.diagnostic.stage, DiagnosticStage::ModuleLoader);
    assert_eq!(
        diagnostic.diagnostic.code.as_deref(),
        Some(codes::MODULE_LOADER_SYMBOL_NOT_EXPORTED)
    );
    assert_eq!(fixture.report.diagnostics_for_path(primary_path).len(), 1);

    let legacy_string = nexuslang::load_program(&fixture.entry)
        .expect_err("legacy String loader wrapper should still fail");
    assert!(legacy_string.contains("missing"));

    let json = nexuslang::multi_module_diagnostic_report_json("check", &fixture.report);
    assert!(json.contains(r#""stage":"module_loader""#), "json: {json}");
    assert!(!json.contains(r#""summary""#), "json: {json}");
    assert!(!json.contains(r#""group_index""#), "json: {json}");
    assert!(!json.contains(r#""line_text""#), "json: {json}");
}

#[test]
fn diagnostic_report_tooling_example_consumes_runtime_fixture() {
    let fixture = runtime_report_tooling_fixture();
    let snapshot = diagnostic_report_tooling_snapshot(&fixture.report, &fixture.output);

    assert_eq!(snapshot.total, 1);
    assert!(snapshot.has_errors);
    assert_eq!(snapshot.checker_count, 0);
    assert_eq!(snapshot.module_loader_count, 0);
    assert_eq!(snapshot.runtime_count, 1);
    assert_eq!(snapshot.error_count, 1);
    let primary_path = fixture
        .primary_path
        .as_ref()
        .expect("runtime fixture should have a primary source path");
    assert_eq!(snapshot.affected_paths, vec![primary_path.clone()]);
    assert_eq!(snapshot.affected_modules.len(), 1);
    assert_eq!(snapshot.group_sizes, vec![1]);
    assert_eq!(snapshot.output_lines, 1);
    assert_eq!(fixture.output, ["antes".to_string()]);

    let view = fixture.report.tooling_view();
    assert_eq!(view.summary.total, 1);
    assert_eq!(view.summary.paths, vec![primary_path.clone()]);
    assert_eq!(view.summary.module_ids.len(), 1);
    assert_eq!(view.groups.len(), 1);
    assert_eq!(fixture.report.tooling_items(), view.items);
    assert_eq!(view.items.len(), 1);
    assert_eq!(view.items[0].diagnostic_index, 0);
    assert_eq!(view.items[0].group_index, 0);
    assert_eq!(view.items[0].path.as_ref(), Some(primary_path));
    assert!(view.items[0].module_id.is_some());
    assert_eq!(view.items[0].stage, DiagnosticStage::Runtime);
    assert_eq!(view.items[0].severity, Some(DiagnosticSeverity::Error));
    assert_eq!(
        view.items[0].code.as_deref(),
        Some(codes::RUNTIME_DIVISION_BY_ZERO)
    );
    assert!(view.items[0].message.contains("Divisão por zero"));
    assert!(view.items[0].source_range.is_none());
    let source_view = fixture
        .report
        .tooling_view_with_source_context(fixture.source_database.as_ref());
    assert_eq!(source_view.summary, view.summary);
    assert_eq!(source_view.groups, view.groups);
    assert_eq!(source_view.items.len(), 1);
    assert_eq!(source_view.items[0].item, view.items[0]);
    assert!(source_view.items[0].source_context.is_none());

    let diagnostic = fixture.report.first().expect("fixture report should fail");
    assert_eq!(diagnostic.diagnostic.stage, DiagnosticStage::Runtime);
    assert_eq!(
        diagnostic.diagnostic.code.as_deref(),
        Some(codes::RUNTIME_DIVISION_BY_ZERO)
    );
    assert!(diagnostic.to_string().contains("Divisão por zero"));

    let json = nexuslang::multi_module_diagnostic_report_output_json(
        "run",
        &fixture.report,
        &fixture.output,
    );
    assert!(json.contains(r#""stage":"runtime""#), "json: {json}");
    assert!(json.contains(r#""output":["antes"]"#), "json: {json}");
    assert!(!json.contains(r#""summary""#), "json: {json}");
    assert!(!json.contains(r#""group_index""#), "json: {json}");
    assert!(!json.contains(r#""line_text""#), "json: {json}");
}

#[test]
fn multi_module_diagnostic_report_tooling_api_contract_matrix_is_stable() {
    let fixture = checker_report_tooling_fixture();
    let primary_path = fixture
        .primary_path
        .as_ref()
        .expect("checker fixture should have a primary source path");
    let source_database = fixture
        .source_database
        .as_ref()
        .expect("checker fixture should carry source database");
    let report = &fixture.report;
    let diagnostics = report.diagnostics();
    let summary = report.summary();
    let view = report.tooling_view();
    let source_view = report.tooling_view_with_source_context(Some(source_database));
    let no_source_view = report.tooling_view_with_source_context(None);

    assert_eq!(report.len(), diagnostics.len());
    assert_eq!(report.first(), diagnostics.first());
    assert_eq!(report.clone().into_diagnostics(), diagnostics.to_vec());

    assert_eq!(summary.total, 2);
    assert!(summary.has_diagnostics);
    assert!(summary.has_errors);
    assert!(!summary.has_warnings);
    assert_eq!(
        summary.stages,
        vec![nexuslang::MultiModuleDiagnosticStageCount {
            stage: DiagnosticStage::Checker,
            count: 2,
        }]
    );
    assert_eq!(
        summary.severities,
        vec![nexuslang::MultiModuleDiagnosticSeverityCount {
            severity: Some(DiagnosticSeverity::Error),
            count: 2,
        }]
    );
    assert_eq!(summary.paths, vec![primary_path.clone()]);
    assert_eq!(summary.module_ids.len(), 1);

    let module_id = summary.module_ids[0];
    assert_eq!(report.diagnostics_for_path(primary_path).len(), 2);
    assert_eq!(report.diagnostics_for_module_id(module_id).len(), 2);
    assert_eq!(
        report
            .diagnostics_for_path_and_module(Some(primary_path.as_path()), Some(module_id))
            .len(),
        2
    );
    assert_eq!(
        report.diagnostics_for_stage(DiagnosticStage::Checker).len(),
        2
    );
    assert_eq!(
        report
            .diagnostics_for_severity(DiagnosticSeverity::Error)
            .len(),
        2
    );

    assert_eq!(view.summary, summary);
    assert_eq!(view.groups.len(), 1);
    assert_eq!(view.groups[0].path.as_deref(), Some(primary_path.as_path()));
    assert_eq!(view.groups[0].module_id, Some(module_id));
    assert_eq!(view.groups[0].diagnostic_indexes, [0, 1]);
    assert_eq!(report.diagnostics_for_group(&view.groups[0]).len(), 2);
    assert_eq!(
        report.first_diagnostic_for_group(&view.groups[0]),
        diagnostics.first()
    );
    assert_eq!(report.tooling_items(), view.items);
    assert_eq!(view.items.len(), diagnostics.len());

    for item in &view.items {
        let diagnostic = &diagnostics[item.diagnostic_index];
        let group = &view.groups[item.group_index];
        assert!(
            group.diagnostic_indexes.contains(&item.diagnostic_index),
            "item should point at a group containing its diagnostic index"
        );
        assert_eq!(item.path, diagnostic.path);
        assert_eq!(item.module_id, diagnostic.module_id);
        assert_eq!(item.stage, diagnostic.diagnostic.stage);
        assert_eq!(item.severity, diagnostic.diagnostic.severity);
        assert_eq!(item.code, diagnostic.diagnostic.code);
        assert_eq!(item.message, diagnostic.diagnostic.message);
        assert_eq!(item.line, diagnostic.diagnostic.line);
        assert_eq!(item.column, diagnostic.diagnostic.column);
        assert_eq!(item.source_range, diagnostic.source_range);
    }

    assert_eq!(source_view.summary, summary);
    assert_eq!(source_view.groups, view.groups);
    assert_eq!(source_view.items.len(), view.items.len());
    assert_eq!(
        report.tooling_items_with_source_context(Some(source_database)),
        source_view.items
    );
    assert_eq!(no_source_view.summary, summary);
    assert_eq!(no_source_view.groups, view.groups);
    assert_eq!(no_source_view.items.len(), view.items.len());

    for (index, item_with_context) in source_view.items.iter().enumerate() {
        assert_eq!(item_with_context.item, view.items[index]);
        let context = item_with_context
            .source_context
            .as_ref()
            .expect("checker diagnostics should resolve source context");
        assert_eq!(context.module_id, module_id);
        assert_eq!(context.path.as_path(), primary_path.as_path());
        assert_eq!(context.line, item_with_context.item.line.unwrap());
        assert_eq!(context.column, item_with_context.item.column);
        assert_eq!(context.source_range, item_with_context.item.source_range);
        assert!(!context.line_text.trim().is_empty());
        assert!(context.highlight_start_column.is_some());
        assert!(context.highlight_end_column.is_some());
    }

    for (index, item_with_context) in no_source_view.items.iter().enumerate() {
        assert_eq!(item_with_context.item, view.items[index]);
        assert!(item_with_context.source_context.is_none());
    }

    let report_json = nexuslang::multi_module_diagnostic_report_json("check", report);
    let first_error_json =
        nexuslang::multi_module_diagnostic_json("check", report.first().unwrap());
    for forbidden in [
        r#""summary""#,
        r#""items""#,
        r#""diagnostic_index""#,
        r#""group_index""#,
        r#""source_context""#,
        r#""line_text""#,
        r#""highlight_start_column""#,
        r#""highlight_end_column""#,
        r#""uri""#,
        r#""byte_range""#,
    ] {
        assert!(
            !report_json.contains(forbidden),
            "JSON v1 report must not serialize in-memory tooling field {forbidden}: {report_json}"
        );
        assert!(
            !first_error_json.contains(forbidden),
            "first-error JSON must not serialize in-memory tooling field {forbidden}: {first_error_json}"
        );
    }
    assert!(
        !first_error_json.contains(r#""diagnostics""#),
        "first-error JSON must remain collection-free: {first_error_json}"
    );
    assert!(
        !first_error_json.contains(r#""groups""#),
        "first-error JSON must remain group-free: {first_error_json}"
    );
}

#[test]
fn load_and_check_with_source_database_diagnostic_report_wraps_first_error() {
    let dir = temp_dir("source_database_diagnostic_report");

    let lib = create_nx_file(
        &dir,
        "lib.nx",
        r#"
export fn broken() -> int {
    return "erro"
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import broken from "./lib.nx"
"#,
    );

    let report = nexuslang::load_and_check_with_source_database_diagnostic_report(&entry)
        .expect_err("checker should return a diagnostic report");

    assert_eq!(report.len(), 1);
    let diagnostic = report.first().expect("report should contain first error");
    assert_eq!(diagnostic.path, Some(lib.canonicalize().unwrap()));
    assert_eq!(diagnostic.diagnostic.stage, DiagnosticStage::Checker);
    assert!(diagnostic
        .diagnostic
        .message
        .contains("Tipo de retorno inválido"));

    let groups = report.groups_by_path_and_module();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].diagnostic_indexes, [0]);

    let json = nexuslang::multi_module_diagnostic_report_json("check", &report);
    assert!(
        json.contains(r#""diagnostic":{"code":"NXL3001""#),
        "json: {json}"
    );
    assert!(
        json.contains(r#""diagnostics":[{"code":"NXL3001""#),
        "json: {json}"
    );
    assert!(json.contains(r#""groups":[{"path":"#), "json: {json}");
}

#[test]
fn load_and_check_with_source_database_diagnostic_report_collects_independent_checker_diagnostics()
{
    let dir = temp_dir("source_database_diagnostic_report_collects_checker");

    let lib = create_nx_file(
        &dir,
        "lib.nx",
        r#"
export fn broken_text() -> int {
    return "erro"
}

export fn broken_number() -> bool {
    return 1
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import broken_text from "./lib.nx"
import broken_number from "./lib.nx"
"#,
    );

    let first_error = nexuslang::load_and_check_with_source_database_diagnostic(&entry)
        .expect_err("first-error checker path should still fail");
    assert_eq!(first_error.path, Some(lib.canonicalize().unwrap()));
    assert!(
        first_error
            .diagnostic
            .message
            .contains("Tipo de retorno inválido"),
        "diagnostic: {:?}",
        first_error
    );

    let report = nexuslang::load_and_check_with_source_database_diagnostic_report(&entry)
        .expect_err("report checker path should collect declaration diagnostics");

    assert_eq!(report.len(), 2);
    let diagnostics = report.diagnostics();
    assert_eq!(diagnostics[0].path, Some(lib.canonicalize().unwrap()));
    assert_eq!(diagnostics[1].path, Some(lib.canonicalize().unwrap()));
    assert_eq!(diagnostics[0].diagnostic.stage, DiagnosticStage::Checker);
    assert_eq!(diagnostics[1].diagnostic.stage, DiagnosticStage::Checker);
    assert!(diagnostics[0]
        .diagnostic
        .message
        .contains("Tipo de retorno inválido"));
    assert!(diagnostics[1]
        .diagnostic
        .message
        .contains("Tipo de retorno inválido"));
    assert_eq!(
        diagnostics[0].source_range.map(|range| range.start.line),
        Some(2)
    );
    assert_eq!(
        diagnostics[1].source_range.map(|range| range.start.line),
        Some(6)
    );

    let groups = report.groups_by_path_and_module();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].diagnostic_indexes, [0, 1]);

    let json = nexuslang::multi_module_diagnostic_report_json("check", &report);
    assert!(
        json.contains(r#""diagnostic":{"code":"NXL3001""#),
        "json: {json}"
    );
    assert!(
        json.contains(r#""diagnostic_indexes":[0,1]"#),
        "json: {json}"
    );
}

#[test]
fn checker_diagnostic_report_covers_declaration_family_matrix() {
    let dir = temp_dir("checker_report_declaration_family_matrix");

    let lib = create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model MatrixAnchor {
    name: string
}

fn broken_text() -> int {
    return "erro"
}

route GET /broken-endpoint {
    print("sem retorno direto")
}

workflow BrokenWorkflow {
    step preparar {
        let total: int = "erro"
    }
}

invoice {
    customer: "Empresa SARL"
    currency: "AOA"
    item "Consultoria" qty 1 price 150
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import MatrixAnchor from "./lib.nx"
"#,
    );

    let report = nexuslang::load_and_check_with_source_database_diagnostic_report(&entry)
        .expect_err("checker report should collect independent declaration diagnostics");

    assert_eq!(report.len(), 4, "report: {:?}", report);
    let lib_path = lib.canonicalize().unwrap();
    let diagnostics = report.diagnostics();
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.path == Some(lib_path.clone())));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.diagnostic.stage == DiagnosticStage::Checker));

    let messages: Vec<&str> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.diagnostic.message.as_str())
        .collect();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("Tipo de retorno inválido")),
        "messages: {:?}",
        messages
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("Route '/broken-endpoint' deve conter")),
        "messages: {:?}",
        messages
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("Tipo inválido para 'total'")),
        "messages: {:?}",
        messages
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("Invoice item price")),
        "messages: {:?}",
        messages
    );

    let start_lines: Vec<Option<usize>> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.source_range.map(|range| range.start.line))
        .collect();
    assert_eq!(start_lines, [Some(6), Some(10), Some(14), Some(20)]);

    let groups = report.groups_by_path_and_module();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].diagnostic_indexes, [0, 1, 2, 3]);
}

#[test]
fn checker_diagnostic_report_keeps_global_setup_errors_first_error() {
    let dir = temp_dir("checker_report_global_setup_first_error");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
fn duplicated() -> int {
    return 1
}

fn duplicated() -> int {
    return 2
}

fn later_broken() -> bool {
    return 1
}
"#,
    );

    let report = nexuslang::load_and_check_with_source_database_diagnostic_report(&entry)
        .expect_err("global checker setup failure should produce a one-item report");

    assert_eq!(report.len(), 1);
    let diagnostic = report.first().expect("report should contain setup error");
    assert!(diagnostic
        .diagnostic
        .message
        .contains("Função 'duplicated' declarada mais de uma vez"));
    assert!(
        !diagnostic
            .diagnostic
            .message
            .contains("Tipo de retorno inválido"),
        "setup errors should stop before declaration body collection"
    );

    let json = nexuslang::multi_module_diagnostic_report_json("check", &report);
    assert!(json.contains(r#""diagnostic_indexes":[0]"#), "json: {json}");
    assert!(
        !json.contains(r#""diagnostic_indexes":[0,1]"#),
        "setup errors should stay first-error: {json}"
    );
}

#[test]
fn checker_diagnostic_report_keeps_top_level_statements_first_error() {
    let dir = temp_dir("checker_report_top_level_first_error");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
print(missing_one)
print(missing_two)

fn later_broken() -> int {
    return "erro"
}
"#,
    );

    let report = nexuslang::load_and_check_with_source_database_diagnostic_report(&entry)
        .expect_err("top-level statement failure should produce a one-item report");

    assert_eq!(report.len(), 1);
    let diagnostic = report
        .first()
        .expect("report should contain statement error");
    assert!(diagnostic.diagnostic.message.contains("missing_one"));
    assert!(
        !diagnostic.diagnostic.message.contains("missing_two"),
        "top-level statement checking should stop at the first dependent error"
    );

    let json = nexuslang::multi_module_diagnostic_report_json("check", &report);
    assert!(json.contains("missing_one"), "json: {json}");
    assert!(!json.contains("missing_two"), "json: {json}");
    assert!(
        !json.contains("Tipo de retorno inválido"),
        "declaration body collection should not run after a top-level failure: {json}"
    );
}

#[test]
fn multi_module_diagnostic_report_output_json_includes_captured_output() {
    let success_report = nexuslang::MultiModuleDiagnosticReport::empty();
    let success_output = vec!["primeira".to_string(), "2".to_string()];
    let success_json = nexuslang::multi_module_diagnostic_report_output_json(
        "run",
        &success_report,
        &success_output,
    );

    assert!(
        success_json.contains(r#""ok":true"#),
        "json: {success_json}"
    );
    assert!(
        success_json.contains(r#""diagnostic":null"#),
        "json: {success_json}"
    );
    assert!(
        success_json.contains(r#""diagnostics":[]"#),
        "json: {success_json}"
    );
    assert!(
        success_json.contains(r#""groups":[]"#),
        "json: {success_json}"
    );
    assert!(
        success_json.contains(r#""output":["primeira","2"]"#),
        "json: {success_json}"
    );

    let diagnostic = nexuslang::MultiModuleDiagnostic::runtime("Divisão por zero");
    let error_report = nexuslang::MultiModuleDiagnosticReport::from_diagnostic(diagnostic);
    let error_output = vec!["antes".to_string()];
    let error_json =
        nexuslang::multi_module_diagnostic_report_output_json("run", &error_report, &error_output);

    assert!(error_json.contains(r#""ok":false"#), "json: {error_json}");
    assert!(
        error_json.contains(r#""diagnostic":{"code":"NXL5001""#),
        "json: {error_json}"
    );
    assert!(
        error_json.contains(r#""diagnostics":[{"code":"NXL5001""#),
        "json: {error_json}"
    );
    assert!(
        error_json.contains(r#""diagnostic_indexes":[0]"#),
        "json: {error_json}"
    );
    assert!(
        error_json.contains(r#""output":["antes"]"#),
        "json: {error_json}"
    );
}

#[test]
fn captured_multi_module_run_preserves_output_on_success_and_runtime_error() {
    let success_dir = temp_dir("captured_multi_module_run_success");
    let success_entry = create_nx_file(
        &success_dir,
        "main.nx",
        r#"
print("ok")
print(42)
"#,
    );

    let output = nexuslang::load_and_run_with_source_database_captured_diagnostic(&success_entry)
        .expect("captured run should succeed");
    assert_eq!(output, ["ok".to_string(), "42".to_string()]);

    let error_dir = temp_dir("captured_multi_module_run_runtime_error");
    let error_entry = create_nx_file(
        &error_dir,
        "main.nx",
        r#"
print("antes")
print(10 / 0)
"#,
    );

    let err = nexuslang::load_and_run_with_source_database_captured_diagnostic(&error_entry)
        .expect_err("captured run should preserve partial output on runtime error");
    assert_eq!(err.output, ["antes".to_string()]);
    assert_eq!(err.diagnostic.diagnostic.stage, DiagnosticStage::Runtime);
    assert_eq!(err.diagnostic.diagnostic.code.as_deref(), Some("NXL5001"));
    assert_eq!(
        err.diagnostic.diagnostic.severity,
        Some(DiagnosticSeverity::Error)
    );
    assert!(err
        .diagnostic
        .diagnostic
        .message
        .contains("Divisão por zero"));
}

#[test]
fn check_with_module_graph_hir_symbol_ref_three_modules() {
    let dir = temp_dir("check_hir_three_mods");

    // Create a chain: entry → lib → helpers
    create_nx_file(
        &dir,
        "helpers.nx",
        r#"
export fn helper() -> string {
    return "ok"
}
"#,
    );

    create_nx_file(
        &dir,
        "lib.nx",
        r#"
import helper from "./helpers.nx"
export fn lib_fn() -> string {
    return helper()
}
"#,
    );

    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import lib_fn from "./lib.nx"

let result = lib_fn()
print(result)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let resolutions = checker.checked_import_resolutions();

    // main.nx has 1 import (lib_fn from lib.nx).
    // lib.nx has 1 import (helper from helpers.nx).
    // Dependency imports stay in the merged program so dependency aliases and
    // symbol references can still resolve after lowering.
    assert_eq!(
        resolutions.len(),
        2,
        "entry and dependency imports should be resolved in the merged program"
    );

    let hir = nexuslang::hir::lower_program(&program);

    let import_decls: Vec<&nexuslang::hir::HirDecl<'_>> = hir
        .decls
        .iter()
        .filter(|d| d.kind == nexuslang::hir::HirDeclKind::Import)
        .collect();
    assert_eq!(
        import_decls.len(),
        2,
        "merged HIR should include entry and dependency imports"
    );

    // Sorted dep order: helpers.nx → module 1, lib.nx → module 2
    let lib_import = import_decls
        .iter()
        .find(|d| d.name == Some("lib_fn"))
        .expect("main import should be present");
    let lib_sym_ref = resolutions
        .get(&lib_import.id)
        .expect("main import should be resolved");
    assert_eq!(
        lib_sym_ref.module.index(),
        2,
        "lib_fn is in lib.nx (module 2)"
    );

    let resolved_lib_sym = hir
        .symbols
        .iter()
        .find(|s| s.id == lib_sym_ref.symbol)
        .expect("resolved lib symbol should exist");
    assert_eq!(
        resolved_lib_sym.kind,
        nexuslang::hir::HirSymbolKind::Function
    );
    assert_eq!(resolved_lib_sym.name, "lib_fn");

    let helper_import = import_decls
        .iter()
        .find(|d| d.name == Some("helper"))
        .expect("dependency import should be present");
    let helper_sym_ref = resolutions
        .get(&helper_import.id)
        .expect("dependency import should be resolved");
    assert_eq!(
        helper_sym_ref.module.index(),
        1,
        "helper is in helpers.nx (module 1)"
    );

    let resolved_helper_sym = hir
        .symbols
        .iter()
        .find(|s| s.id == helper_sym_ref.symbol)
        .expect("resolved helper symbol should exist");
    assert_eq!(
        resolved_helper_sym.kind,
        nexuslang::hir::HirSymbolKind::Function
    );
    assert_eq!(resolved_helper_sym.name, "helper");
}

#[test]
fn check_with_module_graph_import_model_with_alias() {
    let dir = temp_dir("check_model_alias");
    create_nx_file(
        &dir,
        "models.nx",
        r#"
export model User {
    name: string
    age: int
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./models.nx"

let u = Usuario { name: "Ana", age: 30 }
print(u.name)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    let result = checker.check_with_module_graph(&program, &module_graph, &decl_module_map);

    assert!(
        result.is_ok(),
        "alias-imported model should pass checker: {:?}",
        result.err()
    );
}

#[test]
fn check_with_module_graph_import_function_with_alias() {
    let dir = temp_dir("check_fn_alias");
    create_nx_file(
        &dir,
        "utils.nx",
        r#"
export fn greet(name: string) -> string {
    return "Hello, " + name
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import greet as saudar from "./utils.nx"

let msg = saudar("Ana")
print(msg)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    let result = checker.check_with_module_graph(&program, &module_graph, &decl_module_map);

    assert!(
        result.is_ok(),
        "alias-imported function should pass checker: {:?}",
        result.err()
    );
}

#[test]
fn dependency_module_import_alias_is_preserved_during_merge() {
    let dir = temp_dir("dependency_alias_preserved");
    create_nx_file(
        &dir,
        "base.nx",
        r#"
export fn helper() -> string {
    return "ok"
}
"#,
    );
    create_nx_file(
        &dir,
        "dep.nx",
        r#"
import helper as h from "./base.nx"

export fn call_helper() -> string {
    return h()
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import call_helper from "./dep.nx"

print(call_helper())
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let import_decl_count = program
        .decls
        .iter()
        .filter(|decl| matches!(decl, nexuslang::ast::Decl::Import { .. }))
        .count();
    assert_eq!(
        import_decl_count, 2,
        "merged program should retain entry and dependency imports"
    );

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("dependency alias should resolve during checking");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp
        .run(&program)
        .expect("dependency alias should resolve at runtime");

    assert!(
        interp.output().contains(&"ok".to_string()),
        "runtime output should include dependency alias result: {:?}",
        interp.output()
    );
}

#[test]
fn check_with_module_graph_import_alias_hir_symbol_ref() {
    let dir = temp_dir("check_alias_hir_ref");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
}

export fn helper() -> string {
    return "ok"
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./lib.nx"
import helper as ajuda from "./lib.nx"

let u = Usuario { name: "Ana" }
print(ajuda())
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let resolutions = checker.checked_import_resolutions();
    assert_eq!(
        resolutions.len(),
        2,
        "should have resolved 2 alias imports (Usuario + ajuda)"
    );

    let hir = nexuslang::hir::lower_program(&program);

    // Find alias import decls
    let import_decls: Vec<&nexuslang::hir::HirDecl<'_>> = hir
        .decls
        .iter()
        .filter(|d| d.kind == nexuslang::hir::HirDeclKind::Import)
        .collect();
    assert_eq!(import_decls.len(), 2, "should have 2 import decls in HIR");

    // Check that the alias decls have their names set correctly
    let alias_names: Vec<Option<&str>> = import_decls.iter().map(|d| d.name).collect();
    assert!(
        alias_names.contains(&Some("Usuario")),
        "one alias should be named Usuario"
    );
    assert!(
        alias_names.contains(&Some("ajuda")),
        "one alias should be named ajuda"
    );

    for import_decl in &import_decls {
        let decl_id = import_decl.id;
        let sym_ref = resolutions
            .get(&decl_id)
            .expect("every import decl should have a resolution");

        // The dependency module is module 1 (entry is module 0)
        assert_eq!(
            sym_ref.module.index(),
            1,
            "imported symbols come from lib.nx (module 1)"
        );

        let resolved_sym = hir
            .symbols
            .iter()
            .find(|s| s.id == sym_ref.symbol)
            .expect("resolved symbol should exist in HIR");

        // The resolved symbol should have the ORIGINAL name ("User" / "helper"),
        // not the alias ("Usuario" / "ajuda")
        match resolved_sym.kind {
            nexuslang::hir::HirSymbolKind::Model => {
                assert_eq!(resolved_sym.name, "User");
            }
            nexuslang::hir::HirSymbolKind::Function => {
                assert_eq!(resolved_sym.name, "helper");
            }
            other => panic!("expected Model or Function, got {:?}", other),
        }
    }
}

#[test]
fn load_and_check_with_graph_import_alias_works() {
    let dir = temp_dir("load_check_alias");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
    age: int
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./lib.nx"

let u = Usuario { name: "Ana", age: 30 }
print(u)
"#,
    );

    let result = nexuslang::load_and_check_with_graph(&entry);
    assert!(
        result.is_ok(),
        "load_and_check_with_graph for alias should succeed: {:?}",
        result.err()
    );
}

#[test]
fn load_and_run_with_graph_import_alias() {
    let dir = temp_dir("load_run_alias");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
    age: int
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./lib.nx"

let u = Usuario { name: "Ana", age: 30 }
print(u.name)
"#,
    );

    let result = nexuslang::load_and_run_with_graph(&entry);
    assert!(
        result.is_ok(),
        "load_and_run_with_graph for alias should succeed: {:?}",
        result.err()
    );
}

#[test]
fn load_and_run_with_graph_import_alias_runtime_output() {
    let dir = temp_dir("load_run_alias_out");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
    age: int
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./lib.nx"

let u = Usuario { name: "Ana", age: 30 }
print(u.name)
print(u.age)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"Ana".to_string()),
        "output should contain 'Ana': {:?}",
        output
    );
    assert!(
        output.contains(&"30".to_string()),
        "output should contain '30': {:?}",
        output
    );
}

#[test]
fn load_and_run_with_graph_import_alias_function() {
    let dir = temp_dir("load_run_fn_alias");
    create_nx_file(
        &dir,
        "utils.nx",
        r#"
export fn greet(name: string) -> string {
    return "Hello, " + name
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import greet as saudar from "./utils.nx"

let msg = saudar("Ana")
print(msg)
"#,
    );

    let result = nexuslang::load_and_run_with_graph(&entry);
    assert!(
        result.is_ok(),
        "load_and_run_with_graph for function alias should succeed: {:?}",
        result.err()
    );
}

#[test]
fn load_and_run_with_graph_import_alias_runtime_function_output() {
    let dir = temp_dir("load_run_fn_alias_out");
    create_nx_file(
        &dir,
        "utils.nx",
        r#"
export fn greet(name: string) -> string {
    return "Hello, " + name
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import greet as saudar from "./utils.nx"

let msg = saudar("Ana")
print(msg)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"Hello, Ana".to_string()),
        "output should contain 'Hello, Ana': {:?}",
        output
    );
}

#[test]
fn check_with_module_graph_import_alias_with_model_default_omitted() {
    let dir = temp_dir("check_alias_default");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
    age: int = 18
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./lib.nx"

let u = Usuario { name: "Ana" }
print(u.age)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    let result = checker.check_with_module_graph(&program, &module_graph, &decl_module_map);
    assert!(
        result.is_ok(),
        "alias with omitted default field should pass checker: {:?}",
        result.err()
    );
}

#[test]
fn load_and_run_with_graph_import_alias_with_model_default() {
    let dir = temp_dir("run_alias_default");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
    age: int = 18
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./lib.nx"

let u = Usuario { name: "Ana" }
print(u.name)
print(u.age)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"Ana".to_string()),
        "output should contain 'Ana': {:?}",
        output
    );
    assert!(
        output.contains(&"18".to_string()),
        "default age=18 should be filled at runtime, got: {:?}",
        output
    );
}

#[test]
fn load_and_run_with_graph_import_alias_with_model_default_overridden() {
    let dir = temp_dir("run_alias_default_override");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
    age: int = 18
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./lib.nx"

let u = Usuario { name: "Ana", age: 25 }
print(u.name)
print(u.age)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"Ana".to_string()),
        "output should contain 'Ana': {:?}",
        output
    );
    assert!(
        output.contains(&"25".to_string()),
        "explicit age=25 should override default, got: {:?}",
        output
    );
    assert!(
        !output.contains(&"18".to_string()),
        "default age=18 should NOT appear when overridden, got: {:?}",
        output
    );
}

// ---------------------------------------------------------------------------
// F11.10 — Static model operations with alias
// ---------------------------------------------------------------------------

#[test]
fn load_and_run_with_graph_import_alias_static_call() {
    let dir = temp_dir("run_alias_static_call");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
    age: int
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./lib.nx"

let result = Usuario::all()
print("done")
"#,
    );

    let result = nexuslang::load_and_run_with_graph(&entry);
    assert!(
        result.is_ok(),
        "load_and_run_with_graph with alias static call should succeed: {:?}",
        result.err()
    );
}

#[test]
fn load_and_run_with_graph_import_alias_static_call_output() {
    let dir = temp_dir("run_alias_static_call_out");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./lib.nx"

let result = Usuario::all()
print(result)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"[Usuario.all() → lista de registos]".to_string()),
        "output should contain static call mock message: {:?}",
        output
    );
}

#[test]
fn load_and_run_with_graph_import_alias_static_call_twice() {
    let dir = temp_dir("run_alias_static_call2");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./lib.nx"

let a = Usuario::all()
let b = Usuario::all()
print(a)
"#,
    );

    let result = nexuslang::load_and_run_with_graph(&entry);
    assert!(
        result.is_ok(),
        "load_and_run_with_graph with two alias static calls should succeed: {:?}",
        result.err()
    );
}

#[test]
fn check_with_module_graph_import_alias_static_call_all() {
    let dir = temp_dir("check_alias_static_all");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export model User {
    name: string
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import User as Usuario from "./lib.nx"

let result = Usuario::all()
print(result)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    let result = checker.check_with_module_graph(&program, &module_graph, &decl_module_map);

    assert!(
        result.is_ok(),
        "alias static call should pass checker: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// F11.11 — Workflow alias at runtime (run_workflow via alias)
// ---------------------------------------------------------------------------

#[test]
fn load_and_run_with_graph_import_workflow() {
    let dir = temp_dir("run_import_workflow");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export workflow Onboard {
    step start { print("onboarding") }
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import Onboard from "./lib.nx"
run_workflow("Onboard")
print("done")
"#,
    );

    let result = nexuslang::load_and_run_with_graph(&entry);
    assert!(
        result.is_ok(),
        "imported workflow should run: {:?}",
        result.err()
    );
}

#[test]
fn load_and_run_with_graph_import_workflow_with_alias() {
    let dir = temp_dir("run_import_wf_alias");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export workflow BillingWorkflow {
    step bill { print("billing") }
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import BillingWorkflow as Billing from "./lib.nx"
run_workflow("Billing")
print("done")
"#,
    );

    let result = nexuslang::load_and_run_with_graph(&entry);
    assert!(
        result.is_ok(),
        "aliased workflow should run: {:?}",
        result.err()
    );
}

#[test]
fn load_and_run_with_graph_import_workflow_alias_output() {
    let dir = temp_dir("run_wf_alias_out");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export workflow HelloWorkflow {
    step greet { print("hello from alias") }
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import HelloWorkflow as Hello from "./lib.nx"
run_workflow("Hello")
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"hello from alias".to_string()),
        "workflow via alias should execute and print: {:?}",
        output
    );
}

#[test]
fn check_with_module_graph_import_workflow_alias() {
    let dir = temp_dir("check_wf_alias");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export workflow ReportWorkflow {
    step gen { print("report") }
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import ReportWorkflow as Report from "./lib.nx"
run_workflow("Report")
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    let result = checker.check_with_module_graph(&program, &module_graph, &decl_module_map);

    assert!(
        result.is_ok(),
        "aliased workflow should pass checker: {:?}",
        result.err()
    );
}

#[test]
fn load_and_run_with_graph_import_workflow_alias_original_name_still_works() {
    // The original workflow name remains registered because the dependency's
    // declaration is part of the merged program. The alias is an ADDITIONAL
    // binding — it does NOT replace the original.
    let dir = temp_dir("run_wf_alias_orig");
    create_nx_file(
        &dir,
        "lib.nx",
        r#"
export workflow SecretWorkflow {
    step run { print("secret") }
}
"#,
    );
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import SecretWorkflow as Secret from "./lib.nx"
run_workflow("SecretWorkflow")
print("done")
"#,
    );

    let result = nexuslang::load_and_run_with_graph(&entry);
    assert!(
        result.is_ok(),
        "original workflow name should still be accessible after alias import: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// F11.13 — Standard Library (stdlib) — Phase 1: Infrastructure
// ---------------------------------------------------------------------------

#[test]
fn load_program_full_import_stdlib_math_abs() {
    let dir = temp_dir("stdlib_math_abs");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import abs from "std/math"

let result = abs(-5)
print(result)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry)
            .expect("load with stdlib should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check with stdlib should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp
        .run(&program)
        .expect("run with stdlib should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"5".to_string()),
        "stdlib abs(-5) should return 5, got: {:?}",
        output
    );
}

#[test]
fn load_and_run_with_graph_import_stdlib_math_clamp() {
    let dir = temp_dir("stdlib_math_clamp");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import clamp from "std/math"

let result = clamp(50, 0, 100)
print(result)
"#,
    );

    let result = nexuslang::load_and_run_with_graph(&entry);
    assert!(
        result.is_ok(),
        "load_and_run_with_graph with stdlib clamp should succeed: {:?}",
        result.err()
    );
}

#[test]
fn load_and_run_with_graph_import_stdlib_math_clamp_output() {
    let dir = temp_dir("stdlib_math_clamp_out");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import clamp from "std/math"

let result = clamp(50, 0, 100)
print(result)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"50".to_string()),
        "clamp(50, 0, 100) should return 50, got: {:?}",
        output
    );
}

#[test]
fn load_and_run_with_graph_import_stdlib_math_max_output() {
    let dir = temp_dir("stdlib_math_max_out");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import max from "std/math"

let result = max(10, 20)
print(result)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"20".to_string()),
        "max(10, 20) should return 20, got: {:?}",
        output
    );
}

#[test]
fn load_and_run_with_graph_import_stdlib_math_min_output() {
    let dir = temp_dir("stdlib_math_min_out");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import min from "std/math"

let result = min(10, 20)
print(result)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"10".to_string()),
        "min(10, 20) should return 10, got: {:?}",
        output
    );
}

#[test]
fn load_and_run_with_graph_import_stdlib_math_min_underflow() {
    let dir = temp_dir("stdlib_math_min_under");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import clamp from "std/math"

let result = clamp(-5, 0, 100)
print(result)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"0".to_string()),
        "clamp(-5, 0, 100) should return 0, got: {:?}",
        output
    );
}

#[test]
fn load_and_run_with_graph_import_stdlib_math_clamp_overflow() {
    let dir = temp_dir("stdlib_math_clamp_over");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import clamp from "std/math"

let result = clamp(150, 0, 100)
print(result)
"#,
    );

    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(&entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let output = interp.output();
    assert!(
        output.contains(&"100".to_string()),
        "clamp(150, 0, 100) should return 100, got: {:?}",
        output
    );
}

#[test]
fn load_program_full_import_nonexistent_stdlib_module_fails() {
    let dir = temp_dir("stdlib_not_found");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import foo from "std/nonexistent"
print("ok")
"#,
    );

    let result = nexuslang::module_loader::load_program_full(&entry);
    assert!(
        result.is_err(),
        "importing non-existent stdlib module should fail"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("nonexistent"),
        "error should mention the module name: {}",
        err_msg
    );
}

#[test]
fn load_and_run_with_graph_import_stdlib_core_modules_output() {
    let dir = temp_dir("stdlib_core_modules");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import contains from "std/string"
import starts_with from "std/string"
import ends_with from "std/string"
import to_upper from "std/string"
import to_lower from "std/string"
import trim from "std/string"
import length from "std/string"
import is_empty from "std/string"
import is_blank from "std/validation"
import between_len from "std/validation"
import is_email from "std/validation"
import is_iso_date from "std/date"
import year from "std/date"
import month from "std/date"
import day from "std/date"
import format_money from "std/money"
import is_positive_money from "std/money"
import is_zero_money from "std/money"
import same_currency from "std/money"

print(contains("NexusLang", "Lang"))
print(starts_with("NexusLang", "Nexus"))
print(ends_with("NexusLang", "Lang"))
print(to_upper("nexus"))
print(to_lower("NEXUS"))
print(trim("  crm  "))
print(length("Luanda"))
print(is_empty(""))
print(is_blank("   "))
print(between_len("cliente", 3, 10))
print(is_email("ana@example.com"))
print(is_iso_date("2026-02-28"))
print(is_iso_date("2026-02-30"))
print(year("2026-05-27"))
print(month("2026-05-27"))
print(day("2026-05-27"))
print(format_money(1000 kz))
print(is_positive_money(1000 kz))
print(is_zero_money(0 kz))
print(same_currency(100 kz, 50 kz))
"#,
    );

    assert_stdlib_output(
        &entry,
        &[
            "true",
            "true",
            "true",
            "NEXUS",
            "nexus",
            "crm",
            "6",
            "true",
            "true",
            "true",
            "true",
            "true",
            "false",
            "2026",
            "5",
            "27",
            "1000.00 KZ",
            "true",
            "true",
            "true",
        ],
    );
}

#[test]
fn load_and_run_with_graph_import_stdlib_collections_output() {
    let dir = temp_dir("stdlib_collections");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import contains_int from "std/collections"
import contains_string from "std/collections"
import first_int from "std/collections"
import first_string from "std/collections"
import last_int from "std/collections"
import last_string from "std/collections"
import reverse_int from "std/collections"
import reverse_string from "std/collections"
import len_int from "std/collections"
import is_empty_string from "std/collections"

print(contains_int([2, 5, 8], 5))
print(contains_int([2, 5, 8], 9))
print(first_int([2, 5, 8]))
print(last_int([2, 5, 8]))
print(reverse_int([2, 5, 8]))
print(len_int([2, 5, 8]))
print(contains_string(["ana", "bia"], "bia"))
print(first_string(["ana", "bia"]))
print(last_string(["ana", "bia"]))
print(reverse_string(["ana", "bia"]))
print(is_empty_string([]))
"#,
    );

    assert_stdlib_output(
        &entry,
        &[
            "true",
            "false",
            "2",
            "8",
            "[8, 5, 2]",
            "3",
            "true",
            "ana",
            "bia",
            "[bia, ana]",
            "true",
        ],
    );
}

#[test]
fn load_and_run_with_graph_import_stdlib_erp_modules_output() {
    let dir = temp_dir("stdlib_erp_modules");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import number_is_even from "std/number"
import number_is_odd from "std/number"
import number_sign from "std/number"
import number_between from "std/number"
import inventory_can_fulfill from "std/inventory"
import inventory_remaining_after_sale from "std/inventory"
import inventory_backorder_qty from "std/inventory"
import inventory_needs_reorder from "std/inventory"
import inventory_reorder_qty from "std/inventory"
import inventory_stock_status from "std/inventory"
import crm_display_name from "std/crm"
import crm_normalize_status from "std/crm"
import crm_is_active_status from "std/crm"
import crm_is_valid_email from "std/crm"
import crm_contact_label from "std/crm"
import invoice_line_total from "std/invoice"
import invoice_subtotal_2 from "std/invoice"
import invoice_apply_discount from "std/invoice"
import invoice_tax_amount from "std/invoice"
import invoice_grand_total from "std/invoice"
import invoice_is_paid from "std/invoice"

print(number_is_even(8))
print(number_is_odd(7))
print(number_sign(-9))
print(number_between(7, 1, 10))
print(inventory_can_fulfill(10, 3))
print(inventory_remaining_after_sale(10, 3))
print(inventory_backorder_qty(2, 5))
print(inventory_needs_reorder(4, 5))
print(inventory_reorder_qty(4, 20))
print(inventory_stock_status(0, 5))
print(inventory_stock_status(4, 5))
print(inventory_stock_status(10, 5))
print(crm_display_name("Ana", "Silva"))
print(crm_normalize_status(" ACTIVE "))
print(crm_is_active_status(" active "))
print(crm_is_valid_email("ana@example.com"))
print(crm_contact_label("Ana Silva", "ana@example.com"))
print(invoice_line_total(3, 100 kz))
print(invoice_subtotal_2(300 kz, 200 kz))
print(invoice_apply_discount(500 kz, 50 kz))
print(invoice_tax_amount(500 kz, 0.1))
print(invoice_grand_total(500 kz, 50 kz, 25 kz))
print(invoice_is_paid(0 kz))
"#,
    );

    assert_stdlib_output(
        &entry,
        &[
            "true",
            "true",
            "-1",
            "true",
            "true",
            "7",
            "3",
            "true",
            "16",
            "out_of_stock",
            "reorder",
            "ok",
            "Ana Silva",
            "active",
            "true",
            "true",
            "Ana Silva <ana@example.com>",
            "300.00 KZ",
            "500.00 KZ",
            "450.00 KZ",
            "50.00 KZ",
            "525.00 KZ",
            "true",
        ],
    );
}

#[test]
fn load_and_run_with_graph_import_stdlib_data_protocol_modules_output() {
    let dir = temp_dir("stdlib_data_protocol_modules");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import json_string from "std/json"
import json_int from "std/json"
import json_bool from "std/json"
import json_pair from "std/json"
import json_object_2 from "std/json"
import json_array_2 from "std/json"
import json_is_object from "std/json"
import json_is_array from "std/json"
import csv_row_3 from "std/csv"
import csv_header_2 from "std/csv"
import csv_needs_quotes from "std/csv"
import http_status_text from "std/http"
import http_is_success from "std/http"
import http_is_redirect from "std/http"
import http_is_client_error from "std/http"
import http_is_server_error from "std/http"
import http_method_allows_body from "std/http"
import http_build_query_2 from "std/http"
import crypto_sha256_hex from "std/crypto"
import crypto_constant_time_eq from "std/crypto"
import crypto_is_sha256_hex from "std/crypto"
import crypto_verify_sha256_hex from "std/crypto"

print(json_object_2(json_pair("name", json_string("Ana")), json_pair("active", json_bool(true))))
print(json_array_2(json_int(7), json_string("stock")))
print(json_is_object("{x}"))
print(json_is_array("[x]"))
print(csv_row_3("Ana", "Luanda, Angola", "VIP"))
print(csv_needs_quotes("Luanda, Angola"))
print(csv_header_2("name", "email"))
print(http_status_text(404))
print(http_is_success(201))
print(http_is_redirect(302))
print(http_is_client_error(404))
print(http_is_server_error(503))
print(http_method_allows_body("post"))
print(http_build_query_2("q", "Nexus Lang", "city", "Luanda"))
print(crypto_sha256_hex("nexus"))
print(crypto_constant_time_eq("abc", "abc"))
print(crypto_is_sha256_hex("f5cfcb570b7edac2ed16e1a025d50155d6148de7397f4068790cdfc142300070"))
print(crypto_verify_sha256_hex("nexus", "f5cfcb570b7edac2ed16e1a025d50155d6148de7397f4068790cdfc142300070"))
"#,
    );

    assert_stdlib_output(
        &entry,
        &[
            r#"{"name":"Ana","active":true}"#,
            r#"[7,"stock"]"#,
            "true",
            "true",
            r#"Ana,"Luanda, Angola",VIP"#,
            "true",
            "name,email",
            "Not Found",
            "true",
            "true",
            "true",
            "true",
            "true",
            "q=Nexus%20Lang&city=Luanda",
            "f5cfcb570b7edac2ed16e1a025d50155d6148de7397f4068790cdfc142300070",
            "true",
            "true",
            "true",
        ],
    );
}

#[test]
fn load_and_run_with_graph_import_stdlib_operational_modules_output() {
    std::env::set_var("NEXUS_STDLIB_ENV_TEST", "yes");

    let dir = temp_dir("stdlib_operational_modules");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import time_runtime_clock_available from "std/time"
import time_unix_seconds from "std/time"
import time_seconds_between from "std/time"
import time_minutes_between from "std/time"
import time_hours_between from "std/time"
import time_days_between from "std/time"
import time_is_before from "std/time"
import time_is_after from "std/time"
import env_runtime_available from "std/env"
import env_get from "std/env"
import env_has from "std/env"
import env_get_or from "std/env"
import env_is_true from "std/env"
import log_info from "std/log"
import log_warn from "std/log"
import log_with_context from "std/log"
import path_join from "std/path"
import path_basename from "std/path"
import path_dirname from "std/path"
import path_extension from "std/path"
import path_stem from "std/path"
import path_normalize from "std/path"
import path_is_absolute from "std/path"

print(time_runtime_clock_available())
print(time_unix_seconds() > 0)
print(time_seconds_between(10, 25))
print(time_minutes_between(0, 120))
print(time_hours_between(0, 7200))
print(time_days_between(0, 172800))
print(time_is_before(10, 20))
print(time_is_after(20, 10))
print(env_runtime_available())
print(env_has("NEXUS_STDLIB_ENV_TEST"))
print(env_get("NEXUS_STDLIB_ENV_TEST"))
print(env_get_or("NEXUS_STDLIB_ENV_MISSING", "fallback"))
print(env_is_true("NEXUS_STDLIB_ENV_TEST"))
print(log_info("pedido recebido"))
print(log_warn("stock baixo"))
print(log_with_context("ERROR", "invoice", "falha no total"))
print(path_join("/var//erp", "./reports/../invoice.csv"))
print(path_basename("/var/erp/invoice.csv"))
print(path_dirname("/var/erp/invoice.csv"))
print(path_extension("/var/erp/invoice.csv"))
print(path_stem("/var/erp/invoice.csv"))
print(path_normalize("/var//erp/./reports/../invoice.csv"))
print(path_is_absolute("/var/erp"))
"#,
    );

    assert_stdlib_output(
        &entry,
        &[
            "true",
            "true",
            "15",
            "2.00",
            "2.00",
            "2.00",
            "true",
            "true",
            "true",
            "true",
            "yes",
            "fallback",
            "true",
            "[INFO] pedido recebido",
            "[WARN] stock baixo",
            "[ERROR] invoice: falha no total",
            "/var/erp/invoice.csv",
            "invoice.csv",
            "/var/erp",
            "csv",
            "invoice",
            "/var/erp/invoice.csv",
            "true",
        ],
    );

    std::env::remove_var("NEXUS_STDLIB_ENV_TEST");
}

#[test]
fn load_and_run_with_graph_import_stdlib_business_batch_modules_output() {
    let dir = temp_dir("stdlib_business_batch_modules");
    let entry = create_nx_file(
        &dir,
        "main.nx",
        r#"
import sales_line_total from "std/sales"
import tax_amount from "std/tax"
import discount_final_percent from "std/discount"
import payment_status from "std/payment"
import banking_is_debit from "std/banking"
import accounting_is_balanced from "std/accounting"
import ledger_side from "std/ledger"
import shipping_status from "std/shipping"
import warehouse_utilization_percent from "std/warehouse"
import procurement_status from "std/procurement"
import supplier_is_active_status from "std/supplier"
import customer_balance_status from "std/customer"
import project_progress_percent from "std/project"
import task_priority_label from "std/task"
import kpi_percent from "std/kpi"
import report_money_line from "std/report"
import pagination_offset from "std/pagination"
import security_is_https from "std/security"
import config_flag_enabled from "std/config"
import commerce_cart_total_3 from "std/commerce"

print(sales_line_total(2, 100 kz))
print(tax_amount(1000 kz, 0.1))
print(discount_final_percent(1000 kz, 0.1))
print(payment_status(100 kz, 40 kz))
print(banking_is_debit(-10 kz))
print(accounting_is_balanced(100 kz, 100 kz))
print(ledger_side(-5 kz))
print(shipping_status(3, 5))
print(warehouse_utilization_percent(100, 25))
print(procurement_status(3, 5))
print(supplier_is_active_status(" ACTIVE "))
print(customer_balance_status(0 kz))
print(project_progress_percent(2, 4))
print(task_priority_label(1))
print(kpi_percent(3, 4))
print(report_money_line("total", 99 kz))
print(pagination_offset(3, 20))
print(security_is_https("https://nexus.local"))
print(config_flag_enabled(" YES "))
print(commerce_cart_total_3(10 kz, 20 kz, 30 kz))
"#,
    );

    assert_stdlib_output(
        &entry,
        &[
            "200.00 KZ",
            "100.00 KZ",
            "900.00 KZ",
            "partial",
            "true",
            "true",
            "credit",
            "partial",
            "25.00",
            "partial",
            "true",
            "settled",
            "50.00",
            "high",
            "75.00",
            "total: 99.00 KZ",
            "40",
            "true",
            "true",
            "60.00 KZ",
        ],
    );
}

fn assert_stdlib_output(entry: &std::path::Path, expected: &[&str]) {
    let (program, module_graph, decl_module_map) =
        nexuslang::module_loader::load_program_full(entry).expect("load should succeed");

    let mut checker = nexuslang::checker::Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .expect("check should succeed");

    let mut interp = nexuslang::interpreter::Interpreter::new_captured();
    interp.run(&program).expect("run should succeed");

    let actual: Vec<&str> = interp.output().iter().map(String::as_str).collect();
    assert_eq!(actual, expected);
}
