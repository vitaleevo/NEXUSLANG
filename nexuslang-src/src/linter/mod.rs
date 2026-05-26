use crate::ast::*;

#[derive(Debug, Clone)]
pub struct LintWarning {
    pub code: &'static str,
    pub message: String,
}

pub fn lint_program(program: &Program) -> Vec<LintWarning> {
    let mut warnings = Vec::new();

    for decl in &program.decls {
        lint_decl(decl, &mut warnings);
    }

    warnings
}

fn lint_decl(decl: &Decl, warnings: &mut Vec<LintWarning>) {
    match decl {
        Decl::Function {
            name, params, body, ..
        } => {
            if !is_snake_case(name) {
                warn(
                    warnings,
                    "NXL001",
                    format!("Função '{}' deveria usar snake_case", name),
                );
            }
            for (param, _) in params {
                if !is_snake_case(param) {
                    warn(
                        warnings,
                        "NXL002",
                        format!("Parâmetro '{}' deveria usar snake_case", param),
                    );
                }
            }
            lint_stmts(body, warnings);
        }
        Decl::Model { name, fields, .. } => {
            if !is_pascal_case(name) {
                warn(
                    warnings,
                    "NXL003",
                    format!("Model '{}' deveria usar PascalCase", name),
                );
            }
            if fields.is_empty() {
                warn(
                    warnings,
                    "NXL004",
                    format!("Model '{}' não tem campos", name),
                );
            }
            for field in fields {
                if !is_snake_case(&field.name) {
                    warn(
                        warnings,
                        "NXL005",
                        format!("Campo '{}.{}' deveria usar snake_case", name, field.name),
                    );
                }
            }
        }
        Decl::Workflow { name, steps, .. } => {
            if steps.is_empty() {
                warn(
                    warnings,
                    "NXL006",
                    format!("Workflow '{}' não tem steps", name),
                );
            }
            for step in steps {
                if !is_snake_case(&step.name) {
                    warn(
                        warnings,
                        "NXL007",
                        format!("Step '{}.{}' deveria usar snake_case", name, step.name),
                    );
                }
                if step.body.is_empty() {
                    warn(
                        warnings,
                        "NXL008",
                        format!("Step '{}.{}' não executa ações", name, step.name),
                    );
                }
                lint_stmts(&step.body, warnings);
            }
        }
        Decl::Route { path, body, .. } => {
            if !path.starts_with('/') {
                warn(
                    warnings,
                    "NXL009",
                    format!("Route '{}' deveria começar com /", path),
                );
            }
            if body.is_empty() {
                warn(
                    warnings,
                    "NXL010",
                    format!("Route '{}' não tem corpo", path),
                );
            }
            lint_stmts(body, warnings);
        }
        Decl::Invoice { fields, items, .. } => {
            if !fields.iter().any(|f| f.key == "customer") {
                warn(
                    warnings,
                    "NXL011",
                    "Invoice deveria declarar customer".to_string(),
                );
            }
            if items.is_empty() {
                warn(
                    warnings,
                    "NXL012",
                    "Invoice não tem itens estruturados".to_string(),
                );
            }
        }
        Decl::Statement(stmt) => lint_stmt(stmt, warnings),
    }
}

fn lint_stmts(stmts: &[Stmt], warnings: &mut Vec<LintWarning>) {
    for stmt in stmts {
        lint_stmt(stmt, warnings);
    }
}

fn lint_stmt(stmt: &Stmt, warnings: &mut Vec<LintWarning>) {
    match stmt {
        Stmt::Let { name, .. } | Stmt::Const { name, .. } | Stmt::Assign { name, .. } => {
            if !is_snake_case(name) {
                warn(
                    warnings,
                    "NXL013",
                    format!("Variável '{}' deveria usar snake_case", name),
                );
            }
        }
        Stmt::If {
            then_body,
            else_body,
            ..
        } => {
            lint_stmts(then_body, warnings);
            if let Some(else_body) = else_body {
                lint_stmts(else_body, warnings);
            }
        }
        Stmt::While { body, .. } | Stmt::For { body, .. } => lint_stmts(body, warnings),
        Stmt::Return { .. } | Stmt::Print { .. } | Stmt::ExprStmt { .. } => {}
    }
}

fn warn(warnings: &mut Vec<LintWarning>, code: &'static str, message: String) {
    warnings.push(LintWarning { code, message });
}

fn is_snake_case(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !value.starts_with('_')
        && !value.ends_with('_')
        && !value.contains("__")
}

fn is_pascal_case(value: &str) -> bool {
    let Some(first) = value.chars().next() else {
        return false;
    };
    first.is_ascii_uppercase()
        && value.chars().all(|c| c.is_ascii_alphanumeric())
        && !value.contains('_')
}
