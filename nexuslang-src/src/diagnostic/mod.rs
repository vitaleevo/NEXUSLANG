use std::fmt;

use crate::ast::Span;

pub mod codes {
    pub const INPUT_GENERIC: &str = "NXL0001";

    pub const LEXER_GENERIC: &str = "NXL1099";
    pub const LEXER_INVALID_CHARACTER: &str = "NXL1001";
    pub const LEXER_UNTERMINATED_STRING: &str = "NXL1002";
    pub const LEXER_INVALID_OPERATOR: &str = "NXL1003";

    pub const PARSER_SYNTAX: &str = "NXL2001";
    pub const PARSER_IMPORT: &str = "NXL2002";
    pub const PARSER_EXPORT: &str = "NXL2003";
    pub const PARSER_DECLARATION: &str = "NXL2004";
    pub const PARSER_EXPRESSION: &str = "NXL2005";
    pub const PARSER_STATEMENT: &str = "NXL2006";

    pub const CHECKER_TYPE: &str = "NXL3001";
    pub const CHECKER_SYMBOL: &str = "NXL3002";
    pub const CHECKER_ASSIGNMENT: &str = "NXL3003";
    pub const CHECKER_MODEL: &str = "NXL3004";
    pub const CHECKER_ROUTE: &str = "NXL3005";
    pub const CHECKER_AUTH: &str = "NXL3006";
    pub const CHECKER_WORKFLOW: &str = "NXL3007";
    pub const CHECKER_INVOICE: &str = "NXL3008";
    pub const CHECKER_ARGUMENT: &str = "NXL3009";
    pub const CHECKER_GENERIC: &str = "NXL3099";

    pub const MODULE_LOADER_IO: &str = "NXL4001";
    pub const MODULE_LOADER_PARSE: &str = "NXL4002";
    pub const MODULE_LOADER_CIRCULAR_DEPENDENCY: &str = "NXL4003";
    pub const MODULE_LOADER_SYMBOL_NOT_EXPORTED: &str = "NXL4004";
    pub const MODULE_LOADER_DUPLICATE_SYMBOL: &str = "NXL4005";
    pub const MODULE_LOADER_DUPLICATE_ALIAS: &str = "NXL4006";
    pub const MODULE_LOADER_ALIAS_COLLISION: &str = "NXL4007";
    pub const MODULE_LOADER_PATH: &str = "NXL4008";
    pub const MODULE_LOADER_PACKAGE: &str = "NXL4009";
    pub const MODULE_LOADER_STDLIB: &str = "NXL4010";

    pub const RUNTIME_DIVISION_BY_ZERO: &str = "NXL5001";
    pub const RUNTIME_UNDEFINED_VARIABLE: &str = "NXL5002";
    pub const RUNTIME_UNDEFINED_FUNCTION: &str = "NXL5003";
    pub const RUNTIME_MODEL: &str = "NXL5004";
    pub const RUNTIME_WORKFLOW: &str = "NXL5005";
    pub const RUNTIME_ASSERTION: &str = "NXL5006";
    pub const RUNTIME_GENERIC: &str = "NXL5099";
}

fn contains_any(message: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| message.contains(pattern))
}

pub fn parser_code_for_message(message: &str) -> &'static str {
    let message = message.to_lowercase();

    if contains_any(&message, &["export"]) {
        codes::PARSER_EXPORT
    } else if contains_any(&message, &["import"]) {
        codes::PARSER_IMPORT
    } else if contains_any(&message, &["declara", "top-level"]) {
        codes::PARSER_DECLARATION
    } else if contains_any(&message, &["expressão", "expressao"]) {
        codes::PARSER_EXPRESSION
    } else if contains_any(&message, &["statement", "return", "if ", "while "]) {
        codes::PARSER_STATEMENT
    } else {
        codes::PARSER_SYNTAX
    }
}

pub fn checker_code_for_message(message: &str) -> &'static str {
    let message = message.to_lowercase();

    if contains_any(&message, &["invoice", "fatura"]) {
        codes::CHECKER_INVOICE
    } else if contains_any(&message, &["route", "rota", "http"]) {
        codes::CHECKER_ROUTE
    } else if contains_any(&message, &["auth", "identity"]) {
        codes::CHECKER_AUTH
    } else if contains_any(&message, &["workflow", "run_workflow"]) {
        codes::CHECKER_WORKFLOW
    } else if contains_any(
        &message,
        &["model ", "model '", "modelo", "::", "campo ", "field "],
    ) {
        codes::CHECKER_MODEL
    } else if contains_any(
        &message,
        &["constante", "reatribu", "atribuição", "atribuicao"],
    ) {
        codes::CHECKER_ASSIGNMENT
    } else if contains_any(
        &message,
        &[
            "argumento",
            "argumentos",
            "recebe exatamente",
            "recebe ",
            "espera ",
        ],
    ) {
        codes::CHECKER_ARGUMENT
    } else if contains_any(
        &message,
        &[
            "tipo",
            "type",
            "retorno",
            "inválido",
            "invalido",
            "incompat",
            "condição",
            "condicao",
            "operador",
            "array",
            "optional",
        ],
    ) {
        codes::CHECKER_TYPE
    } else if contains_any(
        &message,
        &[
            "não definida",
            "não definido",
            "nao definida",
            "nao definido",
            "não encontrado",
            "nao encontrado",
            "desconhecido",
            "desconhecida",
            "declarada mais de uma vez",
            "declarado mais de uma vez",
        ],
    ) {
        codes::CHECKER_SYMBOL
    } else {
        codes::CHECKER_GENERIC
    }
}

