#![allow(clippy::too_many_arguments, clippy::result_large_err, deprecated)]

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use nexuslang::package_manager;
use nexuslang::server::{Storage, StorageDriver};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage_and_exit(1);
    }

    let command = &args[1];

    match command.as_str() {
        "help" | "--help" | "-h" => {
            print_usage_and_success();
        }
        "run" => {
            let (file_path, output_mode) = run_entry_and_output_mode(&args);
            if output_mode == RunOutputMode::JsonReport {
                match nexuslang::load_and_run_with_source_database_captured_diagnostic_report(
                    &file_path,
                ) {
                    Ok(output) => {
                        let report = nexuslang::MultiModuleDiagnosticReport::empty();
                        println!(
                            "{}",
                            nexuslang::multi_module_diagnostic_report_output_json(
                                "run", &report, &output
                            )
                        );
                    }
                    Err(e) => {
                        println!(
                            "{}",
                            nexuslang::multi_module_diagnostic_report_output_json(
                                "run", &e.report, &e.output
                            )
                        );
                        process::exit(1);
                    }
                }
                return;
            }

            if output_mode == RunOutputMode::Json {
                match nexuslang::load_and_run_with_source_database_captured_diagnostic(&file_path) {
                    Ok(output) => {
                        println!(
                            "{}",
                            nexuslang::multi_module_success_output_json("run", &file_path, &output)
                        );
                    }
                    Err(e) => {
                        println!(
                            "{}",
                            nexuslang::multi_module_diagnostic_output_json(
                                "run",
                                &e.diagnostic,
                                &e.output
                            )
                        );
                        process::exit(1);
                    }
                }
            } else if let Err(e) =
                nexuslang::load_and_run_with_source_database_diagnostic(&file_path)
            {
                eprintln!("Erro de execução: {}", e);
                process::exit(1);
            }
        }
        "check" => {
            let (file_path, output_mode) = check_entry_and_output_mode(&args);
            if output_mode == CheckOutputMode::JsonReport {
                match nexuslang::load_and_check_with_source_database_diagnostic_report(&file_path) {
                    Ok(_) => {
                        let report = nexuslang::MultiModuleDiagnosticReport::empty();
                        println!(
                            "{}",
                            nexuslang::multi_module_diagnostic_report_json("check", &report)
                        );
                    }
                    Err(report) => {
                        println!(
                            "{}",
                            nexuslang::multi_module_diagnostic_report_json("check", &report)
                        );
                        process::exit(1);
                    }
                }
                return;
            }

            match nexuslang::load_and_check_with_source_database_diagnostic(&file_path) {
                Ok(_) if output_mode == CheckOutputMode::Json => {
                    println!(
                        "{}",
                        nexuslang::multi_module_success_json("check", &file_path)
                    );
                }
                Ok(_) => {
                    println!("OK: '{}' é válido", file_path.display());
                }
                Err(e) if output_mode == CheckOutputMode::Json => {
                    println!("{}", nexuslang::multi_module_diagnostic_json("check", &e));
                    process::exit(1);
                }
                Err(e) => {
                    eprintln!("Erro de validação: {}", e);
                    process::exit(1);
                }
            }
        }
        "tokens" => {
            let file_path = required_arg(&args, 2, "tokens <ficheiro.nx>");
            let source = read_source(file_path);
            let tokens = match nexuslang::tokens_source_spanned_diagnostic(&source) {
                Ok(tokens) => tokens,
                Err(e) => {
                    eprintln!("Erro de lexing: {}", e);
                    process::exit(1);
                }
            };
            println!("Tokens em '{}':", file_path);
            for (tok, line, column) in &tokens {
                println!("  {:>4}:{:<3} │ {:?}", line, column, tok);
            }
        }
        "ast" => {
            let file_path = required_arg(&args, 2, "ast <ficheiro.nx>");
            let source = read_source(file_path);
            match nexuslang::ast_source(&source) {
                Ok(prog) => {
                    println!("AST de '{}':", file_path);
                    println!("{:#?}", prog);
                }
                Err(e) => {
                    eprintln!("Erro de parsing: {}", e);
                    process::exit(1);
                }
            }
        }
        "fmt" => {
            let file_path = required_arg(&args, 2, "fmt <ficheiro.nx> [--write]");
            let source = read_source(file_path);
            let formatted = match nexuslang::fmt_source(&source) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Erro de formatação: {}", e);
                    process::exit(1);
                }
            };

            if args.iter().any(|arg| arg == "--write") {
                if let Err(e) = fs::write(file_path, formatted) {
                    eprintln!("Erro ao escrever '{}': {}", file_path, e);
                    process::exit(1);
                }
                println!("Formatado: {}", file_path);
            } else {
                print!("{}", formatted);
            }
        }
        "lint" => {
            let file_path = required_arg(&args, 2, "lint <ficheiro.nx>");
            let source = read_source(file_path);
            match nexuslang::lint_source(&source) {
                Ok(warnings) if warnings.is_empty() => {
                    println!("OK: '{}' sem avisos", file_path);
                }
                Ok(warnings) => {
                    println!("Avisos em '{}':", file_path);
                    for warning in warnings {
                        println!("  {}: {}", warning.code, warning.message);
                    }
                }
                Err(e) => {
                    eprintln!("Erro de lint: {}", e);
                    process::exit(1);
                }
            }
        }
        "docs" => {
            let (file_path, output_path) = docs_entry_and_output_path(&args);
            let title = format!("NexusLang Docs: {}", file_path.display());
            match nexuslang::docs_entry(&file_path, Some(&title)) {
                Ok(markdown) => {
                    if let Some(output_path) = output_path {
                        if let Err(e) = fs::write(&output_path, markdown) {
                            eprintln!("Erro ao escrever '{}': {}", output_path.display(), e);
                            process::exit(1);
                        }
                        println!("Documentacao gerada: {}", output_path.display());
                    } else {
                        print!("{}", markdown);
                    }
                }
                Err(e) => {
                    eprintln!("Erro de documentacao: {}", e);
                    process::exit(1);
                }
            }
        }
        "test" => {
            run_test_command(&args);
        }
        "serve" => {
            let (file_path, addr, storage_driver) = serve_entry_addr_and_storage_driver(&args);
            if let Err(e) = nexuslang::server::serve_file_with_storage_driver(
                file_path.to_string_lossy().as_ref(),
                &addr,
                storage_driver,
            ) {
                eprintln!("Erro no servidor: {}", e);
                process::exit(1);
            }
        }
        "storage-plan" => {
            run_storage_plan_command(&args);
        }
        "repl" => run_repl(),
        "new" => {
            let project_name = required_arg(&args, 2, "new <project>");
            if let Err(e) = create_project(project_name) {
                eprintln!("Erro ao criar projeto: {}", e);
                process::exit(1);
            }
        }
        "install" => {
            run_package_command(package_manager::install_current_dir());
        }
        "add" => {
            let package_name = required_arg(
                &args,
                2,
                "add <pacote> [--path <dir>|--registry <pacote@versao>]",
            );
            let source = parse_add_source(&args);
            run_package_command(package_manager::add_dependency_current_dir(
                package_name,
                source,
            ));
        }
        "update" => {
            run_package_command(package_manager::update_current_dir());
        }
        cmd => {
            eprintln!("Comando desconhecido: '{}'", cmd);
            print_usage_and_exit(1);
        }
    }
}

