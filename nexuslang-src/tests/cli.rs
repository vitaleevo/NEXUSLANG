use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("nexuslang-{}-{}-{}", name, std::process::id(), now));
        fs::create_dir_all(&path).expect("create temp project");
        Self { path }
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn nexus() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nexus"))
}

fn crate_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn run_nexus(current_dir: &Path, args: &[&str]) -> Output {
    nexus()
        .current_dir(current_dir)
        .args(args)
        .output()
        .expect("run nexus")
}

fn assert_success(output: Output) -> String {
    if !output.status.success() {
        panic!(
            "command failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn assert_failure(output: Output) -> String {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn assert_failure_stdout(output: Output) -> String {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn cli_check_and_run_multi_module_example() {
    assert_success(run_nexus(
        crate_dir(),
        &["check", "examples/erp_basico_multi/main.nx"],
    ));

    let stdout = assert_success(run_nexus(
        crate_dir(),
        &["run", "examples/erp_basico_multi/main.nx"],
    ));
    assert!(stdout.contains("ERP Multi"), "stdout: {stdout}");
    assert!(stdout.contains("Bem-vindo"), "stdout: {stdout}");
}

#[test]
fn cli_check_and_run_stdlib_import() {
    let project = TempProject::new("stdlib-cli");
    fs::write(
        project.path.join("main.nx"),
        "import abs from \"std/math\"\nprint(abs(-2))\n",
    )
    .expect("write source");

    assert_success(run_nexus(&project.path, &["check", "main.nx"]));

    let stdout = assert_success(run_nexus(&project.path, &["run", "main.nx"]));
    assert!(stdout.lines().any(|line| line == "2"), "stdout: {stdout}");
}

#[test]
fn cli_docs_generates_markdown_for_erp_declarations() {
    let project = TempProject::new("docs-cli");
    fs::write(
        project.path.join("main.nx"),
        r#"model Customer {
    email: string unique index
    balance: money = 0 kz
}

workflow Onboarding {
    step criar {
        print("ok")
    }
}

route GET /customers/:email ?(active: bool = true) {
    return Customer::find("email", email)
}

invoice {
    customer: "Ana"
    currency: "AOA"
    item "Setup" qty 1 price 250000 kz
}
"#,
    )
    .expect("write source");

    let stdout = assert_success(run_nexus(&project.path, &["docs", "main.nx"]));
    assert!(
        stdout.contains("# NexusLang Docs: main.nx"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("## Models"), "stdout: {stdout}");
    assert!(
        stdout.contains("| email | string | unique, index | - |"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("### GET /customers/:email"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("- Query: active: bool = true"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("### Onboarding"), "stdout: {stdout}");
    assert!(stdout.contains("### Invoice 1"), "stdout: {stdout}");
}

#[test]
fn cli_docs_writes_markdown_output_file() {
    let project = TempProject::new("docs-output-cli");
    fs::write(
        project.path.join("main.nx"),
        r#"model Product {
    sku: string unique
}

route GET /products/:sku {
    return Product::find("sku", sku)
}
"#,
    )
    .expect("write source");

    let output_path = project.path.join("docs.md");
    let stdout = assert_success(run_nexus(
        &project.path,
        &["docs", "main.nx", "--output", "docs.md"],
    ));
    assert!(
        stdout.contains("Documentacao gerada: docs.md"),
        "stdout: {stdout}"
    );

    let docs = fs::read_to_string(output_path).expect("read generated docs");
    assert!(docs.contains("## Models"), "docs: {docs}");
    assert!(docs.contains("### GET /products/:sku"), "docs: {docs}");
}

#[test]
fn cli_test_runs_directory_with_multi_module_files() {
    let project = TempProject::new("test-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("helpers.nx"),
        r#"
export fn label() -> string {
    return "ok"
}
"#,
    )
    .expect("write helper");
    fs::write(
        project.path.join("tests").join("smoke.nx"),
        r#"
import label from "./helpers.nx"
print(label())
"#,
    )
    .expect("write smoke");

    let stdout = assert_success(run_nexus(&project.path, &["test", "tests"]));

    assert!(stdout.contains("Nexus tests: tests"), "stdout: {stdout}");
    assert!(stdout.contains("PASS tests/helpers.nx"), "stdout: {stdout}");
    assert!(stdout.contains("PASS tests/smoke.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains("Resultado: 2 passaram, 0 falharam, 2 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_default_prefers_tests_directory() {
    let project = TempProject::new("test-default-cli");
    fs::write(
        project.path.join("nexus.toml"),
        r#"[package]
name = "test-default-cli"
version = "0.1.0"
entry = "main.nx"

[dependencies]
"#,
    )
    .expect("write manifest");
    fs::write(project.path.join("main.nx"), "print(\"main\")\n").expect("write main");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::create_dir_all(project.path.join("examples")).expect("create examples");
    fs::write(
        project.path.join("tests").join("smoke.nx"),
        "print(\"test\")\n",
    )
    .expect("write smoke");
    fs::write(
        project.path.join("examples").join("broken.nx"),
        "print(missing_value)\n",
    )
    .expect("write broken example");

    let stdout = assert_success(run_nexus(&project.path, &["test"]));

    assert!(stdout.contains("Nexus tests:"), "stdout: {stdout}");
    assert!(stdout.contains("tests"), "stdout: {stdout}");
    assert!(stdout.contains("PASS"), "stdout: {stdout}");
    assert!(
        stdout.contains("Resultado: 1 passaram, 0 falharam, 1 total"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("broken.nx"), "stdout: {stdout}");
}

#[test]
fn cli_test_reports_failures_and_partial_output() {
    let project = TempProject::new("test-failure-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("failing.nx"),
        "print(\"before\")\nprint(10 / 0)\n",
    )
    .expect("write failing");

    let stdout = assert_failure_stdout(run_nexus(&project.path, &["test", "tests"]));

    assert!(stdout.contains("FAIL tests/failing.nx"), "stdout: {stdout}");
    assert!(stdout.contains("por zero"), "stdout: {stdout}");
    assert!(stdout.contains("output:"), "stdout: {stdout}");
    assert!(stdout.contains("before"), "stdout: {stdout}");
    assert!(
        stdout.contains("Resultado: 0 passaram, 1 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_matches_expected_err_sidecar() {
    let project = TempProject::new("test-err-sidecar-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("failing.nx"),
        "print(\"before\")\nprint(10 / 0)\n",
    )
    .expect("write failing");
    fs::write(project.path.join("tests").join("failing.out"), "before\n")
        .expect("write output sidecar");
    fs::write(
        project.path.join("tests").join("failing.err"),
        "Divisão por zero\n",
    )
    .expect("write err sidecar");

    let stdout = assert_success(run_nexus(&project.path, &["test", "tests"]));

    assert!(stdout.contains("PASS tests/failing.nx"), "stdout: {stdout}");
    assert!(
        !stdout.contains("FAIL tests/failing.nx"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Resultado: 1 passaram, 0 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_reports_expected_err_mismatch_when_program_succeeds() {
    let project = TempProject::new("test-err-missing-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke.nx"),
        "print(\"ok\")\n",
    )
    .expect("write smoke");
    fs::write(
        project.path.join("tests").join("smoke.err"),
        "Divisão por zero\n",
    )
    .expect("write err sidecar");

    let stdout = assert_failure_stdout(run_nexus(&project.path, &["test", "tests"]));

    assert!(stdout.contains("FAIL tests/smoke.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains("diagnostico diferente do esperado"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("primeira diferenca: linha 1"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("esperado: Divisão por zero"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("recebido: <sem linha>"), "stdout: {stdout}");
    assert!(stdout.contains("Divisão por zero"), "stdout: {stdout}");
    assert!(stdout.contains("<sem diagnostico>"), "stdout: {stdout}");
    assert!(
        stdout.contains("Resultado: 0 passaram, 1 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_human_report_truncates_long_err_mismatch_but_json_keeps_full_lines() {
    let project = TempProject::new("test-long-err-mismatch-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke.nx"),
        "print(\"ok\")\n",
    )
    .expect("write smoke");
    let expected_err = (0..25)
        .map(|index| format!("expected_error_{index:02}\n"))
        .collect::<String>();
    fs::write(project.path.join("tests").join("smoke.err"), expected_err)
        .expect("write err sidecar");

    let stdout = assert_failure_stdout(run_nexus(&project.path, &["test", "tests"]));

    assert!(
        stdout.contains("diagnostico diferente do esperado"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("expected_error_19"), "stdout: {stdout}");
    assert!(stdout.contains("... 5 linhas omitidas"), "stdout: {stdout}");
    assert!(!stdout.contains("expected_error_20"), "stdout: {stdout}");

    let json = assert_failure_stdout(run_nexus(&project.path, &["test", "--json", "tests"]));
    assert!(json.contains("expected_error_24"), "json: {json}");
    assert!(!json.contains("linhas omitidas"), "json: {json}");
}

#[test]
fn cli_test_json_reports_expected_err_sidecar_as_passed_case() {
    let project = TempProject::new("test-json-err-sidecar-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("failing.nx"),
        "print(\"before\")\nprint(10 / 0)\n",
    )
    .expect("write failing");
    fs::write(
        project.path.join("tests").join("failing.err"),
        "Divisão por zero\n",
    )
    .expect("write err sidecar");

    let stdout = assert_success(run_nexus(&project.path, &["test", "--json", "tests"]));

    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":true"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""summary":{"passed":1,"failed":0,"total":1}"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""expected_diagnostic":["Divisão por zero"]"#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""diagnostics":[]"#), "stdout: {stdout}");
}

#[test]
fn cli_test_fail_fast_stops_after_first_failure() {
    let project = TempProject::new("test-fail-fast-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("a_fail.nx"),
        "print(\"before\")\nprint(10 / 0)\n",
    )
    .expect("write failing");
    fs::write(
        project.path.join("tests").join("b_after.nx"),
        "print(\"after\")\n",
    )
    .expect("write after");

    let stdout = assert_failure_stdout(run_nexus(&project.path, &["test", "--fail-fast", "tests"]));

    assert!(stdout.contains("FAIL tests/a_fail.nx"), "stdout: {stdout}");
    assert!(!stdout.contains("b_after.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains("Resultado: 0 passaram, 1 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_json_fail_fast_reports_partial_cases() {
    let project = TempProject::new("test-json-fail-fast-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("a_fail.nx"),
        "print(\"before\")\nprint(10 / 0)\n",
    )
    .expect("write failing");
    fs::write(
        project.path.join("tests").join("b_after.nx"),
        "print(\"after\")\n",
    )
    .expect("write after");

    let stdout = assert_failure_stdout(run_nexus(
        &project.path,
        &["test", "--json", "--fail-fast", "tests"],
    ));

    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":false"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""summary":{"passed":0,"failed":1,"total":1}"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""path":"tests/a_fail.nx""#),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("b_after.nx"), "stdout: {stdout}");
}

#[test]
fn cli_test_assert_helpers_pass_without_stdout_sidecar() {
    let project = TempProject::new("test-assert-pass-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("asserts.nx"),
        r#"
assert_true(10 > 2, "comparacao basica")
assert_eq("ativo", "ativo", "cliente ativo deve bater")
assert_ne("ativo", "inativo", "cliente nao deve estar inativo")
assert_contains("cliente ativo premium", "ativo", "texto deve conter status")
assert_contains([1, 2, 3], 2, "lista deve conter id")
"#,
    )
    .expect("write asserts");

    let stdout = assert_success(run_nexus(&project.path, &["test", "tests"]));

    assert!(stdout.contains("PASS tests/asserts.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains("Resultado: 1 passaram, 0 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_json_reports_assert_failure_as_runtime_diagnostic() {
    let project = TempProject::new("test-assert-json-failure-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("asserts.nx"),
        r#"
assert_contains("cliente ativo", "inativo", "cliente ativo deve bater")
"#,
    )
    .expect("write asserts");

    let stdout = assert_failure_stdout(run_nexus(&project.path, &["test", "--json", "tests"]));

    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":false"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""summary":{"passed":0,"failed":1,"total":1}"#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""stage":"runtime""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""code":"NXL5006""#), "stdout: {stdout}");
    assert!(
        stdout.contains("assert_contains falhou"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("cliente ativo deve bater"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_matches_optional_out_sidecar() {
    let project = TempProject::new("test-sidecar-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke.nx"),
        "print(\"ok\")\nprint(42)\n",
    )
    .expect("write smoke");
    fs::write(project.path.join("tests").join("smoke.out"), "ok\n42\n").expect("write sidecar");

    let stdout = assert_success(run_nexus(&project.path, &["test", "tests"]));

    assert!(stdout.contains("PASS tests/smoke.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains("Resultado: 1 passaram, 0 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_reports_out_sidecar_mismatch() {
    let project = TempProject::new("test-sidecar-mismatch-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke.nx"),
        "print(\"actual\")\n",
    )
    .expect("write smoke");
    fs::write(project.path.join("tests").join("smoke.out"), "expected\n").expect("write sidecar");

    let stdout = assert_failure_stdout(run_nexus(&project.path, &["test", "tests"]));

    assert!(stdout.contains("FAIL tests/smoke.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains("output diferente do esperado"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("primeira diferenca: linha 1"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("esperado: expected"), "stdout: {stdout}");
    assert!(stdout.contains("recebido: actual"), "stdout: {stdout}");
    assert!(stdout.contains("esperado:"), "stdout: {stdout}");
    assert!(stdout.contains("expected"), "stdout: {stdout}");
    assert!(stdout.contains("recebido:"), "stdout: {stdout}");
    assert!(stdout.contains("actual"), "stdout: {stdout}");
    assert!(
        stdout.contains("Resultado: 0 passaram, 1 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_human_report_truncates_long_out_mismatch_but_json_keeps_full_lines() {
    let project = TempProject::new("test-long-out-mismatch-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    let source = (0..25)
        .map(|index| format!("print(\"actual_{index:02}\")\n"))
        .collect::<String>();
    let expected = (0..25)
        .map(|index| format!("expected_{index:02}\n"))
        .collect::<String>();
    fs::write(project.path.join("tests").join("long.nx"), source).expect("write source");
    fs::write(project.path.join("tests").join("long.out"), expected).expect("write sidecar");

    let stdout = assert_failure_stdout(run_nexus(&project.path, &["test", "tests"]));

    assert!(
        stdout.contains("output diferente do esperado"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("expected_19"), "stdout: {stdout}");
    assert!(stdout.contains("actual_19"), "stdout: {stdout}");
    assert!(stdout.contains("... 5 linhas omitidas"), "stdout: {stdout}");
    assert!(!stdout.contains("expected_20"), "stdout: {stdout}");
    assert!(!stdout.contains("actual_20"), "stdout: {stdout}");

    let json = assert_failure_stdout(run_nexus(&project.path, &["test", "--json", "tests"]));
    assert!(json.contains("expected_24"), "json: {json}");
    assert!(json.contains("actual_24"), "json: {json}");
    assert!(!json.contains("linhas omitidas"), "json: {json}");
}

#[test]
fn cli_test_update_writes_out_sidecar() {
    let project = TempProject::new("test-update-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke.nx"),
        "print(\"updated\")\nprint(99)\n",
    )
    .expect("write smoke");

    let stdout = assert_success(run_nexus(&project.path, &["test", "--update", "tests"]));

    assert!(stdout.contains("PASS tests/smoke.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains("atualizado: tests/smoke.out"),
        "stdout: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(project.path.join("tests").join("smoke.out"))
            .expect("read updated sidecar"),
        "updated\n99\n"
    );
    assert!(
        stdout.contains("Resultado: 1 passaram, 0 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_update_does_not_write_out_sidecar_on_failure() {
    let project = TempProject::new("test-update-failure-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("failing.nx"),
        "print(\"before\")\nprint(10 / 0)\n",
    )
    .expect("write failing");
    fs::write(project.path.join("tests").join("failing.out"), "stale\n")
        .expect("write stale sidecar");

    let stdout = assert_failure_stdout(run_nexus(&project.path, &["test", "--update", "tests"]));

    assert!(stdout.contains("FAIL tests/failing.nx"), "stdout: {stdout}");
    assert!(stdout.contains("por zero"), "stdout: {stdout}");
    assert_eq!(
        fs::read_to_string(project.path.join("tests").join("failing.out")).expect("read sidecar"),
        "stale\n"
    );
}

#[test]
fn cli_test_update_err_writes_err_sidecar() {
    let project = TempProject::new("test-update-err-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("failing.nx"),
        "print(\"before\")\nprint(10 / 0)\n",
    )
    .expect("write failing");

    let stdout = assert_success(run_nexus(&project.path, &["test", "--update-err", "tests"]));

    assert!(stdout.contains("PASS tests/failing.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains("atualizado: tests/failing.err"),
        "stdout: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(project.path.join("tests").join("failing.err"))
            .expect("read err sidecar"),
        "Divisão por zero\n"
    );
    assert!(
        stdout.contains("Resultado: 1 passaram, 0 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_update_err_does_not_write_sidecar_on_success() {
    let project = TempProject::new("test-update-err-success-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke.nx"),
        "print(\"ok\")\n",
    )
    .expect("write smoke");

    let stdout = assert_success(run_nexus(&project.path, &["test", "--update-err", "tests"]));

    assert!(stdout.contains("PASS tests/smoke.nx"), "stdout: {stdout}");
    assert!(!project.path.join("tests").join("smoke.err").exists());
    assert!(
        stdout.contains("Resultado: 1 passaram, 0 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_json_update_err_reports_updated_sidecar() {
    let project = TempProject::new("test-json-update-err-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("failing.nx"),
        "print(\"before\")\nprint(10 / 0)\n",
    )
    .expect("write failing");

    let stdout = assert_success(run_nexus(
        &project.path,
        &["test", "--json", "--update-err", "tests"],
    ));

    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":true"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""summary":{"passed":1,"failed":0,"total":1}"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""expected_diagnostic":["Divisão por zero"]"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""expected_diagnostic_updated":"tests/failing.err""#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""diagnostics":[]"#), "stdout: {stdout}");
}

#[test]
fn cli_test_name_filter_runs_only_matching_files() {
    let project = TempProject::new("test-name-filter-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke_invoice.nx"),
        "print(\"invoice\")\n",
    )
    .expect("write selected");
    fs::write(
        project.path.join("tests").join("failing_inventory.nx"),
        "print(10 / 0)\n",
    )
    .expect("write skipped");

    let stdout = assert_success(run_nexus(
        &project.path,
        &["test", "--name", "invoice", "tests"],
    ));

    assert!(
        stdout.contains("PASS tests/smoke_invoice.nx"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("failing_inventory.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains("Resultado: 1 passaram, 0 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_list_reports_filtered_cases_without_executing_or_updating() {
    let project = TempProject::new("test-list-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke_billing.nx"),
        "print(10 / 0)\n",
    )
    .expect("write selected");
    fs::write(
        project.path.join("tests").join("smoke_inventory.nx"),
        "print(\"inventory\")\n",
    )
    .expect("write skipped");

    let stdout = assert_success(run_nexus(
        &project.path,
        &[
            "test",
            "--list",
            "--update",
            "--update-err",
            "--fail-fast",
            "--name",
            "billing",
            "tests",
        ],
    ));

    assert!(stdout.contains("Nexus tests: tests"), "stdout: {stdout}");
    assert!(
        stdout.contains("LIST tests/smoke_billing.nx"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("smoke_inventory.nx"), "stdout: {stdout}");
    assert!(!stdout.contains("PASS"), "stdout: {stdout}");
    assert!(!stdout.contains("FAIL"), "stdout: {stdout}");
    assert!(
        stdout.contains("Resultado: 1 casos encontrados"),
        "stdout: {stdout}"
    );
    assert!(!project
        .path
        .join("tests")
        .join("smoke_billing.out")
        .exists());
    assert!(!project
        .path
        .join("tests")
        .join("smoke_billing.err")
        .exists());
}

#[test]
fn cli_test_json_list_reports_filtered_cases_without_execution() {
    let project = TempProject::new("test-json-list-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("a_billing.nx"),
        "print(10 / 0)\n",
    )
    .expect("write selected");
    fs::write(
        project.path.join("tests").join("b_inventory.nx"),
        "print(\"inventory\")\n",
    )
    .expect("write skipped");

    let stdout = assert_success(run_nexus(
        &project.path,
        &["test", "--json", "--list", "--name", "billing", "tests"],
    ));

    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":true"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""mode":"list""#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""summary":{"passed":0,"failed":0,"total":1}"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""path":"tests/a_billing.nx""#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""status":"listed""#), "stdout: {stdout}");
    assert!(!stdout.contains("b_inventory.nx"), "stdout: {stdout}");
}

#[test]
fn cli_test_update_with_name_filter_updates_only_matching_sidecars() {
    let project = TempProject::new("test-update-name-filter-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke_billing.nx"),
        "print(\"billing\")\n",
    )
    .expect("write selected");
    fs::write(
        project.path.join("tests").join("smoke_inventory.nx"),
        "print(\"inventory\")\n",
    )
    .expect("write skipped");

    let stdout = assert_success(run_nexus(
        &project.path,
        &["test", "--update", "--name", "billing", "tests"],
    ));

    assert!(
        stdout.contains("PASS tests/smoke_billing.nx"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("atualizado: tests/smoke_billing.out"),
        "stdout: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(project.path.join("tests").join("smoke_billing.out"))
            .expect("read selected sidecar"),
        "billing\n"
    );
    assert!(!project
        .path
        .join("tests")
        .join("smoke_inventory.out")
        .exists());
    assert!(
        stdout.contains("Resultado: 1 passaram, 0 falharam, 1 total"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_json_reports_success_summary_and_cases() {
    let project = TempProject::new("test-json-success-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke.nx"),
        "print(\"ok\")\n",
    )
    .expect("write smoke");
    fs::write(project.path.join("tests").join("smoke.out"), "ok\n").expect("write sidecar");

    let stdout = assert_success(run_nexus(&project.path, &["test", "--json", "tests"]));

    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":true"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""command":"test""#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""summary":{"passed":1,"failed":0,"total":1}"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""path":"tests/smoke.nx""#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""output":["ok"]"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""expected_output":["ok"]"#),
        "stdout: {stdout}"
    );
    assert!(
        !stdout.contains("Resultado:"),
        "--json should not include the human report: {stdout}"
    );
}

#[test]
fn cli_test_json_reports_out_sidecar_mismatch_and_exits_failure() {
    let project = TempProject::new("test-json-mismatch-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke.nx"),
        "print(\"actual\")\n",
    )
    .expect("write smoke");
    fs::write(project.path.join("tests").join("smoke.out"), "expected\n").expect("write sidecar");

    let stdout = assert_failure_stdout(run_nexus(&project.path, &["test", "--json", "tests"]));

    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":false"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""summary":{"passed":0,"failed":1,"total":1}"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(
            r#""output_mismatch":{"expected":["expected"],"actual":["actual"],"first_diff":{"line":1,"expected":"expected","actual":"actual"}}"#
        ),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_json_composes_with_update_and_name_filter() {
    let project = TempProject::new("test-json-update-name-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("smoke_billing.nx"),
        "print(\"billing\")\n",
    )
    .expect("write selected");
    fs::write(
        project.path.join("tests").join("smoke_inventory.nx"),
        "print(\"inventory\")\n",
    )
    .expect("write skipped");

    let stdout = assert_success(run_nexus(
        &project.path,
        &["test", "--json", "--update", "--name", "billing", "tests"],
    ));

    assert!(stdout.contains(r#""ok":true"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""summary":{"passed":1,"failed":0,"total":1}"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""expected_output_updated":"tests/smoke_billing.out""#),
        "stdout: {stdout}"
    );
    assert!(
        !stdout.contains("smoke_inventory.nx"),
        "stdout should only mention the filtered case: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(project.path.join("tests").join("smoke_billing.out"))
            .expect("read selected sidecar"),
        "billing\n"
    );
    assert!(!project
        .path
        .join("tests")
        .join("smoke_inventory.out")
        .exists());
}

#[test]
fn cli_test_timeout_reports_failure_and_does_not_update_sidecar() {
    let project = TempProject::new("test-timeout-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("hang.nx"),
        "let contador = 0\nwhile true {\n    contador = contador + 1\n}\n",
    )
    .expect("write hanging test");

    let stdout = assert_failure_stdout(run_nexus(
        &project.path,
        &["test", "--update", "--timeout", "20ms", "tests"],
    ));

    assert!(stdout.contains("FAIL tests/hang.nx"), "stdout: {stdout}");
    assert!(stdout.contains("timeout excedido"), "stdout: {stdout}");
    assert!(
        stdout.contains("Timeout de teste excedido"),
        "stdout: {stdout}"
    );
    assert!(!project.path.join("tests").join("hang.out").exists());
}

#[test]
fn cli_test_json_timeout_reports_timed_out_case() {
    let project = TempProject::new("test-json-timeout-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(
        project.path.join("tests").join("hang.nx"),
        "let contador = 0\nwhile true {\n    contador = contador + 1\n}\n",
    )
    .expect("write hanging test");

    let stdout = assert_failure_stdout(run_nexus(
        &project.path,
        &["test", "--json", "--timeout", "20ms", "tests"],
    ));

    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":false"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""timed_out":true"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""summary":{"passed":0,"failed":1,"total":1}"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Timeout de teste excedido"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_test_isolate_data_sets_per_case_data_dir_without_workspace_data() {
    let project = TempProject::new("test-isolate-data-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    let source = r#"import env_get_or from "std/env"
print(env_get_or("NEXUS_DATA_DIR", "missing"))
"#;
    fs::write(project.path.join("tests").join("alpha.nx"), source).expect("write alpha");
    fs::write(project.path.join("tests").join("beta.nx"), source).expect("write beta");

    let stdout = assert_success(run_nexus(
        &project.path,
        &["test", "--json", "--isolate-data", "--jobs", "2", "tests"],
    ));

    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":true"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""summary":{"passed":2,"failed":0,"total":2}"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.matches("nexuslang-test-data").count() >= 4,
        "stdout should include per-case isolated dirs in output and metadata: {stdout}"
    );
    assert!(
        !stdout.contains(r#""output":["missing"]"#),
        "NEXUS_DATA_DIR should be set for isolated cases: {stdout}"
    );
    assert!(
        !project.path.join(".nexus-data").exists(),
        "isolated test data should not create workspace .nexus-data"
    );
}

#[test]
fn cli_test_jobs_runs_cases_in_stable_report_order() {
    let project = TempProject::new("test-jobs-cli");
    fs::create_dir_all(project.path.join("tests")).expect("create tests");
    fs::write(project.path.join("tests").join("b.nx"), "print(\"b\")\n").expect("write b");
    fs::write(project.path.join("tests").join("a.nx"), "print(\"a\")\n").expect("write a");

    let stdout = assert_success(run_nexus(
        &project.path,
        &["test", "--json", "--jobs", "2", "tests"],
    ));

    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":true"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""summary":{"passed":2,"failed":0,"total":2}"#),
        "stdout: {stdout}"
    );
    let a_index = stdout.find(r#""path":"tests/a.nx""#).expect("a case");
    let b_index = stdout.find(r#""path":"tests/b.nx""#).expect("b case");
    assert!(
        a_index < b_index,
        "parallel report should preserve sorted case order: {stdout}"
    );
}

#[test]
fn cli_check_and_run_core_stdlib_imports() {
    let project = TempProject::new("stdlib-core-cli");
    fs::write(
        project.path.join("main.nx"),
        r#"import contains from "std/string"
import contains_int from "std/collections"
import is_email from "std/validation"
import is_iso_date from "std/date"
import is_positive_money from "std/money"

print(contains("NexusLang", "Lang"))
print(contains_int([1, 2, 3], 2))
print(is_email("ana@example.com"))
print(is_iso_date("2026-05-27"))
print(is_positive_money(100 kz))
"#,
    )
    .expect("write source");

    assert_success(run_nexus(&project.path, &["check", "main.nx"]));

    let stdout = assert_success(run_nexus(&project.path, &["run", "main.nx"]));
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, ["true", "true", "true", "true", "true"]);
}

#[test]
fn cli_check_and_run_erp_stdlib_imports() {
    let project = TempProject::new("stdlib-erp-cli");
    fs::write(
        project.path.join("main.nx"),
        r#"import number_is_even from "std/number"
import inventory_stock_status from "std/inventory"
import crm_is_active_status from "std/crm"
import invoice_line_total from "std/invoice"

print(number_is_even(10))
print(inventory_stock_status(3, 5))
print(crm_is_active_status(" ACTIVE "))
print(invoice_line_total(2, 150 kz))
"#,
    )
    .expect("write source");

    assert_success(run_nexus(&project.path, &["check", "main.nx"]));

    let stdout = assert_success(run_nexus(&project.path, &["run", "main.nx"]));
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, ["true", "reorder", "true", "300.00 KZ"]);
}

#[test]
fn cli_check_and_run_data_protocol_stdlib_imports() {
    let project = TempProject::new("stdlib-data-protocol-cli");
    fs::write(
        project.path.join("main.nx"),
        r#"import json_pair from "std/json"
import json_bool from "std/json"
import json_object_1 from "std/json"
import csv_row_2 from "std/csv"
import http_status_text from "std/http"
import crypto_sha256_hex from "std/crypto"
import crypto_is_sha256_hex from "std/crypto"

print(json_object_1(json_pair("ok", json_bool(true))))
print(csv_row_2("Ana", "Luanda, Angola"))
print(http_status_text(201))
print(crypto_is_sha256_hex(crypto_sha256_hex("nexus")))
"#,
    )
    .expect("write source");

    assert_success(run_nexus(&project.path, &["check", "main.nx"]));

    let stdout = assert_success(run_nexus(&project.path, &["run", "main.nx"]));
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines,
        [
            r#"{"ok":true}"#,
            r#"Ana,"Luanda, Angola""#,
            "Created",
            "true"
        ]
    );
}

#[test]
fn cli_check_and_run_operational_stdlib_imports() {
    let project = TempProject::new("stdlib-operational-cli");
    fs::write(
        project.path.join("main.nx"),
        r#"import time_seconds_between from "std/time"
import env_get_or from "std/env"
import log_info from "std/log"
import path_basename from "std/path"

print(time_seconds_between(10, 16))
print(env_get_or("NEXUS_STDLIB_CLI_MISSING", "fallback"))
print(log_info("ok"))
print(path_basename("/tmp/report.csv"))
"#,
    )
    .expect("write source");

    assert_success(run_nexus(&project.path, &["check", "main.nx"]));

    let stdout = assert_success(run_nexus(&project.path, &["run", "main.nx"]));
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, ["6", "fallback", "[INFO] ok", "report.csv"]);
}

#[test]
fn cli_check_and_run_business_batch_stdlib_imports() {
    let project = TempProject::new("stdlib-business-batch-cli");
    fs::write(
        project.path.join("main.nx"),
        r#"import sales_line_total from "std/sales"
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
    )
    .expect("write source");

    assert_success(run_nexus(&project.path, &["check", "main.nx"]));

    let stdout = assert_success(run_nexus(&project.path, &["run", "main.nx"]));
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines,
        [
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
            "60.00 KZ"
        ]
    );
}

#[test]
fn cli_check_and_run_manifest_entrypoint() {
    let project = TempProject::new("manifest-entry");
    fs::create_dir_all(project.path.join("src")).expect("create src");
    fs::write(
        project.path.join("nexus.toml"),
        r#"[package]
name = "manifest-entry"
version = "0.1.0"
entry = "src/main.nx"

[dependencies]
"#,
    )
    .expect("write manifest");
    fs::write(
        project.path.join("src").join("main.nx"),
        r#"print("entry via manifest")"#,
    )
    .expect("write source");

    assert_success(run_nexus(&project.path, &["check"]));

    let stdout = assert_success(run_nexus(&project.path, &["run"]));
    assert!(stdout.contains("entry via manifest"), "stdout: {stdout}");
}

#[test]
fn cli_resolves_manifest_path_dependency_imports() {
    let workspace = TempProject::new("path-dep-graph");
    let dependency = workspace.path.join("crm_core");
    let app = workspace.path.join("erp_app");
    fs::create_dir_all(&dependency).expect("create dependency");
    fs::create_dir_all(&app).expect("create app");

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
export fn customer_label() -> string {
    return "Cliente via dep"
}
"#,
    )
    .expect("write dependency entry");
    fs::write(
        dependency.join("models.nx"),
        r#"
export fn account_status() -> string {
    return "Conta ativa"
}
"#,
    )
    .expect("write dependency module");

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
import customer_label from "crm_core"
import account_status from "crm_core/models"

print(customer_label())
print(account_status())
"#,
    )
    .expect("write app entry");

    assert_success(run_nexus(&app, &["check"]));

    let stdout = assert_success(run_nexus(&app, &["run"]));
    assert!(stdout.contains("Cliente via dep"), "stdout: {stdout}");
    assert!(stdout.contains("Conta ativa"), "stdout: {stdout}");
}

#[test]
fn cli_check_reports_duplicate_symbol_surface_for_path_dependencies() {
    let workspace = TempProject::new("path-dep-duplicate-surface");
    let dependency = workspace.path.join("crm_core");
    let app = workspace.path.join("erp_app");
    fs::create_dir_all(&dependency).expect("create dependency");
    fs::create_dir_all(&app).expect("create app");

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
import shared as dep_shared from "crm_core"
import shared as local_shared from "./local.nx"
"#,
    )
    .expect("write app entry");

    let stderr = assert_failure(run_nexus(&app, &["check"]));
    assert!(
        stderr.contains("Nome duplicado no module graph"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("shared"), "stderr: {stderr}");
}

#[test]
fn cli_check_reports_checker_error_path_for_imported_module() {
    let project = TempProject::new("imported-module-diagnostic-path");
    fs::write(
        project.path.join("lib.nx"),
        r#"
export fn broken() -> int {
    return "erro"
}
"#,
    )
    .expect("write lib");
    fs::write(
        project.path.join("main.nx"),
        r#"
import broken from "./lib.nx"
"#,
    )
    .expect("write main");

    let stderr = assert_failure(run_nexus(&project.path, &["check", "main.nx"]));
    assert!(stderr.contains("Erro de validação"), "stderr: {stderr}");
    assert!(stderr.contains("lib.nx"), "stderr: {stderr}");
    assert!(stderr.contains(":3:"), "stderr: {stderr}");
    assert!(
        stderr.contains("Tipo de retorno inválido"),
        "stderr: {stderr}"
    );
}

#[test]
fn cli_check_json_reports_success() {
    let project = TempProject::new("check-json-success");
    fs::write(project.path.join("main.nx"), r#"print("ok")"#).expect("write main");

    let output = run_nexus(&project.path, &["check", "main.nx", "--json"]);
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert!(stdout.contains(r#""ok":true"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""schema_version":1"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""command":"check""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""path":"main.nx""#), "stdout: {stdout}");
    assert!(
        !stdout.contains(r#""diagnostics":"#),
        "check --json should keep the first-error/success shape: {stdout}"
    );
    assert!(
        !stdout.contains(r#""groups":"#),
        "check --json should not emit report groups: {stdout}"
    );
}

#[test]
fn cli_check_json_report_reports_success() {
    let project = TempProject::new("check-json-report-success");
    fs::write(project.path.join("main.nx"), r#"print("ok")"#).expect("write main");

    let output = run_nexus(&project.path, &["check", "main.nx", "--json-report"]);
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":true"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""schema_version":1"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""command":"check""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""diagnostic":null"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""diagnostics":[]"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""groups":[]"#), "stdout: {stdout}");
}

#[test]
fn cli_check_json_reports_structured_imported_module_diagnostic() {
    let project = TempProject::new("check-json-imported-diagnostic");
    fs::write(
        project.path.join("lib.nx"),
        r#"
export fn broken() -> int {
    return "erro"
}
"#,
    )
    .expect("write lib");
    fs::write(
        project.path.join("main.nx"),
        r#"
import broken from "./lib.nx"
"#,
    )
    .expect("write main");

    let output = run_nexus(&project.path, &["check", "--json", "main.nx"]);
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert!(stdout.contains(r#""ok":false"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""schema_version":1"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""command":"check""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""code":"NXL3001""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""severity":"error""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""stage":"checker""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""path":"#), "stdout: {stdout}");
    assert!(stdout.contains("lib.nx"), "stdout: {stdout}");
    assert!(stdout.contains(r#""module_id":1"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""owner":{"decl_index":"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""source_range":{"start":{"line":2"#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""end":{"line":4"#), "stdout: {stdout}");
    assert!(
        stdout.contains("origem do erro de tipo"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("O checker compara"), "stdout: {stdout}");
    assert!(stdout.contains("Ajuste a anotacao"), "stdout: {stdout}");
    assert!(
        stdout.contains("Tipo de retorno inválido"),
        "stdout: {stdout}"
    );
    assert!(
        !stdout.contains(r#""diagnostics":"#),
        "check --json should keep the first-error diagnostic shape: {stdout}"
    );
    assert!(
        !stdout.contains(r#""groups":"#),
        "check --json should not emit report groups: {stdout}"
    );
}

#[test]
fn cli_check_json_report_reports_diagnostic_collection() {
    let project = TempProject::new("check-json-report-imported-diagnostic");
    fs::write(
        project.path.join("lib.nx"),
        r#"
export fn broken() -> int {
    return "erro"
}
"#,
    )
    .expect("write lib");
    fs::write(
        project.path.join("main.nx"),
        r#"
import broken from "./lib.nx"
"#,
    )
    .expect("write main");

    let output = run_nexus(&project.path, &["check", "--json-report", "main.nx"]);
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":false"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""schema_version":1"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""command":"check""#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""diagnostic":{"code":"NXL3001""#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""diagnostics":[{"code":"NXL3001""#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""groups":[{"path":"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""module_id":1"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""diagnostic_indexes":[0]"#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("lib.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""source_range":{"start":{"line":2"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Tipo de retorno inválido"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_check_json_report_collects_independent_checker_diagnostics() {
    let project = TempProject::new("check-json-report-multiple-checker-diagnostics");
    fs::write(
        project.path.join("lib.nx"),
        r#"
export fn broken_text() -> int {
    return "erro"
}

export fn broken_number() -> bool {
    return 1
}
"#,
    )
    .expect("write lib");
    fs::write(
        project.path.join("main.nx"),
        r#"
import broken_text from "./lib.nx"
import broken_number from "./lib.nx"
"#,
    )
    .expect("write main");

    let output = run_nexus(&project.path, &["check", "--json-report", "main.nx"]);
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":false"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""diagnostics":[{"code":"NXL3001""#),
        "stdout: {stdout}"
    );
    assert_eq!(
        stdout.matches(r#""code":"NXL3001""#).count(),
        3,
        "first diagnostic plus two collected diagnostics should be present: {stdout}"
    );
    assert!(
        stdout.contains(r#""diagnostic_indexes":[0,1]"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""source_range":{"start":{"line":2"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""source_range":{"start":{"line":6"#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("lib.nx"), "stdout: {stdout}");
}

#[test]
fn cli_check_json_reports_module_loader_diagnostic_contract() {
    let project = TempProject::new("check-json-loader-diagnostic");
    fs::write(
        project.path.join("lib.nx"),
        r#"
export fn present() -> int {
    return 1
}
"#,
    )
    .expect("write lib");
    fs::write(
        project.path.join("main.nx"),
        r#"
import missing from "./lib.nx"
"#,
    )
    .expect("write main");

    let output = run_nexus(&project.path, &["check", "--json", "main.nx"]);
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert!(stdout.contains(r#""ok":false"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""schema_version":1"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""command":"check""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""code":"NXL4004""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""severity":"error""#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""stage":"module_loader""#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""path":"#), "stdout: {stdout}");
    assert!(stdout.contains("lib.nx"), "stdout: {stdout}");
    assert!(stdout.contains(r#""module_id":null"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""owner":null"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""source_range":null"#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("simbolo importado aqui"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("nao exporta o simbolo solicitado"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("Exporte 'missing'"), "stdout: {stdout}");
    assert!(stdout.contains("não é exportado"), "stdout: {stdout}");
}

#[test]
fn cli_run_json_reports_success_with_captured_output() {
    let project = TempProject::new("run-json-success");
    fs::write(
        project.path.join("main.nx"),
        r#"
print("primeira")
print(2)
"#,
    )
    .expect("write main");

    let output = run_nexus(&project.path, &["run", "--json", "main.nx"]);
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":true"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""schema_version":1"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""command":"run""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""path":"main.nx""#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""output":["primeira","2"]"#),
        "stdout: {stdout}"
    );
    assert!(
        !stdout.contains(r#""diagnostics":"#),
        "run --json should keep the first-error/success shape: {stdout}"
    );
    assert!(
        !stdout.contains(r#""groups":"#),
        "run --json should not emit report groups: {stdout}"
    );
}

#[test]
fn cli_run_json_report_reports_success_with_captured_output() {
    let project = TempProject::new("run-json-report-success");
    fs::write(
        project.path.join("main.nx"),
        r#"
print("primeira")
print(2)
"#,
    )
    .expect("write main");

    let output = run_nexus(&project.path, &["run", "--json-report", "main.nx"]);
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":true"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""schema_version":1"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""command":"run""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""diagnostic":null"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""diagnostics":[]"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""groups":[]"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""output":["primeira","2"]"#),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_run_json_reports_runtime_diagnostic_with_partial_output() {
    let project = TempProject::new("run-json-runtime-diagnostic");
    fs::write(
        project.path.join("main.nx"),
        r#"
print("antes")
print(10 / 0)
"#,
    )
    .expect("write main");

    let output = run_nexus(&project.path, &["run", "main.nx", "--json"]);
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":false"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""schema_version":1"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""command":"run""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""code":"NXL5001""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""severity":"error""#), "stdout: {stdout}");
    assert!(stdout.contains(r#""stage":"runtime""#), "stdout: {stdout}");
    assert!(
        stdout.contains("operacao aritmetica em runtime"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("A execucao tentou dividir"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Garanta que o divisor seja diferente de zero"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("Divisão por zero"), "stdout: {stdout}");
    assert!(stdout.contains(r#""output":["antes"]"#), "stdout: {stdout}");
    assert!(
        !stdout.contains(r#""diagnostics":"#),
        "run --json should keep the first-error diagnostic shape: {stdout}"
    );
    assert!(
        !stdout.contains(r#""groups":"#),
        "run --json should not emit report groups: {stdout}"
    );
}

#[test]
fn cli_run_json_report_reports_runtime_diagnostic_with_partial_output() {
    let project = TempProject::new("run-json-report-runtime-diagnostic");
    fs::write(
        project.path.join("main.nx"),
        r#"
print("antes")
print(10 / 0)
"#,
    )
    .expect("write main");

    let output = run_nexus(&project.path, &["run", "main.nx", "--json-report"]);
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(stdout.contains(r#""ok":false"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""schema_version":1"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""command":"run""#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""diagnostic":{"code":"NXL5001""#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""diagnostics":[{"code":"NXL5001""#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""groups":[{"path":"#), "stdout: {stdout}");
    assert!(stdout.contains("main.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""module_id":0,"diagnostic_indexes":[0]}]"#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("Divisão por zero"), "stdout: {stdout}");
    assert!(stdout.contains(r#""output":["antes"]"#), "stdout: {stdout}");
}

#[test]
fn cli_run_json_report_reports_checker_diagnostic_without_output() {
    let project = TempProject::new("run-json-report-checker-diagnostic");
    fs::write(
        project.path.join("lib.nx"),
        r#"
export fn broken() -> int {
    return "erro"
}
"#,
    )
    .expect("write lib");
    fs::write(
        project.path.join("main.nx"),
        r#"
import broken from "./lib.nx"
print("nao executa")
"#,
    )
    .expect("write main");

    let output = run_nexus(&project.path, &["run", "--json-report", "main.nx"]);
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(
        stdout.contains(r#""diagnostic":{"code":"NXL3001""#),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(r#""diagnostics":[{"code":"NXL3001""#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""groups":[{"path":"#), "stdout: {stdout}");
    assert!(stdout.contains(r#""module_id":1"#), "stdout: {stdout}");
    assert!(
        stdout.contains(r#""diagnostic_indexes":[0]"#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("lib.nx"), "stdout: {stdout}");
    assert!(
        stdout.contains("Tipo de retorno inválido"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""output":[]"#), "stdout: {stdout}");
}

#[test]
fn cli_run_json_report_collects_checker_diagnostics_without_output() {
    let project = TempProject::new("run-json-report-multiple-checker-diagnostics");
    fs::write(
        project.path.join("lib.nx"),
        r#"
export fn broken_text() -> int {
    return "erro"
}

export fn broken_number() -> bool {
    return 1
}
"#,
    )
    .expect("write lib");
    fs::write(
        project.path.join("main.nx"),
        r#"
import broken_text from "./lib.nx"
import broken_number from "./lib.nx"
print("nao executa")
"#,
    )
    .expect("write main");

    let output = run_nexus(&project.path, &["run", "--json-report", "main.nx"]);
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert_eq!(stdout.lines().count(), 1, "stdout: {stdout}");
    assert!(
        stdout.contains(r#""diagnostics":[{"code":"NXL3001""#),
        "stdout: {stdout}"
    );
    assert_eq!(
        stdout.matches(r#""code":"NXL3001""#).count(),
        3,
        "first diagnostic plus two collected diagnostics should be present: {stdout}"
    );
    assert!(
        stdout.contains(r#""diagnostic_indexes":[0,1]"#),
        "stdout: {stdout}"
    );
    assert!(stdout.contains(r#""output":[]"#), "stdout: {stdout}");
    assert!(
        !stdout.contains("nao executa"),
        "program output should not run when checker report fails: {stdout}"
    );
}

#[test]
fn cli_serve_loads_multi_module_program() {
    let project = TempProject::new("serve-multi-module");
    fs::write(
        project.path.join("models.nx"),
        r#"
export model Customer {
    name: string
}
"#,
    )
    .expect("write models");
    fs::write(
        project.path.join("main.nx"),
        r#"
import Customer from "./models.nx"

route GET /customers {
    return Customer::all()
}
"#,
    )
    .expect("write main");
    fs::write(
        project.path.join("nexus.toml"),
        r#"[package]
name = "serve-multi-module"
version = "0.1.0"
entry = "main.nx"

[dependencies]
"#,
    )
    .expect("write manifest");

    let addr = free_addr();
    let mut child = nexus()
        .current_dir(&project.path)
        .args(["serve", "--addr", &addr])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn nexus serve");

    let response = wait_for_http(&mut child, &addr, "/__health");
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "unexpected response: {response}"
    );

    stop_child(&mut child);
}

#[test]
fn cli_serve_accepts_sqlite_storage_driver() {
    let project = TempProject::new("serve-sqlite-driver");
    fs::write(
        project.path.join("main.nx"),
        r#"
model Customer {
    name: string
}

route GET /customers {
    return Customer::all()
}
"#,
    )
    .expect("write main");

    let addr = free_addr();
    let mut child = nexus()
        .current_dir(&project.path)
        .args(["serve", "main.nx", &addr, "--storage", "sqlite"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn nexus serve");

    let response = wait_for_http(&mut child, &addr, "/customers");
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "unexpected response: {response}"
    );
    assert!(response.ends_with("[]"), "unexpected response: {response}");
    assert!(
        project.path.join(".nexus-data").join("nexus.db").exists(),
        "sqlite driver should create .nexus-data/nexus.db"
    );
    assert!(
        !project
            .path
            .join(".nexus-data")
            .join("customer.json")
            .exists(),
        "sqlite driver should not create JSON model files"
    );

    stop_child(&mut child);
}

#[test]
fn cli_storage_plan_sqlite_dry_run_and_apply() {
    let project = TempProject::new("storage-plan-sqlite");
    fs::write(
        project.path.join("main.nx"),
        r#"
model Customer {
    email: string unique
    status: string index
}
"#,
    )
    .expect("write main");

    let dry_run = assert_success(run_nexus(
        &project.path,
        &["storage-plan", "main.nx", "--storage", "sqlite"],
    ));
    assert!(dry_run.contains("Mode: dry-run"), "stdout: {dry_run}");
    assert!(
        dry_run.contains("create SQLite model table 'customer'"),
        "stdout: {dry_run}"
    );
    assert!(dry_run.contains("idx_customer_email"), "stdout: {dry_run}");
    assert!(dry_run.contains("idx_customer_status"), "stdout: {dry_run}");
    assert!(
        !project.path.join(".nexus-data").join("nexus.db").exists(),
        "storage-plan dry-run must not create the SQLite database"
    );
    assert!(
        !project
            .path
            .join(".nexus-data")
            .join("nexus.db-wal")
            .exists(),
        "storage-plan dry-run must not create SQLite WAL files"
    );

    let applied = assert_success(run_nexus(
        &project.path,
        &["storage-plan", "main.nx", "--storage", "sqlite", "--apply"],
    ));
    assert!(applied.contains("Mode: applied"), "stdout: {applied}");
    assert!(
        project.path.join(".nexus-data").join("nexus.db").exists(),
        "storage-plan --apply should create the SQLite database"
    );

    let after_apply = assert_success(run_nexus(
        &project.path,
        &["storage-plan", "main.nx", "--storage", "sqlite"],
    ));
    assert!(
        after_apply.contains("Actions: none"),
        "stdout: {after_apply}"
    );
    assert!(
        after_apply.contains("Blockers: none"),
        "stdout: {after_apply}"
    );
}

#[test]
fn cli_serve_rejects_unknown_storage_driver() {
    let project = TempProject::new("serve-unknown-driver");
    fs::write(project.path.join("main.nx"), r#"print("ok")"#).expect("write main");

    let stderr = assert_failure(run_nexus(
        &project.path,
        &["serve", "main.nx", "--storage", "memory"],
    ));
    assert!(stderr.contains("Storage driver 'memory' nao suportado"));
    assert!(stderr.contains("Drivers disponiveis: json, sqlite"));
}

fn free_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind free port");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);
    addr.to_string()
}

fn wait_for_http(child: &mut Child, addr: &str, path: &str) -> String {
    let socket_addr: SocketAddr = addr.parse().expect("socket addr");
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if let Some(status) = child.try_wait().expect("poll child") {
            panic!("server exited early with status {status}");
        }

        if let Ok(mut stream) = TcpStream::connect_timeout(&socket_addr, Duration::from_millis(250))
        {
            let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\n\r\n");
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            if stream.write_all(request.as_bytes()).is_err() {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            let mut response = String::new();
            if stream.read_to_string(&mut response).is_ok() && !response.is_empty() {
                return response;
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    stop_child(child);
    panic!("server did not respond on {addr}");
}

fn stop_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
