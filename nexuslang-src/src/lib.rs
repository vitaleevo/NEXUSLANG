#![allow(clippy::too_many_arguments)]
#![allow(clippy::result_large_err)]
#![allow(deprecated)]

pub mod ast;
pub mod auth_ops;
pub mod checker;
pub mod diagnostic;
pub mod diagnostics;
pub mod docs;
pub mod formatter;
pub mod hir;
pub mod interpreter;
pub mod lexer;
pub mod linter;
pub mod model_ops;
pub mod module_loader;
pub mod package_manager;
pub mod parser;
pub mod playground;
pub mod route_hir;
pub mod runtime_env;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
pub mod test_runner;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

use checker::Checker;
use diagnostic::Diagnostic;
use docs::document_program;
use formatter::format_program;
use interpreter::Interpreter;
use lexer::Lexer;
use linter::{lint_program, LintWarning};
use parser::Parser;
use std::path::Path;

#[deprecated(
    note = "use run_source_diagnostic or parse_checked_source_diagnostic for structured diagnostics"
)]
pub fn run_source(source: &str) -> Result<(), String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer
        .tokenize_spanned_diagnostic()
        .map_err(|diagnostic| diagnostic.to_string())?;
    let mut parser = Parser::new_spanned(tokens);
    let program = parser.parse_program()?;
    let mut checker = Checker::new();
    checker.check(&program)?;
    let mut interp = Interpreter::new();
    interp.run(&program).map_err(|d| d.to_string())
}

#[deprecated(
    note = "use parse_checked_source_diagnostic + Interpreter::new_captured for structured diagnostics"
)]
pub fn run_source_captured(source: &str) -> Result<Vec<String>, String> {
    let program = parse_checked_source(source)?;
    let mut interp = Interpreter::new_captured();
    interp.run(&program).map_err(|d| d.to_string())?;
    Ok(interp.output().to_vec())
}

#[deprecated(note = "use parse_checked_source_diagnostic for structured diagnostics")]
pub fn check_source(source: &str) -> Result<(), String> {
    parse_checked_source_diagnostic(source)
        .map(|_| ())
        .map_err(|diagnostic| diagnostic.to_string())
}

#[deprecated(note = "use parse_source_diagnostic for structured diagnostics")]
pub fn ast_source(source: &str) -> Result<ast::Program, String> {
    parse_source(source)
}

#[deprecated(note = "use parse_source_diagnostic for structured diagnostics")]
pub fn parse_source(source: &str) -> Result<ast::Program, String> {
    parse_source_diagnostic(source).map_err(|diagnostic| diagnostic.to_string())
}

#[deprecated(note = "use parse_checked_source_diagnostic for structured diagnostics")]
pub fn parse_checked_source(source: &str) -> Result<ast::Program, String> {
    parse_checked_source_diagnostic(source).map_err(|diagnostic| diagnostic.to_string())
}

pub fn tokens_source(source: &str) -> Vec<(lexer::Token, usize)> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize()
}

pub fn tokens_source_spanned(source: &str) -> Vec<(lexer::Token, usize, usize)> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize_spanned()
}

pub fn tokens_source_spanned_diagnostic(
    source: &str,
) -> Result<Vec<(lexer::Token, usize, usize)>, Diagnostic> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize_spanned_diagnostic()
}

pub fn parse_source_diagnostic(source: &str) -> Result<ast::Program, Diagnostic> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize_spanned_diagnostic()?;
    let mut parser = Parser::new_spanned(tokens);
    parser.parse_program_diagnostic()
}

pub fn parse_checked_source_diagnostic(source: &str) -> Result<ast::Program, Diagnostic> {
    let program = parse_source_diagnostic(source)?;
    let mut checker = Checker::new();
    checker.check_diagnostic(&program)?;
    Ok(program)
}

#[deprecated(
    note = "use ast_source_diagnostic (parse_source_diagnostic) for structured diagnostics"
)]
pub fn fmt_source(source: &str) -> Result<String, String> {
    let program = ast_source(source)?;
    Ok(format_program(&program))
}

#[deprecated(note = "use parse_checked_source_diagnostic for structured diagnostics")]
pub fn lint_source(source: &str) -> Result<Vec<LintWarning>, String> {
    let program = ast_source(source)?;
    let mut checker = Checker::new();
    checker.check(&program)?;
    Ok(lint_program(&program))
}