fn print_usage_and_success() -> ! {
    let mut stdout = io::stdout();
    print_usage(&mut stdout);
    process::exit(0);
}

fn print_usage_and_exit(code: i32) -> ! {
    let mut stderr = io::stderr();
    print_usage(&mut stderr);
    process::exit(code);
}

fn print_usage(out: &mut dyn Write) {
    writeln!(out, "NexusLang 🔷 v{}", env!("CARGO_PKG_VERSION")).ok();
    writeln!(out).ok();
    writeln!(out, "Uso:").ok();
    writeln!(
        out,
        "  nexus run [ficheiro.nx] [--json|--json-report] — Executar programa"
    )
    .ok();
    writeln!(
        out,
        "  nexus check [ficheiro.nx] [--json|--json-report] — Validar sem executar"
    )
    .ok();
    writeln!(out, "  nexus fmt <ficheiro.nx> [--write] — Formatar código").ok();
    writeln!(
        out,
        "  nexus lint <ficheiro.nx>         — Analisar estilo e riscos"
    )
    .ok();
    writeln!(
        out,
        "  nexus docs [ficheiro.nx] [--output docs.md] — Gerar documentacao Markdown"
    )
    .ok();
    writeln!(
        out,
        "  nexus {} — Rodar ou listar testes/exemplos .nx",
        TEST_USAGE
    )
    .ok();
    writeln!(
        out,
        "  nexus serve [ficheiro.nx] [addr] [--storage json|sqlite] — Servir routes HTTP"
    )
    .ok();
    writeln!(
        out,
        "  nexus serve --addr <addr> [--storage json|sqlite] — Servir entry do nexus.toml"
    )
    .ok();
    writeln!(
        out,
        "  nexus storage-plan [ficheiro.nx] [--storage sqlite] [--apply] — Planejar/aplicar migração de storage"
    )
    .ok();
    writeln!(
        out,
        "  nexus repl                       — Abrir REPL simples"
    )
    .ok();
    writeln!(
        out,
        "  nexus new <project>              — Criar projeto NexusLang"
    )
    .ok();
    writeln!(
        out,
        "  nexus install                    — Instalar dependências locais"
    )
    .ok();
    writeln!(
        out,
        "  nexus add <pacote> [--path <dir>|--registry <pkg@ver>]"
    )
    .ok();
    writeln!(
        out,
        "  nexus update                     — Atualizar lockfile local"
    )
    .ok();
    writeln!(
        out,
        "  nexus tokens <ficheiro.nx>       — Ver tokens (debug)"
    )
    .ok();
    writeln!(out, "  nexus ast <ficheiro.nx>          — Ver AST (debug)").ok();
}

