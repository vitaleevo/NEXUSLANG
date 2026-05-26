use std::fmt;

use crate::ast::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticStage {
    Input,
    Lexer,
    Parser,
    Checker,
    Runtime,
}

impl DiagnosticStage {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticStage::Input => "input",
            DiagnosticStage::Lexer => "lexer",
            DiagnosticStage::Parser => "parser",
            DiagnosticStage::Checker => "checker",
            DiagnosticStage::Runtime => "runtime",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub stage: DiagnosticStage,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl Diagnostic {
    pub fn new(stage: DiagnosticStage, message: impl Into<String>) -> Self {
        Diagnostic {
            stage,
            message: message.into(),
            line: None,
            column: None,
        }
    }

    pub fn parser(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self::new(DiagnosticStage::Parser, message).with_location(line, column)
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
