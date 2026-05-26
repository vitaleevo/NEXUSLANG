#![allow(clippy::too_many_arguments)]

pub mod ast;
pub mod checker;
pub mod diagnostic;
pub mod formatter;
pub mod interpreter;
pub mod lexer;
pub mod linter;
pub mod parser;
pub mod playground;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

use checker::Checker;
use diagnostic::Diagnostic;
use formatter::format_program;
use interpreter::Interpreter;
use lexer::Lexer;
use linter::{lint_program, LintWarning};
use parser::Parser;

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
    interp.run(&program)
}

pub fn run_source_captured(source: &str) -> Result<Vec<String>, String> {
    let program = parse_checked_source(source)?;
    let mut interp = Interpreter::new_captured();
    interp.run(&program)?;
    Ok(interp.output().to_vec())
}

pub fn check_source(source: &str) -> Result<(), String> {
    parse_checked_source_diagnostic(source)
        .map(|_| ())
        .map_err(|diagnostic| diagnostic.to_string())
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

pub fn ast_source(source: &str) -> Result<ast::Program, String> {
    parse_source(source)
}

pub fn parse_source(source: &str) -> Result<ast::Program, String> {
    parse_source_diagnostic(source).map_err(|diagnostic| diagnostic.to_string())
}

pub fn parse_source_diagnostic(source: &str) -> Result<ast::Program, Diagnostic> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize_spanned_diagnostic()?;
    let mut parser = Parser::new_spanned(tokens);
    parser.parse_program_diagnostic()
}

pub fn parse_checked_source(source: &str) -> Result<ast::Program, String> {
    parse_checked_source_diagnostic(source).map_err(|diagnostic| diagnostic.to_string())
}

pub fn parse_checked_source_diagnostic(source: &str) -> Result<ast::Program, Diagnostic> {
    let program = parse_source_diagnostic(source)?;
    let mut checker = Checker::new();
    checker.check_diagnostic(&program)?;
    Ok(program)
}

pub fn fmt_source(source: &str) -> Result<String, String> {
    let program = ast_source(source)?;
    Ok(format_program(&program))
}

pub fn lint_source(source: &str) -> Result<Vec<LintWarning>, String> {
    let program = ast_source(source)?;
    let mut checker = Checker::new();
    checker.check(&program)?;
    Ok(lint_program(&program))
}