fn required_arg<'a>(args: &'a [String], index: usize, usage: &str) -> &'a str {
    match args.get(index) {
        Some(value) => value,
        None => {
            eprintln!("Uso: nexus {}", usage);
            process::exit(1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunOutputMode {
    Text,
    Json,
    JsonReport,
}

fn run_entry_and_output_mode(args: &[String]) -> (PathBuf, RunOutputMode) {
    let mut output_mode = RunOutputMode::Text;
    let mut file_path: Option<&str> = None;

    for arg in args.iter().skip(2) {
        match arg.as_str() {
            "--json" => {
                if output_mode != RunOutputMode::Text {
                    eprintln!("Uso: nexus run [ficheiro.nx] [--json|--json-report]");
                    process::exit(1);
                }
                output_mode = RunOutputMode::Json;
            }
            "--json-report" => {
                if output_mode != RunOutputMode::Text {
                    eprintln!("Uso: nexus run [ficheiro.nx] [--json|--json-report]");
                    process::exit(1);
                }
                output_mode = RunOutputMode::JsonReport;
            }
            _ if arg.starts_with("--") => {
                eprintln!("Opção desconhecida para run: '{}'", arg);
                print_usage_and_exit(1);
            }
            _ => {
                if file_path.replace(arg.as_str()).is_some() {
                    eprintln!("Uso: nexus run [ficheiro.nx] [--json|--json-report]");
                    process::exit(1);
                }
            }
        }
    }

    let path = file_path
        .map(PathBuf::from)
        .unwrap_or_else(|| project_entry_or_exit("run [ficheiro.nx] [--json|--json-report]"));
    (path, output_mode)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckOutputMode {
    Text,
    Json,
    JsonReport,
}

fn check_entry_and_output_mode(args: &[String]) -> (PathBuf, CheckOutputMode) {
    let mut output_mode = CheckOutputMode::Text;
    let mut file_path: Option<&str> = None;

    for arg in args.iter().skip(2) {
        match arg.as_str() {
            "--json" => {
                if output_mode != CheckOutputMode::Text {
                    eprintln!("Uso: nexus check [ficheiro.nx] [--json|--json-report]");
                    process::exit(1);
                }
                output_mode = CheckOutputMode::Json;
            }
            "--json-report" => {
                if output_mode != CheckOutputMode::Text {
                    eprintln!("Uso: nexus check [ficheiro.nx] [--json|--json-report]");
                    process::exit(1);
                }
                output_mode = CheckOutputMode::JsonReport;
            }
            _ if arg.starts_with("--") => {
                eprintln!("Opção desconhecida para check: '{}'", arg);
                print_usage_and_exit(1);
            }
            _ => {
                if file_path.replace(arg.as_str()).is_some() {
                    eprintln!("Uso: nexus check [ficheiro.nx] [--json|--json-report]");
                    process::exit(1);
                }
            }
        }
    }

    let path = file_path
        .map(PathBuf::from)
        .unwrap_or_else(|| project_entry_or_exit("check [ficheiro.nx] [--json|--json-report]"));
    (path, output_mode)
}

fn docs_entry_and_output_path(args: &[String]) -> (PathBuf, Option<PathBuf>) {
    let mut file_path: Option<&str> = None;
    let mut output_path: Option<&str> = None;
    let mut index = 2;

    while index < args.len() {
        match args[index].as_str() {
            "--output" | "-o" => {
                index += 1;
                let value = args.get(index).unwrap_or_else(|| {
                    eprintln!("Uso: nexus docs [ficheiro.nx] [--output docs.md]");
                    process::exit(1);
                });
                if output_path.replace(value.as_str()).is_some() {
                    eprintln!("Opcao --output repetida");
                    process::exit(1);
                }
            }
            option if option.starts_with("--") => {
                eprintln!("Opcao desconhecida para docs: '{}'", option);
                print_usage_and_exit(1);
            }
            value => {
                if file_path.replace(value).is_some() {
                    eprintln!("Uso: nexus docs [ficheiro.nx] [--output docs.md]");
                    process::exit(1);
                }
            }
        }
        index += 1;
    }

    let path = file_path
        .map(PathBuf::from)
        .unwrap_or_else(|| project_entry_or_exit("docs [ficheiro.nx] [--output docs.md]"));
    (path, output_path.map(PathBuf::from))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestOutputMode {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestAction {
    Run,
    List,
}

const TEST_USAGE: &str = "test [--json] [--list] [--update] [--update-err] [--fail-fast] [--name termo] [--timeout 5s] [--isolate-data] [--jobs 4] [ficheiro-ou-diretorio]";
const TEST_HUMAN_OUTPUT_LINE_LIMIT: usize = 20;

fn test_target_options_and_output_mode(
    args: &[String],
) -> (
    Option<PathBuf>,
    nexuslang::test_runner::NexusTestOptions,
    TestOutputMode,
    TestAction,
) {
    let mut target: Option<&str> = None;
    let mut options = nexuslang::test_runner::NexusTestOptions::default();
    let mut output_mode = TestOutputMode::Text;
    let mut action = TestAction::Run;
    let mut index = 2;

    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                output_mode = TestOutputMode::Json;
            }
            "--list" => {
                if action == TestAction::List {
                    eprintln!("Opcao --list repetida");
                    process::exit(1);
                }
                action = TestAction::List;
            }
            "--update" => {
                options.update_expected = true;
            }
            "--update-err" => {
                if options.update_expected_diagnostic {
                    eprintln!("Opcao --update-err repetida");
                    process::exit(1);
                }
                options.update_expected_diagnostic = true;
            }
            "--fail-fast" => {
                if options.fail_fast {
                    eprintln!("Opcao --fail-fast repetida");
                    process::exit(1);
                }
                options.fail_fast = true;
            }
            "--isolate-data" => {
                options.isolate_data = true;
            }
            "--jobs" => {
                index += 1;
                let value = args.get(index).unwrap_or_else(|| {
                    eprintln!("Uso: nexus {}", TEST_USAGE);
                    process::exit(1);
                });
                let jobs = parse_test_jobs(value).unwrap_or_else(|e| {
                    eprintln!("{}", e);
                    process::exit(1);
                });
                if options.jobs != 1 {
                    eprintln!("Opcao --jobs repetida");
                    process::exit(1);
                }
                options.jobs = jobs;
            }
            "--name" => {
                index += 1;
                let value = args.get(index).unwrap_or_else(|| {
                    eprintln!("Uso: nexus {}", TEST_USAGE);
                    process::exit(1);
                });
                if value.is_empty() {
                    eprintln!("Opcao --name requer um termo nao vazio");
                    process::exit(1);
                }
                if options.name_filter.replace(value.to_string()).is_some() {
                    eprintln!("Opcao --name repetida");
                    process::exit(1);
                }
            }
            "--timeout" => {
                index += 1;
                let value = args.get(index).unwrap_or_else(|| {
                    eprintln!("Uso: nexus {}", TEST_USAGE);
                    process::exit(1);
                });
                let timeout = parse_test_timeout(value).unwrap_or_else(|e| {
                    eprintln!("{}", e);
                    process::exit(1);
                });
                if options.timeout.replace(timeout).is_some() {
                    eprintln!("Opcao --timeout repetida");
                    process::exit(1);
                }
            }
            option if option.starts_with("--") => {
                eprintln!("Opcao desconhecida para test: '{}'", option);
                print_usage_and_exit(1);
            }
            value => {
                if target.replace(value).is_some() {
                    eprintln!("Uso: nexus {}", TEST_USAGE);
                    process::exit(1);
                }
            }
        }
        index += 1;
    }

    (target.map(PathBuf::from), options, output_mode, action)
}

fn parse_test_timeout(value: &str) -> Result<Duration, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("Opcao --timeout requer duracao nao vazia".to_string());
    }

    let (number, unit) = if let Some(number) = value.strip_suffix("ms") {
        (number, "ms")
    } else if let Some(number) = value.strip_suffix('s') {
        (number, "s")
    } else if let Some(number) = value.strip_suffix('m') {
        (number, "m")
    } else {
        (value, "s")
    };

    let amount = number.parse::<u64>().map_err(|_| {
        format!(
            "Duracao invalida para --timeout: '{}'. Use valores como 500ms, 5s ou 1m",
            value
        )
    })?;
    if amount == 0 {
        return Err("Opcao --timeout deve ser maior que zero".to_string());
    }

    match unit {
        "ms" => Ok(Duration::from_millis(amount)),
        "s" => Ok(Duration::from_secs(amount)),
        "m" => amount
            .checked_mul(60)
            .map(Duration::from_secs)
            .ok_or_else(|| "Duracao de --timeout e grande demais".to_string()),
        _ => unreachable!("known timeout unit"),
    }
}

fn parse_test_jobs(value: &str) -> Result<usize, String> {
    let jobs = value.parse::<usize>().map_err(|_| {
        format!(
            "Valor invalido para --jobs: '{}'. Use um inteiro maior que zero",
            value
        )
    })?;
    if jobs == 0 {
        return Err("Opcao --jobs deve ser maior que zero".to_string());
    }
    Ok(jobs)
}

fn run_test_command(args: &[String]) {
    let (target, options, output_mode, action) = test_target_options_and_output_mode(args);

    match action {
        TestAction::Run => {
            let result = match target {
                Some(path) => nexuslang::test_runner::run_tests_at_with_options(&path, options),
                None => {
                    nexuslang::test_runner::run_default_tests_from_current_dir_with_options(options)
                }
            };
            match result {
                Ok(report) => {
                    print_test_report_for_mode(&report, output_mode);
                    if !report.is_success() {
                        process::exit(1);
                    }
                }
                Err(e) => {
                    print_test_error_for_mode(&e, output_mode);
                    process::exit(1);
                }
            }
        }
        TestAction::List => {
            let result = match target {
                Some(path) => nexuslang::test_runner::list_tests_at_with_options(&path, &options),
                None => nexuslang::test_runner::list_default_tests_from_current_dir_with_options(
                    &options,
                ),
            };
            match result {
                Ok(report) => print_test_list_report_for_mode(&report, output_mode),
                Err(e) => {
                    print_test_error_for_mode(&e, output_mode);
                    process::exit(1);
                }
            }
        }
    }
}

fn print_test_report_for_mode(
    report: &nexuslang::test_runner::NexusTestReport,
    output_mode: TestOutputMode,
) {
    match output_mode {
        TestOutputMode::Text => print_test_report(report),
        TestOutputMode::Json => println!("{}", nexuslang::test_runner::test_report_json(report)),
    }
}

fn print_test_list_report_for_mode(
    report: &nexuslang::test_runner::NexusTestListReport,
    output_mode: TestOutputMode,
) {
    match output_mode {
        TestOutputMode::Text => print_test_list_report(report),
        TestOutputMode::Json => {
            println!("{}", nexuslang::test_runner::test_list_report_json(report))
        }
    }
}

fn print_test_error_for_mode(message: &str, output_mode: TestOutputMode) {
    match output_mode {
        TestOutputMode::Text => eprintln!("Erro de teste: {}", message),
        TestOutputMode::Json => println!("{}", nexuslang::test_runner::test_error_json(message)),
    }
}

fn print_test_list_report(report: &nexuslang::test_runner::NexusTestListReport) {
    println!("Nexus tests: {}", report.target.display());
    for case in &report.cases {
        println!("LIST {}", case.display());
    }
    println!("Resultado: {} casos encontrados", report.cases.len());
}

fn print_test_report(report: &nexuslang::test_runner::NexusTestReport) {
    println!("Nexus tests: {}", report.target.display());
    for case in &report.cases {
        if case.passed() {
            println!("PASS {}", case.path.display());
            if let Some(path) = &case.expected_output_updated {
                println!("  atualizado: {}", path.display());
            }
            if let Some(path) = &case.expected_diagnostic_updated {
                println!("  atualizado: {}", path.display());
            }
        } else {
            println!("FAIL {}", case.path.display());
            if case.timed_out {
                println!("  timeout excedido");
            }
            if let Some(diagnostic) = &case.diagnostic {
                println!("  {}", diagnostic);
            }
            if let Some(mismatch) = &case.output_mismatch {
                println!("  output diferente do esperado");
                if let Some(diff) = &mismatch.first_diff {
                    print_first_diff(diff);
                }
                println!("  esperado:");
                print_output_lines(&mismatch.expected);
                println!("  recebido:");
                print_output_lines(&mismatch.actual);
            }
            if let Some(mismatch) = &case.diagnostic_mismatch {
                println!("  diagnostico diferente do esperado");
                if let Some(diff) = &mismatch.first_diff {
                    print_first_diff(diff);
                }
                println!("  esperado:");
                print_output_lines(&mismatch.expected);
                println!("  recebido:");
                match &mismatch.actual {
                    Some(actual) => print_output_lines(actual),
                    None => println!("    <sem diagnostico>"),
                }
            }
            if case.output_mismatch.is_none() && !case.output.is_empty() {
                println!("  output:");
                print_output_lines(&case.output);
            }
        }
    }
    println!(
        "Resultado: {} passaram, {} falharam, {} total",
        report.passed(),
        report.failed(),
        report.total()
    );
}

fn print_first_diff(diff: &nexuslang::test_runner::NexusLineDiff) {
    println!("  primeira diferenca: linha {}", diff.line);
    println!(
        "    esperado: {}",
        diff.expected.as_deref().unwrap_or("<sem linha>")
    );
    println!(
        "    recebido: {}",
        diff.actual.as_deref().unwrap_or("<sem linha>")
    );
}

fn print_output_lines(lines: &[String]) {
    if lines.is_empty() {
        println!("    <vazio>");
    } else {
        for line in lines.iter().take(TEST_HUMAN_OUTPUT_LINE_LIMIT) {
            println!("    {}", line);
        }
        if lines.len() > TEST_HUMAN_OUTPUT_LINE_LIMIT {
            println!(
                "    ... {} linhas omitidas",
                lines.len() - TEST_HUMAN_OUTPUT_LINE_LIMIT
            );
        }
    }
}

fn serve_entry_addr_and_storage_driver(args: &[String]) -> (PathBuf, String, StorageDriver) {
    let mut file_path: Option<&str> = None;
    let mut addr: Option<&str> = None;
    let mut storage_driver = StorageDriver::DEFAULT;
    let mut storage_driver_seen = false;
    let mut index = 2;

    while index < args.len() {
        match args[index].as_str() {
            "--addr" => {
                index += 1;
                let value = args.get(index).unwrap_or_else(|| {
                    eprintln!("Uso: nexus serve --addr <addr> [--storage json|sqlite]");
                    process::exit(1);
                });
                if addr.replace(value.as_str()).is_some() {
                    eprintln!("Opcao --addr repetida");
                    process::exit(1);
                }
            }
            "--storage" | "--driver" => {
                if storage_driver_seen {
                    eprintln!("Opcao --storage repetida");
                    process::exit(1);
                }
                storage_driver_seen = true;
                index += 1;
                let value = args.get(index).unwrap_or_else(|| {
                    eprintln!("Uso: nexus serve [ficheiro.nx] [addr] [--storage json|sqlite]");
                    process::exit(1);
                });
                storage_driver = StorageDriver::parse(value).unwrap_or_else(|e| {
                    eprintln!("{}", e);
                    process::exit(1);
                });
            }
            option if option.starts_with("--") => {
                eprintln!("Opcao desconhecida para serve: '{}'", option);
                print_usage_and_exit(1);
            }
            value => {
                if file_path.is_none() {
                    file_path = Some(value);
                } else if addr.is_none() {
                    addr = Some(value);
                } else {
                    eprintln!("Uso: nexus serve [ficheiro.nx] [addr] [--storage json|sqlite]");
                    process::exit(1);
                }
            }
        }
        index += 1;
    }

    let file_path = file_path
        .map(PathBuf::from)
        .unwrap_or_else(|| project_entry_or_exit("serve [ficheiro.nx] [addr]"));
    let addr = addr.unwrap_or("127.0.0.1:5050").to_string();
    (file_path, addr, storage_driver)
}

fn run_storage_plan_command(args: &[String]) {
    let (file_path, storage_driver, apply) = storage_plan_entry_driver_and_apply(args);
    let (program, _) = nexuslang::load_and_check_with_graph(&file_path).unwrap_or_else(|e| {
        eprintln!("Erro de validação: {}", e);
        process::exit(1);
    });
    let data_dir = nexuslang::server::default_data_dir(file_path.to_string_lossy().as_ref());
    let storage = Storage::new_driver(storage_driver, &data_dir).unwrap_or_else(|e| {
        eprintln!("Erro ao abrir storage {}: {}", storage_driver, e);
        process::exit(1);
    });
    let plan = if apply {
        storage.apply_schema_migration_plan(&program)
    } else {
        storage.schema_migration_plan(&program)
    }
    .unwrap_or_else(|e| {
        eprintln!("Erro no plano de storage: {}", e);
        process::exit(1);
    });
    print!("{}", plan.render_text(apply));
}

fn storage_plan_entry_driver_and_apply(args: &[String]) -> (PathBuf, StorageDriver, bool) {
    let mut file_path: Option<&str> = None;
    let mut storage_driver = StorageDriver::Sqlite;
    let mut storage_driver_seen = false;
    let mut apply = false;
    let mut index = 2;

    while index < args.len() {
        match args[index].as_str() {
            "--apply" => {
                if apply {
                    eprintln!("Opcao --apply repetida");
                    process::exit(1);
                }
                apply = true;
            }
            "--storage" | "--driver" => {
                if storage_driver_seen {
                    eprintln!("Opcao --storage repetida");
                    process::exit(1);
                }
                storage_driver_seen = true;
                index += 1;
                let value = args.get(index).unwrap_or_else(|| {
                    eprintln!(
                        "Uso: nexus storage-plan [ficheiro.nx] [--storage json|sqlite] [--apply]"
                    );
                    process::exit(1);
                });
                storage_driver = StorageDriver::parse(value).unwrap_or_else(|e| {
                    eprintln!("{}", e);
                    process::exit(1);
                });
            }
            option if option.starts_with("--") => {
                eprintln!("Opcao desconhecida para storage-plan: '{}'", option);
                print_usage_and_exit(1);
            }
            value => {
                if file_path.is_some() {
                    eprintln!(
                        "Uso: nexus storage-plan [ficheiro.nx] [--storage json|sqlite] [--apply]"
                    );
                    process::exit(1);
                }
                file_path = Some(value);
            }
        }
        index += 1;
    }

    let file_path = file_path
        .map(PathBuf::from)
        .unwrap_or_else(|| project_entry_or_exit("storage-plan [ficheiro.nx]"));
    (file_path, storage_driver, apply)
}

fn project_entry_or_exit(usage: &str) -> PathBuf {
    match package_manager::project_entry_current_dir() {
        Ok(entry) => entry,
        Err(e) => {
            eprintln!("Uso: nexus {}", usage);
            eprintln!("Erro: {}", e);
            process::exit(1);
        }
    }
}

fn read_source(file_path: &str) -> String {
    match fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Erro ao ler '{}': {}", file_path, e);
            process::exit(1);
        }
    }
}