pub fn runtime_code_for_message(message: &str) -> &'static str {
    let message = message.to_lowercase();

    if contains_any(
        &message,
        &[
            "divisão por zero",
            "divisao por zero",
            "módulo por zero",
            "modulo por zero",
        ],
    ) {
        codes::RUNTIME_DIVISION_BY_ZERO
    } else if contains_any(&message, &["variável", "variavel"]) {
        codes::RUNTIME_UNDEFINED_VARIABLE
    } else if contains_any(&message, &["função", "funcao"]) {
        codes::RUNTIME_UNDEFINED_FUNCTION
    } else if contains_any(&message, &["workflow", "run_workflow"]) {
        codes::RUNTIME_WORKFLOW
    } else if contains_any(
        &message,
        &["assert_true", "assert_eq", "assert_ne", "assert_contains"],
    ) {
        codes::RUNTIME_ASSERTION
    } else if contains_any(&message, &["model", "modelo", "campo", "field"]) {
        codes::RUNTIME_MODEL
    } else {
        codes::RUNTIME_GENERIC
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticStage {
    Input,
    Lexer,
    Parser,
    Checker,
    ModuleLoader,
    Runtime,
}

impl DiagnosticStage {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticStage::Input => "input",
            DiagnosticStage::Lexer => "lexer",
            DiagnosticStage::Parser => "parser",
            DiagnosticStage::Checker => "checker",
            DiagnosticStage::ModuleLoader => "module_loader",
            DiagnosticStage::Runtime => "runtime",
        }
    }

    pub fn default_code(self) -> &'static str {
        match self {
            DiagnosticStage::Input => codes::INPUT_GENERIC,
            DiagnosticStage::Lexer => codes::LEXER_GENERIC,
            DiagnosticStage::Parser => codes::PARSER_SYNTAX,
            DiagnosticStage::Checker => codes::CHECKER_TYPE,
            DiagnosticStage::ModuleLoader => codes::MODULE_LOADER_IO,
            DiagnosticStage::Runtime => codes::RUNTIME_DIVISION_BY_ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl DiagnosticSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Info => "info",
            DiagnosticSeverity::Hint => "hint",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticOwner {
    pub decl_index: usize,
    pub module_id: Option<usize>,
}

impl DiagnosticOwner {
    pub fn new(decl_index: usize) -> Self {
        DiagnosticOwner {
            decl_index,
            module_id: None,
        }
    }

    pub fn with_module_id(mut self, module_id: usize) -> Self {
        self.module_id = Some(module_id);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLabel {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl DiagnosticLabel {
    pub fn new(message: impl Into<String>) -> Self {
        DiagnosticLabel {
            message: message.into(),
            line: None,
            column: None,
        }
    }

    pub fn at_location(message: impl Into<String>, line: usize, column: usize) -> Self {
        DiagnosticLabel::new(message).with_location(line, column)
    }

    pub fn with_location(mut self, line: usize, column: usize) -> Self {
        self.line = if line == 0 { None } else { Some(line) };
        self.column = if column == 0 { None } else { Some(column) };
        self
    }

    pub fn with_span(self, span: Span) -> Self {
        self.with_location(span.line, span.column)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSuggestion {
    pub message: String,
    pub replacement: Option<String>,
}

impl DiagnosticSuggestion {
    pub fn new(message: impl Into<String>) -> Self {
        DiagnosticSuggestion {
            message: message.into(),
            replacement: None,
        }
    }

    pub fn with_replacement(message: impl Into<String>, replacement: impl Into<String>) -> Self {
        DiagnosticSuggestion {
            message: message.into(),
            replacement: Some(replacement.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub stage: DiagnosticStage,
    pub code: Option<String>,
    pub severity: Option<DiagnosticSeverity>,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub owner: Option<DiagnosticOwner>,
    pub labels: Vec<DiagnosticLabel>,
    pub notes: Vec<String>,
    pub suggestions: Vec<DiagnosticSuggestion>,
}

impl Diagnostic {
    pub fn new(stage: DiagnosticStage, message: impl Into<String>) -> Self {
        Diagnostic {
            stage,
            code: Some(stage.default_code().to_string()),
            severity: Some(DiagnosticSeverity::Error),
            message: message.into(),
            line: None,
            column: None,
            owner: None,
            labels: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn parser(message: impl Into<String>, line: usize, column: usize) -> Self {
        let message = message.into();
        let code = parser_code_for_message(&message);
        let diagnostic = Self::new(DiagnosticStage::Parser, message)
            .with_code(code)
            .with_location(line, column);
        enrich_parser_diagnostic(diagnostic, code, line, column)
    }

    pub fn lexer(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self::new(DiagnosticStage::Lexer, message).with_location(line, column)
    }

    pub fn with_location(mut self, line: usize, column: usize) -> Self {
        self.line = if line == 0 { None } else { Some(line) };
        self.column = if column == 0 { None } else { Some(column) };
        self
    }

    pub fn with_span(self, span: Span) -> Self {
        self.with_location(span.line, span.column)
    }

    pub fn with_owner(mut self, owner: DiagnosticOwner) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn without_code(mut self) -> Self {
        self.code = None;
        self
    }

    pub fn with_severity(mut self, severity: DiagnosticSeverity) -> Self {
        self.severity = Some(severity);
        self
    }

    pub fn without_severity(mut self) -> Self {
        self.severity = None;
        self
    }

    pub fn with_label(mut self, label: impl Into<DiagnosticLabel>) -> Self {
        self.labels.push(label.into());
        self
    }

    pub fn with_label_at(self, message: impl Into<String>, line: usize, column: usize) -> Self {
        self.with_label(DiagnosticLabel::at_location(message, line, column))
    }

    pub fn without_labels(mut self) -> Self {
        self.labels.clear();
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn without_notes(mut self) -> Self {
        self.notes.clear();
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<DiagnosticSuggestion>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    pub fn with_replacement_suggestion(
        self,
        message: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        self.with_suggestion(DiagnosticSuggestion::with_replacement(message, replacement))
    }

    pub fn without_suggestions(mut self) -> Self {
        self.suggestions.clear();
        self
    }
}

impl From<&str> for DiagnosticLabel {
    fn from(message: &str) -> Self {
        DiagnosticLabel::new(message)
    }
}

impl From<String> for DiagnosticLabel {
    fn from(message: String) -> Self {
        DiagnosticLabel::new(message)
    }
}

impl From<&str> for DiagnosticSuggestion {
    fn from(message: &str) -> Self {
        DiagnosticSuggestion::new(message)
    }
}

impl From<String> for DiagnosticSuggestion {
    fn from(message: String) -> Self {
        DiagnosticSuggestion::new(message)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.column) {
            (Some(line), Some(column)) => {
                write!(f, "Linha {}, coluna {}: {}", line, column, self.message)
            }
            (Some(line), None) => write!(f, "Linha {}: {}", line, self.message),
            _ => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for Diagnostic {}

fn enrich_parser_diagnostic(
    diagnostic: Diagnostic,
    code: &str,
    line: usize,
    column: usize,
) -> Diagnostic {
    match code {
        codes::PARSER_IMPORT => diagnostic
            .with_label_at("sintaxe do import", line, column)
            .with_note("Imports usam a forma: import Nome [as Alias] from \"./modulo.nx\".")
            .with_suggestion("Use: import Nome from \"./modulo.nx\""),
        codes::PARSER_EXPORT => diagnostic
            .with_label_at("sintaxe do export", line, column)
            .with_note("Exports sao suportados para fn, model, workflow e auth.")
            .with_suggestion("Coloque export antes de uma declaracao nomeada suportada."),
        _ => diagnostic,
    }
}

pub(crate) fn enrich_runtime_diagnostic(diagnostic: Diagnostic, code: &str) -> Diagnostic {
    match code {
        codes::RUNTIME_DIVISION_BY_ZERO => diagnostic
            .with_label("operacao aritmetica em runtime")
            .with_note("A execucao tentou dividir ou calcular modulo por zero.")
            .with_suggestion("Garanta que o divisor seja diferente de zero antes da operacao."),
        codes::RUNTIME_UNDEFINED_VARIABLE => diagnostic
            .with_label("variavel acessada em runtime")
            .with_note("A execucao tentou ler uma variavel que nao existe no escopo atual.")
            .with_suggestion("Declare a variavel antes do uso ou corrija o nome."),
        codes::RUNTIME_UNDEFINED_FUNCTION => diagnostic
            .with_label("funcao chamada em runtime")
            .with_note("A execucao tentou chamar uma funcao que nao existe no programa carregado.")
            .with_suggestion("Declare a funcao antes do uso, importe-a ou corrija o nome."),
        codes::RUNTIME_MODEL => diagnostic
            .with_label("model usado em runtime")
            .with_note("A execucao tentou acessar um model ou campo indisponivel.")
            .with_suggestion("Declare ou importe o model esperado, ou corrija o nome."),
        codes::RUNTIME_WORKFLOW => diagnostic
            .with_label("workflow chamado em runtime")
            .with_note("A execucao tentou iniciar um workflow que nao foi encontrado.")
            .with_suggestion("Declare o workflow esperado ou corrija o nome."),
        codes::RUNTIME_ASSERTION => diagnostic
            .with_label("assertion falhou em runtime")
            .with_note("A execucao parou porque uma assertion de teste nao passou.")
            .with_suggestion("Ajuste o valor esperado ou corrija o comportamento testado."),
        _ => diagnostic,
    }
}