#[deprecated(note = "use parse_checked_source_diagnostic for structured diagnostics")]
pub fn docs_source(source: &str, title: Option<&str>) -> Result<String, String> {
    let program = parse_checked_source(source)?;
    Ok(document_program(&program, title))
}

#[deprecated(
    note = "use module_loader::load_program or load_and_check_with_source_database_diagnostic"
)]
pub fn load_program(entry_path: &std::path::Path) -> Result<ast::Program, String> {
    module_loader::load_program(entry_path).map_err(|e| e.to_string())
}

#[deprecated(
    note = "use load_and_check_with_source_database_diagnostic for structured diagnostics"
)]
pub fn load_and_check(entry_path: &std::path::Path) -> Result<ast::Program, String> {
    let program = load_program(entry_path)?;
    let mut checker = Checker::new();
    checker.check(&program)?;
    Ok(program)
}

#[deprecated(
    note = "use load_and_check_with_source_database_diagnostic for structured diagnostics"
)]
pub fn load_and_check_with_graph(
    entry_path: &std::path::Path,
) -> Result<(ast::Program, module_loader::ModuleGraph), String> {
    let (program, module_graph, decl_module_map) =
        module_loader::load_program_full(entry_path).map_err(|e| e.to_string())?;
    let mut checker = Checker::new();
    checker
        .check_with_module_graph(&program, &module_graph, &decl_module_map)
        .map_err(|d| d.to_string())?;
    Ok((program, module_graph))
}

#[deprecated(note = "use load_and_run_with_source_database_diagnostic for structured diagnostics")]
pub fn load_and_run(entry_path: &std::path::Path) -> Result<(), String> {
    let program = load_and_check(entry_path)?;
    let mut interp = Interpreter::new();
    interp.run(&program).map_err(|d| d.to_string())
}

#[deprecated(note = "use load_and_run_with_source_database_diagnostic for structured diagnostics")]
pub fn load_and_run_with_graph(entry_path: &std::path::Path) -> Result<(), String> {
    let (program, _module_graph) = load_and_check_with_graph(entry_path)?;
    let mut interp = Interpreter::new();
    interp.run(&program).map_err(|d| d.to_string())
}

pub fn docs_entry(entry_path: &Path, title: Option<&str>) -> Result<String, MultiModuleDiagnostic> {
    let checked = load_and_check_with_source_database_diagnostic(entry_path)?;
    Ok(document_program(&checked.program, title))
}

// ─── Re-exports from diagnostics module ──────────────────────────────────
pub use diagnostics::{
    CheckedMultiModuleProgram, MultiModuleDiagnostic, MultiModuleDiagnosticGroup,
    MultiModuleDiagnosticReport, MultiModuleDiagnosticReportSourceView,
    MultiModuleDiagnosticReportSummary, MultiModuleDiagnosticReportView,
    MultiModuleDiagnosticSeverityCount, MultiModuleDiagnosticSourceContext,
    MultiModuleDiagnosticStageCount, MultiModuleDiagnosticToolingItem,
    MultiModuleDiagnosticToolingItemWithSourceContext, MultiModuleRunDiagnosticReport,
    MULTI_MODULE_DIAGNOSTIC_JSON_SCHEMA_VERSION,
};

// Re-export MultiModuleRunDiagnostic with its original name
pub use diagnostics::MultiModuleRunDiagnostic;

// Re-export JSON formatters from diagnostics module
pub use diagnostics::{
    multi_module_diagnostic_json, multi_module_diagnostic_output_json,
    multi_module_diagnostic_report_json, multi_module_diagnostic_report_output_json,
    multi_module_success_json, multi_module_success_output_json,
};

// Re-export load/check/run pipeline from diagnostics module
pub use diagnostics::{
    check_with_source_database, check_with_source_database_diagnostic_report,
    load_and_check_with_source_database, load_and_check_with_source_database_diagnostic,
    load_and_check_with_source_database_diagnostic_report, load_and_run_with_source_database,
    load_and_run_with_source_database_captured_diagnostic,
    load_and_run_with_source_database_captured_diagnostic_report,
    load_and_run_with_source_database_diagnostic,
};