fn run_package_command(result: Result<package_manager::PackageReport, String>) {
    match result {
        Ok(report) => {
            println!("Projeto: {}", report.root.display());
            if report.manifest_created {
                println!("Criado: {}", package_manager::MANIFEST_FILE);
            }
            if let Some(name) = report.dependency_name.as_deref() {
                if report.dependency_added.unwrap_or(false) {
                    println!("Dependência adicionada: {}", name);
                } else {
                    println!("Dependência já existia: {}", name);
                }
                if let Some(source) = report.dependency_source.as_deref() {
                    println!("Origem: {}", source);
                }
            }
            if report.lock_written {
                println!("Gerado: {}", package_manager::LOCK_FILE);
            }
            println!("Dependências locais: {}", report.dependency_count);
        }
        Err(e) => {
            eprintln!("Erro no package manager: {}", e);
            process::exit(1);
        }
    }
}

fn parse_add_source(args: &[String]) -> package_manager::DependencyRequest<'_> {
    match args.len() {
        3 => package_manager::DependencyRequest::Local,
        5 if args[3] == "--path" => package_manager::DependencyRequest::Path(&args[4]),
        5 if args[3] == "--registry" => package_manager::DependencyRequest::Registry(&args[4]),
        _ => {
            eprintln!("Uso: nexus add <pacote> [--path <dir>|--registry <pacote@versao>]");
            process::exit(1);
        }
    }
}

fn run_repl() {
    println!("NexusLang REPL. Use :quit para sair, :clear para limpar o buffer.");
    let mut buffer = String::new();

    loop {
        print!("nexus> ");
        io::stdout().flush().ok();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            eprintln!("Erro ao ler entrada");
            process::exit(1);
        }

        let trimmed = line.trim();
        match trimmed {
            ":quit" | ":exit" => break,
            ":clear" => {
                buffer.clear();
                println!("Buffer limpo");
                continue;
            }
            "" => continue,
            _ => {}
        }

        buffer.push_str(&line);
        if let Err(e) = nexuslang::run_source(&buffer) {
            eprintln!("Erro: {}", e);
            eprintln!("Dica: use :clear para reiniciar o buffer.");
        }
    }
}

fn create_project(project_name: &str) -> Result<(), String> {
    let root = Path::new(project_name);
    if root.exists() {
        return Err(format!("'{}' já existe", project_name));
    }

    fs::create_dir_all(root.join("examples")).map_err(|e| e.to_string())?;
    fs::create_dir_all(root.join("tests")).map_err(|e| e.to_string())?;

    let main_nx = r#"model Customer {
    name: string
    balance: money
}

workflow Onboarding {
    step criar_cliente {
        print("Criando cliente")
    }
    step notificar {
        print("Notificando cliente")
    }
}

route GET /customers/:id {
    return "customer " + id
}

invoice {
    customer: "Cliente Exemplo"
    currency: "AOA"
    tax: 14
    item "Setup ERP" qty 1 price 250000 kz
}

print("Projeto NexusLang pronto")
run_workflow("Onboarding")
"#;

    let readme = format!(
        "# {}\n\nProjeto criado com `nexus new`.\n\n## Comandos\n\n```bash\nnexus check\nnexus run\nnexus test\nnexus docs --output docs.md\nnexus lint main.nx\nnexus fmt main.nx --write\n```\n",
        project_name
    );

    fs::write(root.join("main.nx"), main_nx).map_err(|e| e.to_string())?;
    fs::write(root.join("README.md"), readme).map_err(|e| e.to_string())?;
    fs::write(root.join("examples").join("invoice.nx"), main_nx).map_err(|e| e.to_string())?;
    fs::write(
        root.join("tests").join("smoke.nx"),
        "print(\"NexusLang smoke test\")\n",
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        root.join("tests").join("smoke.out"),
        "NexusLang smoke test\n",
    )
    .map_err(|e| e.to_string())?;
    let package_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(project_name);
    package_manager::write_new_project_package_files(root, package_name)?;

    println!("Projeto criado: {}", root.display());
    println!("Próximo passo:");
    println!("  cd {}", root.display());
    println!("  nexus install");
    println!("  nexus check");
    Ok(())
}
