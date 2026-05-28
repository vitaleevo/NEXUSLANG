use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use nexuslang::lexer::Token;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, Diagnostic,
    DiagnosticRelatedInformation, DiagnosticSeverity, DocumentSymbol, DocumentSymbolResponse,
    Hover, HoverContents, Location, MarkupContent, MarkupKind, NumberOrString, Position, Range,
    SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend, SymbolKind, Url,
};

#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    uri: Url,
    version: Option<i32>,
    text: String,
}

impl DocumentSnapshot {
    pub fn new(uri: Url, version: Option<i32>, text: String) -> Self {
        Self { uri, version, text }
    }

    pub fn uri(&self) -> &Url {
        &self.uri
    }

    pub fn version(&self) -> Option<i32> {
        self.version
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn file_path(&self) -> Option<PathBuf> {
        self.uri.to_file_path().ok()
    }

    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut lsp_diags = Vec::new();
        if let Err(diagnostic) = nexuslang::parse_checked_source_diagnostic(&self.text) {
            lsp_diags.push(diagnostic_to_lsp(&self.uri, &diagnostic));
        }
        lsp_diags
    }

    pub fn hover(&self, position: Position) -> Option<Hover> {
        hover_for_source(&self.text, position)
    }

    pub fn completion(&self) -> CompletionResponse {
        CompletionResponse::Array(completion_items_for_source(&self.text))
    }

    pub fn goto_definition(&self, position: Position) -> Option<Location> {
        definition_for_source(&self.uri, &self.text, position)
    }

    pub fn semantic_tokens(&self) -> SemanticTokens {
        semantic_tokens_for_source(&self.text)
    }

    pub fn document_symbols(&self) -> DocumentSymbolResponse {
        document_symbols_for_source(&self.text)
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticPublishBatch {
    pub uri: Url,
    pub version: Option<i32>,
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticPublishBatch {
    fn new(uri: Url, version: Option<i32>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            uri,
            version,
            diagnostics,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct LspCore {
    documents: HashMap<Url, DocumentSnapshot>,
    diagnostic_publication_groups: HashMap<Url, HashSet<Url>>,
}

impl LspCore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_document(&mut self, uri: Url, version: Option<i32>, text: String) {
        self.upsert_document(uri, version, text);
    }

    pub fn change_document(&mut self, uri: Url, version: Option<i32>, text: String) {
        self.upsert_document(uri, version, text);
    }

    pub fn close_document(&mut self, uri: &Url) -> Option<DocumentSnapshot> {
        let snapshot = self.documents.remove(uri);
        self.diagnostic_publication_groups.remove(uri);
        snapshot
    }

    pub fn close_document_publish_batches(&mut self, uri: &Url) -> Vec<DiagnosticPublishBatch> {
        self.documents.remove(uri);
        let mut stale_uris = self
            .diagnostic_publication_groups
            .remove(uri)
            .unwrap_or_default();
        stale_uris.insert(uri.clone());
        stale_uris
            .into_iter()
            .filter(|stale_uri| !self.diagnostic_uri_has_other_publisher(uri, stale_uri))
            .map(|uri| DiagnosticPublishBatch::new(uri, None, Vec::new()))
            .collect()
    }

    pub fn document(&self, uri: &Url) -> Option<&DocumentSnapshot> {
        self.documents.get(uri)
    }

    pub fn diagnostics_for(&self, uri: &Url) -> Option<Vec<Diagnostic>> {
        self.document(uri).map(DocumentSnapshot::diagnostics)
    }

    pub fn document_snapshot_matches(&self, uri: &Url, other: &LspCore) -> bool {
        let mut uris = other
            .diagnostic_publication_groups
            .get(uri)
            .cloned()
            .unwrap_or_default();
        uris.insert(uri.clone());

        uris.into_iter()
            .all(|uri| self.document_text_and_version_match(&uri, other))
    }

    fn document_text_and_version_match(&self, uri: &Url, other: &LspCore) -> bool {
        match (self.document(uri), other.document(uri)) {
            (Some(current), Some(candidate)) => {
                current.version() == candidate.version() && current.text() == candidate.text()
            }
            (None, None) => true,
            _ => false,
        }
    }

    pub fn sync_diagnostic_publication_group_from(&mut self, entry_uri: &Url, source: &LspCore) {
        if let Some(group) = source.diagnostic_publication_groups.get(entry_uri) {
            self.diagnostic_publication_groups
                .insert(entry_uri.clone(), group.clone());
        } else {
            self.diagnostic_publication_groups.remove(entry_uri);
        }
    }

    pub fn diagnostic_publish_batches_for(
        &mut self,
        uri: &Url,
    ) -> Option<Vec<DiagnosticPublishBatch>> {
        let snapshot = self.document(uri)?;
        let batches = self
            .multi_file_diagnostic_batches(snapshot)
            .unwrap_or_else(|| vec![single_document_diagnostic_batch(snapshot)]);
        Some(self.with_stale_diagnostic_clears(uri, batches))
    }

    pub fn hover(&self, uri: &Url, position: Position) -> Option<Hover> {
        self.document(uri)
            .and_then(|snapshot| snapshot.hover(position))
    }

    pub fn completion(&self, uri: &Url) -> Option<CompletionResponse> {
        self.document(uri).map(DocumentSnapshot::completion)
    }

    pub fn goto_definition(&self, uri: &Url, position: Position) -> Option<Location> {
        let snapshot = self.document(uri)?;
        self.multi_file_goto_definition(snapshot, position)
            .or_else(|| snapshot.goto_definition(position))
    }

    pub fn semantic_tokens(&self, uri: &Url) -> Option<SemanticTokens> {
        self.document(uri).map(DocumentSnapshot::semantic_tokens)
    }

    pub fn document_symbols(&self, uri: &Url) -> Option<DocumentSymbolResponse> {
        self.document(uri).map(DocumentSnapshot::document_symbols)
    }

    fn upsert_document(&mut self, uri: Url, version: Option<i32>, text: String) {
        let snapshot = DocumentSnapshot::new(uri.clone(), version, text);
        self.documents.insert(uri, snapshot);
    }

    fn multi_file_diagnostic_batches(
        &self,
        snapshot: &DocumentSnapshot,
    ) -> Option<Vec<DiagnosticPublishBatch>> {
        let entry_path = snapshot.file_path()?;
        if !snapshot_matches_disk(snapshot, &entry_path) {
            return None;
        }

        match nexuslang::module_loader::load_program_full_with_source_database(&entry_path) {
            Ok((program, module_graph, decl_module_map, source_database)) => {
                if !self.source_database_matches_open_documents(&source_database) {
                    return None;
                }

                match nexuslang::check_with_source_database_diagnostic_report(
                    &program,
                    &module_graph,
                    &decl_module_map,
                    &source_database,
                ) {
                    Ok(()) => Some(self.empty_batches_for_source_database(&source_database)),
                    Err(report) => {
                        Some(self.report_batches(&report, Some(&source_database), snapshot.uri()))
                    }
                }
            }
            Err(error) => {
                let report = nexuslang::MultiModuleDiagnosticReport::from_diagnostic(
                    nexuslang::MultiModuleDiagnostic::from_module_error(error),
                );
                Some(self.report_batches(&report, None, snapshot.uri()))
            }
        }
    }

    fn multi_file_goto_definition(
        &self,
        snapshot: &DocumentSnapshot,
        position: Position,
    ) -> Option<Location> {
        let entry_path = snapshot.file_path()?;
        if !snapshot_matches_disk(snapshot, &entry_path) {
            return None;
        }

        let (program, module_graph, decl_module_map, source_database) =
            nexuslang::module_loader::load_program_full_with_source_database(&entry_path).ok()?;
        if !self.source_database_matches_open_documents(&source_database) {
            return None;
        }

        cross_file_definition_for_snapshot(
            snapshot,
            position,
            &program,
            &module_graph,
            &decl_module_map,
            &source_database,
        )
    }

    fn source_database_matches_open_documents(
        &self,
        source_database: &nexuslang::module_loader::SourceDatabase,
    ) -> bool {
        source_database.modules().iter().all(|module| {
            let Some(uri) = file_url_for_path(&module.path) else {
                return true;
            };
            self.documents
                .get(&uri)
                .map(|document| document.text() == module.source)
                .unwrap_or(true)
        })
    }

    fn empty_batches_for_source_database(
        &self,
        source_database: &nexuslang::module_loader::SourceDatabase,
    ) -> Vec<DiagnosticPublishBatch> {
        source_database
            .modules()
            .iter()
            .filter_map(|module| {
                let uri = file_url_for_path(&module.path)?;
                Some(DiagnosticPublishBatch::new(
                    uri.clone(),
                    self.version_for_uri(&uri),
                    Vec::new(),
                ))
            })
            .collect()
    }

    fn report_batches(
        &self,
        report: &nexuslang::MultiModuleDiagnosticReport,
        source_database: Option<&nexuslang::module_loader::SourceDatabase>,
        fallback_uri: &Url,
    ) -> Vec<DiagnosticPublishBatch> {
        let mut batches = source_database
            .map(|source_database| self.empty_batches_for_source_database(source_database))
            .unwrap_or_default();

        if !batches.iter().any(|batch| batch.uri == *fallback_uri) {
            batches.push(DiagnosticPublishBatch::new(
                fallback_uri.clone(),
                self.version_for_uri(fallback_uri),
                Vec::new(),
            ));
        }

        for diagnostic in report.diagnostics() {
            let uri = multi_module_diagnostic_uri(diagnostic, source_database)
                .unwrap_or_else(|| fallback_uri.clone());
            let version = self.version_for_uri(&uri);
            let lsp_diagnostic = multi_module_diagnostic_to_lsp(&uri, diagnostic);
            push_diagnostic_batch(&mut batches, uri, version, lsp_diagnostic);
        }

        batches
    }

    fn version_for_uri(&self, uri: &Url) -> Option<i32> {
        self.documents.get(uri).and_then(DocumentSnapshot::version)
    }

    fn with_stale_diagnostic_clears(
        &mut self,
        entry_uri: &Url,
        mut batches: Vec<DiagnosticPublishBatch>,
    ) -> Vec<DiagnosticPublishBatch> {
        let current_uris = batches
            .iter()
            .map(|batch| batch.uri.clone())
            .collect::<HashSet<_>>();

        let stale_uris = self.stale_diagnostic_clear_uris(entry_uri, &current_uris);

        for uri in stale_uris {
            batches.push(DiagnosticPublishBatch::new(
                uri.clone(),
                self.version_for_uri(&uri),
                Vec::new(),
            ));
        }

        self.diagnostic_publication_groups
            .insert(entry_uri.clone(), current_uris);
        batches
    }

    fn stale_diagnostic_clear_uris(
        &self,
        entry_uri: &Url,
        current_uris: &HashSet<Url>,
    ) -> Vec<Url> {
        self.diagnostic_publication_groups
            .get(entry_uri)
            .into_iter()
            .flat_map(|previous| previous.difference(current_uris))
            .filter(|uri| !self.diagnostic_uri_has_other_publisher(entry_uri, uri))
            .cloned()
            .collect()
    }

    fn diagnostic_uri_has_other_publisher(&self, entry_uri: &Url, diagnostic_uri: &Url) -> bool {
        self.diagnostic_publication_groups
            .iter()
            .any(|(other_entry_uri, published_uris)| {
                other_entry_uri != entry_uri && published_uris.contains(diagnostic_uri)
            })
    }
}

const KEYWORDS: &[&str] = &[
    "model", "route", "auth", "workflow", "step", "fn", "let", "const", "return", "if", "else",
    "for", "while", "in", "true", "false", "nil", "import", "export", "from", "as", "String",
    "Int", "Float", "Bool", "Money", "Date", "GET", "POST", "PUT", "DELETE", "print", "invoice",
];

const SEMANTIC_TOKEN_KEYWORD: u32 = 0;
const SEMANTIC_TOKEN_TYPE: u32 = 1;
const SEMANTIC_TOKEN_STRING: u32 = 2;
const SEMANTIC_TOKEN_NUMBER: u32 = 3;
const SEMANTIC_TOKEN_VARIABLE: u32 = 4;
const SEMANTIC_TOKEN_ERP_SYMBOL: u32 = 5;

pub fn semantic_tokens_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,
            SemanticTokenType::TYPE,
            SemanticTokenType::STRING,
            SemanticTokenType::NUMBER,
            SemanticTokenType::VARIABLE,
            SemanticTokenType::new("erpSymbol"),
        ],
        token_modifiers: Vec::new(),
    }
}

fn nx_to_lsp_zero(v: usize) -> u32 {
    v.saturating_sub(1) as u32
}

fn token_text(tok: &Token) -> &str {
    match tok {
        Token::Integer(_) => "<integer>",
        Token::Float(_) => "<float>",
        Token::StringLit(s) => s,
        Token::Bool(b) => {
            if *b {
                "true"
            } else {
                "false"
            }
        }
        Token::Money(_, _c) => "<money>",
        Token::Nil => "nil",
        Token::Ident(s) => s,
        Token::Let => "let",
        Token::Const => "const",
        Token::Fn => "fn",
        Token::Return => "return",
        Token::If => "if",
        Token::Else => "else",
        Token::While => "while",
        Token::For => "for",
        Token::In => "in",
        Token::Model => "model",
        Token::Workflow => "workflow",
        Token::Step => "step",
        Token::Route => "route",
        Token::Auth => "auth",
        Token::Invoice => "invoice",
        Token::Print => "print",
        Token::TypeString => "String",
        Token::TypeInt => "Int",
        Token::TypeFloat => "Float",
        Token::TypeBool => "Bool",
        Token::TypeMoney => "Money",
        Token::TypeDate => "Date",
        Token::Get => "GET",
        Token::Post => "POST",
        Token::Put => "PUT",
        Token::Delete => "DELETE",
        Token::Plus => "+",
        Token::Minus => "-",
        Token::Star => "*",
        Token::Slash => "/",
        Token::Percent => "%",
        Token::Eq => "==",
        Token::NotEq => "!=",
        Token::Lt => "<",
        Token::LtEq => "<=",
        Token::Gt => ">",
        Token::GtEq => ">=",
        Token::And => "&&",
        Token::Or => "||",
        Token::Not => "!",
        Token::Assign => "=",
        Token::Arrow => "->",
        Token::ColonColon => "::",
        Token::LParen => "(",
        Token::RParen => ")",
        Token::LBrace => "{",
        Token::RBrace => "}",
        Token::LBracket => "[",
        Token::RBracket => "]",
        Token::Comma => ",",
        Token::Colon => ":",
        Token::Semicolon => ";",
        Token::Dot => ".",
        Token::Question => "?",
        Token::Slash2 => "",
        Token::Eof => "",
        Token::Newline => "\n",
    }
}

pub fn range_from_line_col(line: usize, col: usize) -> Range {
    Range {
        start: Position {
            line: nx_to_lsp_zero(line),
            character: nx_to_lsp_zero(col),
        },
        end: Position {
            line: nx_to_lsp_zero(line),
            character: nx_to_lsp_zero(col) + 1,
        },
    }
}

fn range_from_source_range(range: nexuslang::module_loader::SourceRange) -> Range {
    Range {
        start: Position {
            line: nx_to_lsp_zero(range.start.line),
            character: nx_to_lsp_zero(range.start.column),
        },
        end: Position {
            line: nx_to_lsp_zero(range.end.line),
            character: nx_to_lsp_zero(range.end.column),
        },
    }
}

pub fn diagnostic_to_lsp(uri: &Url, d: &nexuslang::diagnostic::Diagnostic) -> Diagnostic {
    let range = match (d.line, d.column) {
        (Some(l), Some(c)) => range_from_line_col(l, c),
        (Some(l), None) => Range {
            start: Position {
                line: nx_to_lsp_zero(l),
                character: 0,
            },
            end: Position {
                line: nx_to_lsp_zero(l),
                character: 0,
            },
        },
        _ => Range {
            start: Position::new(0, 0),
            end: Position::new(0, 0),
        },
    };

    let sev = match d.severity {
        Some(nexuslang::diagnostic::DiagnosticSeverity::Error) => DiagnosticSeverity::ERROR,
        Some(nexuslang::diagnostic::DiagnosticSeverity::Warning) => DiagnosticSeverity::WARNING,
        Some(nexuslang::diagnostic::DiagnosticSeverity::Info) => DiagnosticSeverity::INFORMATION,
        Some(nexuslang::diagnostic::DiagnosticSeverity::Hint) => DiagnosticSeverity::HINT,
        None => DiagnosticSeverity::ERROR,
    };

    let code = d
        .code
        .clone()
        .or_else(|| Some(d.stage.default_code().to_string()))
        .unwrap_or_else(|| "NXL0000".to_string());

    let mut related: Vec<DiagnosticRelatedInformation> = Vec::new();
    for label in &d.labels {
        if let (Some(ll), Some(lc)) = (label.line, label.column) {
            related.push(DiagnosticRelatedInformation {
                location: Location {
                    uri: uri.clone(),
                    range: range_from_line_col(ll, lc),
                },
                message: label.message.clone(),
            });
        }
    }

    Diagnostic {
        range,
        severity: Some(sev),
        code: Some(NumberOrString::String(code)),
        code_description: None,
        source: Some("nexuslang".to_string()),
        message: d.message.clone(),
        related_information: if related.is_empty() {
            None
        } else {
            Some(related)
        },
        tags: None,
        data: None,
    }
}

fn multi_module_diagnostic_to_lsp(
    uri: &Url,
    diagnostic: &nexuslang::MultiModuleDiagnostic,
) -> Diagnostic {
    let mut lsp = diagnostic_to_lsp(uri, &diagnostic.diagnostic);
    if let Some(source_range) = diagnostic.source_range {
        if diagnostic.diagnostic.line.is_none() || diagnostic.diagnostic.column.is_none() {
            lsp.range = range_from_source_range(source_range);
        }
    }
    lsp
}

fn multi_module_diagnostic_uri(
    diagnostic: &nexuslang::MultiModuleDiagnostic,
    source_database: Option<&nexuslang::module_loader::SourceDatabase>,
) -> Option<Url> {
    diagnostic
        .path
        .as_deref()
        .and_then(file_url_for_path)
        .or_else(|| {
            diagnostic
                .module_id
                .and_then(|module_id| source_database?.module_path(module_id))
                .and_then(file_url_for_path)
        })
}

fn single_document_diagnostic_batch(snapshot: &DocumentSnapshot) -> DiagnosticPublishBatch {
    DiagnosticPublishBatch::new(
        snapshot.uri().clone(),
        snapshot.version(),
        snapshot.diagnostics(),
    )
}

fn snapshot_matches_disk(snapshot: &DocumentSnapshot, path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|source| source == snapshot.text())
        .unwrap_or(false)
}

fn file_url_for_path(path: &Path) -> Option<Url> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    Url::from_file_path(path).ok()
}

fn push_diagnostic_batch(
    batches: &mut Vec<DiagnosticPublishBatch>,
    uri: Url,
    version: Option<i32>,
    diagnostic: Diagnostic,
) {
    if let Some(batch) = batches.iter_mut().find(|batch| batch.uri == uri) {
        batch.diagnostics.push(diagnostic);
        return;
    }

    batches.push(DiagnosticPublishBatch::new(uri, version, vec![diagnostic]));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AbsoluteSemanticToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
}

fn semantic_tokens_for_source(source: &str) -> SemanticTokens {
    let mut absolute_tokens = nexuslang::tokens_source_spanned(source)
        .into_iter()
        .filter_map(|(token, line, column)| {
            let token_type = semantic_token_type(&token)?;
            let length = semantic_token_length(source, &token, line, column)?;
            if length == 0 {
                return None;
            }
            Some(AbsoluteSemanticToken {
                line: nx_to_lsp_zero(line),
                start: nx_to_lsp_zero(column),
                length,
                token_type,
            })
        })
        .collect::<Vec<_>>();

    absolute_tokens.sort_by_key(|token| (token.line, token.start));

    SemanticTokens {
        result_id: None,
        data: encode_semantic_tokens(&absolute_tokens),
    }
}

fn encode_semantic_tokens(tokens: &[AbsoluteSemanticToken]) -> Vec<SemanticToken> {
    let mut encoded = Vec::with_capacity(tokens.len());
    let mut previous_line = 0;
    let mut previous_start = 0;
    let mut has_previous = false;

    for token in tokens {
        let delta_line = if has_previous {
            token.line.saturating_sub(previous_line)
        } else {
            token.line
        };
        let delta_start = if has_previous && delta_line == 0 {
            token.start.saturating_sub(previous_start)
        } else {
            token.start
        };

        encoded.push(SemanticToken {
            delta_line,
            delta_start,
            length: token.length,
            token_type: token.token_type,
            token_modifiers_bitset: 0,
        });

        previous_line = token.line;
        previous_start = token.start;
        has_previous = true;
    }

    encoded
}

fn semantic_token_type(token: &Token) -> Option<u32> {
    match token {
        Token::Model
        | Token::Workflow
        | Token::Step
        | Token::Route
        | Token::Auth
        | Token::Invoice => Some(SEMANTIC_TOKEN_ERP_SYMBOL),
        Token::TypeString
        | Token::TypeInt
        | Token::TypeFloat
        | Token::TypeBool
        | Token::TypeMoney
        | Token::TypeDate => Some(SEMANTIC_TOKEN_TYPE),
        Token::StringLit(_) => Some(SEMANTIC_TOKEN_STRING),
        Token::Integer(_) | Token::Float(_) | Token::Money(_, _) => Some(SEMANTIC_TOKEN_NUMBER),
        Token::Ident(name) if is_contextual_keyword(name) => Some(SEMANTIC_TOKEN_KEYWORD),
        Token::Ident(_) => Some(SEMANTIC_TOKEN_VARIABLE),
        Token::Let
        | Token::Const
        | Token::Fn
        | Token::Return
        | Token::If
        | Token::Else
        | Token::While
        | Token::For
        | Token::In
        | Token::Bool(_)
        | Token::Nil
        | Token::Print
        | Token::Get
        | Token::Post
        | Token::Put
        | Token::Delete => Some(SEMANTIC_TOKEN_KEYWORD),
        _ => None,
    }
}

fn is_contextual_keyword(name: &str) -> bool {
    matches!(name, "import" | "export" | "from" | "as")
}

fn semantic_token_length(source: &str, token: &Token, line: usize, column: usize) -> Option<u32> {
    let length = match token {
        Token::StringLit(_) => string_literal_len_on_line(source, line, column)?,
        Token::Integer(_) | Token::Float(_) | Token::Money(_, _) => {
            number_literal_len_on_line(source, line, column)?
        }
        Token::Ident(name) => name.chars().count(),
        _ => token_text(token).chars().count(),
    };
    u32::try_from(length).ok()
}

fn string_literal_len_on_line(source: &str, line: usize, column: usize) -> Option<usize> {
    let line_text = source.lines().nth(line.checked_sub(1)?)?;
    let start = byte_index_for_column(line_text, column);
    let mut escaped = false;
    let mut length = 0;

    for ch in line_text.get(start..)?.chars() {
        length += 1;
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' && length > 1 {
            return Some(length);
        }
    }

    Some(length)
}

fn number_literal_len_on_line(source: &str, line: usize, column: usize) -> Option<usize> {
    let line_text = source.lines().nth(line.checked_sub(1)?)?;
    let start = byte_index_for_column(line_text, column);
    let mut length = 0;

    for ch in line_text.get(start..)?.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            length += 1;
        } else {
            break;
        }
    }

    Some(length)
}

fn document_symbols_for_source(source: &str) -> DocumentSymbolResponse {
    let symbols = nexuslang::parse_source_diagnostic(source)
        .map(|program| document_symbols_for_program(source, &program))
        .unwrap_or_default();
    DocumentSymbolResponse::Nested(symbols)
}

fn document_symbols_for_program(
    source: &str,
    program: &nexuslang::ast::Program,
) -> Vec<DocumentSymbol> {
    program
        .decls
        .iter()
        .filter_map(|decl| document_symbol_for_decl(source, decl))
        .collect()
}

fn document_symbol_for_decl(source: &str, decl: &nexuslang::ast::Decl) -> Option<DocumentSymbol> {
    match decl {
        nexuslang::ast::Decl::Function { name, span, .. } => Some(document_symbol(
            source,
            name,
            Some("function"),
            SymbolKind::FUNCTION,
            *span,
            Some(name),
            None,
        )),
        nexuslang::ast::Decl::Model { name, fields, span } => Some(document_symbol(
            source,
            name,
            Some("model"),
            SymbolKind::STRUCT,
            *span,
            Some(name),
            Some(
                fields
                    .iter()
                    .map(|field| {
                        document_symbol(
                            source,
                            &field.name,
                            Some("field"),
                            SymbolKind::FIELD,
                            field.span,
                            Some(&field.name),
                            None,
                        )
                    })
                    .collect(),
            ),
        )),
        nexuslang::ast::Decl::Workflow { name, steps, span } => Some(document_symbol(
            source,
            name,
            Some("workflow"),
            SymbolKind::EVENT,
            *span,
            Some(name),
            Some(
                steps
                    .iter()
                    .map(|step| {
                        document_symbol(
                            source,
                            &step.name,
                            Some("step"),
                            SymbolKind::METHOD,
                            step.span,
                            Some(&step.name),
                            None,
                        )
                    })
                    .collect(),
            ),
        )),
        nexuslang::ast::Decl::Auth { config } => Some(document_symbol(
            source,
            &config.name,
            Some("auth"),
            SymbolKind::CLASS,
            config.span,
            Some(&config.name),
            None,
        )),
        nexuslang::ast::Decl::Route {
            method,
            path,
            query_params,
            span,
            ..
        } => {
            let name = format!("{} {}", http_method_text(method), path);
            Some(document_symbol(
                source,
                &name,
                Some("route"),
                SymbolKind::METHOD,
                *span,
                Some(http_method_text(method)),
                Some(
                    query_params
                        .iter()
                        .map(|param| {
                            document_symbol(
                                source,
                                &param.name,
                                Some("query param"),
                                SymbolKind::FIELD,
                                param.span,
                                Some(&param.name),
                                None,
                            )
                        })
                        .collect(),
                ),
            ))
        }
        nexuslang::ast::Decl::Invoice {
            fields,
            items,
            span,
        } => {
            let mut children = fields
                .iter()
                .map(|field| {
                    document_symbol(
                        source,
                        &field.key,
                        Some("invoice field"),
                        SymbolKind::FIELD,
                        field.span,
                        Some(&field.key),
                        None,
                    )
                })
                .collect::<Vec<_>>();
            children.extend(items.iter().enumerate().map(|(index, item)| {
                let name = format!("item {}", index + 1);
                document_symbol(
                    source,
                    &name,
                    Some("invoice item"),
                    SymbolKind::OBJECT,
                    item.span,
                    Some("item"),
                    None,
                )
            }));
            Some(document_symbol(
                source,
                "invoice",
                Some("invoice"),
                SymbolKind::OBJECT,
                *span,
                Some("invoice"),
                Some(children),
            ))
        }
        nexuslang::ast::Decl::Import { import } => {
            let name = import.alias.as_deref().unwrap_or(&import.name);
            Some(document_symbol(
                source,
                name,
                Some("import"),
                SymbolKind::MODULE,
                import.span,
                Some(name),
                None,
            ))
        }
        nexuslang::ast::Decl::Export { decl, .. } => {
            document_symbol_for_decl(source, decl).map(|mut symbol| {
                symbol.detail = Some(match symbol.detail {
                    Some(detail) => format!("export {detail}"),
                    None => "export".to_string(),
                });
                symbol
            })
        }
        nexuslang::ast::Decl::Statement(stmt) => top_level_statement_symbol(source, stmt),
    }
}

fn top_level_statement_symbol(source: &str, stmt: &nexuslang::ast::Stmt) -> Option<DocumentSymbol> {
    match stmt {
        nexuslang::ast::Stmt::Let { name, span, .. }
        | nexuslang::ast::Stmt::Const { name, span, .. } => Some(document_symbol(
            source,
            name,
            Some("binding"),
            SymbolKind::VARIABLE,
            *span,
            Some(name),
            None,
        )),
        _ => None,
    }
}

#[allow(deprecated)]
fn document_symbol(
    source: &str,
    name: &str,
    detail: Option<&str>,
    kind: SymbolKind,
    span: nexuslang::ast::Span,
    selection_text: Option<&str>,
    children: Option<Vec<DocumentSymbol>>,
) -> DocumentSymbol {
    let selection_text = selection_text.unwrap_or(name);
    let selection_range = symbol_selection_range(source, span, selection_text);
    let range = symbol_full_range(source, span).unwrap_or(selection_range);
    let children = children.and_then(|children| {
        if children.is_empty() {
            None
        } else {
            Some(children)
        }
    });

    DocumentSymbol {
        name: name.to_string(),
        detail: detail.map(str::to_string),
        kind,
        tags: None,
        deprecated: None,
        range,
        selection_range,
        children,
    }
}

fn symbol_selection_range(source: &str, span: nexuslang::ast::Span, selection_text: &str) -> Range {
    if !span.is_known() {
        return Range::new(Position::new(0, 0), Position::new(0, 0));
    }

    let line_text = source
        .lines()
        .nth(span.line.saturating_sub(1))
        .unwrap_or("");
    let search_start = byte_index_for_column(line_text, span.column);
    if !selection_text.is_empty() {
        if let Some(relative_start) = line_text
            .get(search_start..)
            .and_then(|text| text.find(selection_text))
        {
            let character = search_start + relative_start;
            return Range::new(
                Position::new(nx_to_lsp_zero(span.line), character as u32),
                Position::new(
                    nx_to_lsp_zero(span.line),
                    (character + selection_text.chars().count()) as u32,
                ),
            );
        }
    }

    let start = nx_to_lsp_zero(span.column);
    Range::new(
        Position::new(nx_to_lsp_zero(span.line), start),
        Position::new(
            nx_to_lsp_zero(span.line),
            u32::try_from(line_text.chars().count()).unwrap_or(start),
        ),
    )
}

fn symbol_full_range(source: &str, span: nexuslang::ast::Span) -> Option<Range> {
    if !span.is_known() {
        return None;
    }

    let start = Position::new(nx_to_lsp_zero(span.line), nx_to_lsp_zero(span.column));
    let start_byte = byte_index_for_span(source, span)?;
    let end = block_end_position(source, start_byte)
        .or_else(|| line_end_position(source, span.line))
        .unwrap_or(start);
    Some(Range::new(start, end))
}

fn block_end_position(source: &str, start_byte: usize) -> Option<Position> {
    let mut depth = 0usize;
    let mut saw_block = false;
    let mut in_string = false;
    let mut escaped = false;

    for (relative_byte, ch) in source.get(start_byte..)?.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '\n' if !saw_block => return None,
            '{' => {
                saw_block = true;
                depth += 1;
            }
            '}' if saw_block => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return position_for_byte_index(
                        source,
                        start_byte + relative_byte + ch.len_utf8(),
                    );
                }
            }
            _ => {}
        }
    }

    None
}

fn line_end_position(source: &str, line: usize) -> Option<Position> {
    let line_text = source.lines().nth(line.checked_sub(1)?)?;
    Some(Position::new(
        nx_to_lsp_zero(line),
        u32::try_from(line_text.chars().count()).ok()?,
    ))
}

fn byte_index_for_span(source: &str, span: nexuslang::ast::Span) -> Option<usize> {
    let mut current_line = 1usize;
    let mut line_start = 0usize;

    for (byte_index, ch) in source.char_indices() {
        if current_line == span.line {
            break;
        }
        if ch == '\n' {
            current_line += 1;
            line_start = byte_index + ch.len_utf8();
        }
    }

    if current_line != span.line {
        return None;
    }

    let line_end = source
        .get(line_start..)?
        .find('\n')
        .map(|relative| line_start + relative)
        .unwrap_or(source.len());
    let line_text = source.get(line_start..line_end)?;
    Some(line_start + byte_index_for_column(line_text, span.column))
}

fn position_for_byte_index(source: &str, byte_index: usize) -> Option<Position> {
    let byte_index = byte_index.min(source.len());
    if !source.is_char_boundary(byte_index) {
        return None;
    }

    let mut line = 0u32;
    let mut character = 0u32;
    for ch in source.get(..byte_index)?.chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }
    Some(Position::new(line, character))
}

fn http_method_text(method: &nexuslang::ast::HttpMethod) -> &'static str {
    match method {
        nexuslang::ast::HttpMethod::Get => "GET",
        nexuslang::ast::HttpMethod::Post => "POST",
        nexuslang::ast::HttpMethod::Put => "PUT",
        nexuslang::ast::HttpMethod::Delete => "DELETE",
    }
}

fn cross_file_definition_for_snapshot(
    snapshot: &DocumentSnapshot,
    position: Position,
    program: &nexuslang::ast::Program,
    module_graph: &nexuslang::module_loader::ModuleGraph,
    decl_module_map: &[nexuslang::hir::HirModuleId],
    source_database: &nexuslang::module_loader::SourceDatabase,
) -> Option<Location> {
    let entry_path = snapshot.file_path()?;
    let source_module = source_database.module_by_path(&entry_path)?;
    let identifier = identifier_at_position(snapshot.text(), position)?;
    let (line, column) = lsp_to_nx_position(position);

    let import_edge = import_edge_at_position(
        source_database,
        source_module.module_id,
        line,
        column,
        &identifier,
    )
    .or_else(|| {
        import_edge_for_local_definition(
            snapshot.text(),
            source_database,
            source_module.module_id,
            &identifier,
        )
    })?;

    let target_module = import_edge.target_module?;
    if !module_graph.entries.iter().any(|entry| {
        entry.module_id == target_module
            && entry
                .export_names
                .iter()
                .any(|name| name == &import_edge.imported_name)
    }) {
        return None;
    }

    exported_definition_location(
        program,
        decl_module_map,
        source_database,
        target_module,
        &import_edge.imported_name,
    )
}

fn import_edge_at_position<'a>(
    source_database: &'a nexuslang::module_loader::SourceDatabase,
    module_id: nexuslang::hir::HirModuleId,
    line: usize,
    column: usize,
    identifier: &str,
) -> Option<&'a nexuslang::module_loader::SourceImportEdge> {
    source_database.import_edges_from(module_id).find(|edge| {
        span_contains_name(edge.name_span, &edge.imported_name, line, column)
            || edge.alias.as_deref() == Some(identifier)
                && edge
                    .alias_span
                    .map(|span| span_contains_name(span, identifier, line, column))
                    .unwrap_or(false)
    })
}

fn import_edge_for_local_definition<'a>(
    source: &str,
    source_database: &'a nexuslang::module_loader::SourceDatabase,
    module_id: nexuslang::hir::HirModuleId,
    identifier: &str,
) -> Option<&'a nexuslang::module_loader::SourceImportEdge> {
    let (definition_line, definition_column) = find_definition_location(source, identifier)?;
    source_database.import_edges_from(module_id).find(|edge| {
        edge_local_name(edge) == identifier && {
            let span = edge_local_span(edge);
            span.line == definition_line && span.column == definition_column
        }
    })
}

fn exported_definition_location(
    program: &nexuslang::ast::Program,
    decl_module_map: &[nexuslang::hir::HirModuleId],
    source_database: &nexuslang::module_loader::SourceDatabase,
    target_module: nexuslang::hir::HirModuleId,
    imported_name: &str,
) -> Option<Location> {
    let target_source = source_database.module(target_module)?;
    let uri = file_url_for_path(&target_source.path)?;

    program.decls.iter().enumerate().find_map(|(index, decl)| {
        if decl_module_map.get(index).copied() != Some(target_module) {
            return None;
        }
        if exported_decl_name(decl) != Some(imported_name) {
            return None;
        }
        let (line, column) = declaration_name_location(&target_source.source, decl)?;
        Some(location_from_line_col(uri.clone(), line, column))
    })
}

fn exported_decl_name(decl: &nexuslang::ast::Decl) -> Option<&str> {
    match decl {
        nexuslang::ast::Decl::Export { decl, .. } => declaration_name(decl.as_ref()),
        _ => None,
    }
}

fn declaration_name(decl: &nexuslang::ast::Decl) -> Option<&str> {
    match decl {
        nexuslang::ast::Decl::Function { name, .. }
        | nexuslang::ast::Decl::Model { name, .. }
        | nexuslang::ast::Decl::Workflow { name, .. } => Some(name.as_str()),
        nexuslang::ast::Decl::Auth { config } => Some(config.name.as_str()),
        nexuslang::ast::Decl::Export { decl, .. } => declaration_name(decl.as_ref()),
        _ => None,
    }
}

fn declaration_span(decl: &nexuslang::ast::Decl) -> Option<nexuslang::ast::Span> {
    match decl {
        nexuslang::ast::Decl::Function { span, .. }
        | nexuslang::ast::Decl::Model { span, .. }
        | nexuslang::ast::Decl::Workflow { span, .. } => Some(*span),
        nexuslang::ast::Decl::Auth { config } => Some(config.span),
        nexuslang::ast::Decl::Export { decl, .. } => declaration_span(decl.as_ref()),
        _ => None,
    }
}

fn declaration_name_location(source: &str, decl: &nexuslang::ast::Decl) -> Option<(usize, usize)> {
    let name = declaration_name(decl)?;
    let span = declaration_span(decl)?;
    let line = source.lines().nth(span.line.checked_sub(1)?)?;
    let search_start = byte_index_for_column(line, span.column);
    let relative_name_start = line.get(search_start..)?.find(name)?;
    Some((span.line, search_start + relative_name_start + 1))
}

fn edge_local_name(edge: &nexuslang::module_loader::SourceImportEdge) -> &str {
    edge.alias.as_deref().unwrap_or(&edge.imported_name)
}

fn edge_local_span(edge: &nexuslang::module_loader::SourceImportEdge) -> nexuslang::ast::Span {
    edge.alias_span.unwrap_or(edge.name_span)
}

fn identifier_at_position(source: &str, position: Position) -> Option<String> {
    let (line, column) = lsp_to_nx_position(position);
    nexuslang::tokens_source_spanned(source)
        .into_iter()
        .find_map(|(token, token_line, token_column)| {
            let Token::Ident(name) = token else {
                return None;
            };
            if span_contains_name(
                nexuslang::ast::Span::new(token_line, token_column),
                &name,
                line,
                column,
            ) {
                Some(name)
            } else {
                None
            }
        })
}

fn span_contains_name(span: nexuslang::ast::Span, name: &str, line: usize, column: usize) -> bool {
    line == span.line
        && column >= span.column
        && column < span.column.saturating_add(name.chars().count())
}

fn lsp_to_nx_position(position: Position) -> (usize, usize) {
    (position.line as usize + 1, position.character as usize + 1)
}

fn byte_index_for_column(line: &str, column: usize) -> usize {
    if column <= 1 {
        return 0;
    }
    line.char_indices()
        .nth(column - 1)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

fn location_from_line_col(uri: Url, line: usize, column: usize) -> Location {
    Location {
        uri,
        range: range_from_line_col(line, column),
    }
}

fn hover_for_source(source: &str, position: Position) -> Option<Hover> {
    let tokens = nexuslang::tokens_source_spanned(source);
    for (tok, line, col) in &tokens {
        let tok_line = nx_to_lsp_zero(*line);
        let tok_col = nx_to_lsp_zero(*col);
        let text = token_text(tok);
        let len = text.len() as u32;

        if tok_line == position.line
            && tok_col <= position.character
            && position.character < tok_col + len
        {
            let hover_text = format_hover_text(tok, text);
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: hover_text,
                }),
                range: Some(Range {
                    start: Position::new(tok_line, tok_col),
                    end: Position::new(tok_line, tok_col + len),
                }),
            });
        }
    }
    None
}

fn completion_items_for_source(source: &str) -> Vec<CompletionItem> {
    let mut seen = HashSet::new();
    let mut items: Vec<CompletionItem> = Vec::new();

    for kw in KEYWORDS {
        seen.insert(kw.to_string());
        items.push(CompletionItem {
            label: kw.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("keyword".to_string()),
            insert_text: Some(kw.to_string()),
            ..Default::default()
        });
    }

    let tokens = nexuslang::tokens_source_spanned(source);
    for (tok, _line, _col) in &tokens {
        if let Token::Ident(name) = tok {
            if !seen.contains(name) {
                seen.insert(name.clone());
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some("identifier".to_string()),
                    ..Default::default()
                });
            }
        }
    }

    items.sort_by(|a, b| {
        let a_kw = matches!(a.kind, Some(CompletionItemKind::KEYWORD));
        let b_kw = matches!(b.kind, Some(CompletionItemKind::KEYWORD));
        b_kw.cmp(&a_kw).then(a.label.cmp(&b.label))
    });

    items
}

fn definition_for_source(uri: &Url, source: &str, position: Position) -> Option<Location> {
    let tokens = nexuslang::tokens_source_spanned(source);
    for (tok, line, col) in &tokens {
        let tok_line = nx_to_lsp_zero(*line);
        let tok_col = nx_to_lsp_zero(*col);
        let text = token_text(tok);
        let len = text.len() as u32;

        if tok_line == position.line
            && tok_col <= position.character
            && position.character < tok_col + len
            && !text.is_empty()
        {
            if let Some(loc) = find_definition_location(source, text) {
                return Some(Location {
                    uri: uri.clone(),
                    range: Range {
                        start: Position {
                            line: nx_to_lsp_zero(loc.0),
                            character: nx_to_lsp_zero(loc.1),
                        },
                        end: Position {
                            line: nx_to_lsp_zero(loc.0),
                            character: nx_to_lsp_zero(loc.1) + 1,
                        },
                    },
                });
            }
            break;
        }
    }
    None
}

fn format_hover_text(tok: &Token, text: &str) -> String {
    match tok {
        Token::Ident(_) => format!("**{}**  \nidentifier", text),
        Token::StringLit(_) => format!("`\"{}\"`  \nstring literal", text),
        Token::Integer(n) => format!("**{}**  \ninteger literal (`{}`)", text, n),
        Token::Float(n) => format!("**{}**  \nfloat literal (`{}`)", text, n),
        Token::Bool(_b) => format!("**{}**  \nboolean literal", text),
        Token::Money(_, c) => format!("**{}**  \nmoney literal (currency: {})", text, c),
        Token::Nil => "**nil**  \nnull value".to_string(),
        _ => {
            let kind = match tok {
                Token::Let | Token::Const | Token::Fn | Token::Return => "keyword",
                Token::Model => "keyword (declaration)",
                Token::Route => "keyword (route declaration)",
                Token::Auth => "keyword (auth declaration)",
                Token::Workflow => "keyword (workflow declaration)",
                Token::Step => "keyword (workflow step)",
                Token::If | Token::Else => "keyword (conditional)",
                Token::While | Token::For | Token::In => "keyword (loop)",
                Token::TypeString
                | Token::TypeInt
                | Token::TypeFloat
                | Token::TypeBool
                | Token::TypeMoney
                | Token::TypeDate => "type",
                Token::Get | Token::Post | Token::Put | Token::Delete => "HTTP method",
                _ => "operator / delimiter",
            };
            format!("`{}`  \n{}", text, kind)
        }
    }
}

pub fn find_definition_location(source: &str, name: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = source.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let decl_line = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let is_decl_keyword = decl_line.starts_with("model ")
            || decl_line.starts_with("route ")
            || decl_line.starts_with("auth ")
            || decl_line.starts_with("workflow ")
            || decl_line.starts_with("fn ");

        if is_decl_keyword {
            let start = decl_line.find(' ')? + 1;
            let end = decl_line[start..]
                .find(|c: char| c.is_whitespace() || c == '(' || c == '{')
                .map(|p| start + p)
                .unwrap_or(decl_line.len());
            if decl_line[start..end] == *name {
                let col = line.find(name)?;
                return Some((i + 1, col + 1));
            }
        }

        if let Some(content) = trimmed.strip_prefix("let ") {
            if let Some(eq_pos) = content.find('=') {
                let binding = content[..eq_pos].trim();
                if binding == name {
                    let col = line.find(name)?;
                    return Some((i + 1, col + 1));
                }
            }
        }

        if let Some(content) = trimmed.strip_prefix("import ") {
            if let Some(from_pos) = content.find(" from") {
                let imported = content[..from_pos].trim();
                let imported_name = if let Some(as_pos) = imported.find(" as ") {
                    &imported[as_pos + 4..]
                } else {
                    imported
                };
                if imported_name == name {
                    let col = line.find(name)?;
                    return Some((i + 1, col + 1));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexuslang::diagnostic::{
        Diagnostic as CoreDiagnostic, DiagnosticLabel,
        DiagnosticSeverity as CoreDiagnosticSeverity, DiagnosticStage,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn range_from_line_col_converts_to_lsp_zero_based_positions() {
        let range = range_from_line_col(3, 7);

        assert_eq!(range.start.line, 2);
        assert_eq!(range.start.character, 6);
        assert_eq!(range.end.line, 2);
        assert_eq!(range.end.character, 7);
    }

    #[test]
    fn diagnostic_to_lsp_preserves_core_metadata() {
        let uri = Url::parse("file:///workspace/main.nx").unwrap();
        let diagnostic = CoreDiagnostic::new(DiagnosticStage::Checker, "Tipo invalido")
            .with_code("NXL3001")
            .with_severity(CoreDiagnosticSeverity::Warning)
            .with_location(4, 9)
            .with_label(DiagnosticLabel::at_location("expressao", 4, 9));

        let lsp = diagnostic_to_lsp(&uri, &diagnostic);

        assert_eq!(lsp.range.start, Position::new(3, 8));
        assert_eq!(lsp.range.end, Position::new(3, 9));
        assert_eq!(lsp.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(lsp.source.as_deref(), Some("nexuslang"));
        assert_eq!(
            lsp.code,
            Some(NumberOrString::String("NXL3001".to_string()))
        );
        assert_eq!(lsp.message, "Tipo invalido");

        let related = lsp.related_information.expect("related label");
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].location.uri, uri);
        assert_eq!(related[0].location.range.start, Position::new(3, 8));
    }

    #[test]
    fn document_snapshot_produces_diagnostics_without_transport() {
        let uri = Url::parse("file:///workspace/broken.nx").unwrap();
        let snapshot = DocumentSnapshot::new(uri, Some(7), "let total =".to_string());

        let diagnostics = snapshot.diagnostics();

        assert_eq!(snapshot.version(), Some(7));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].source.as_deref(), Some("nexuslang"));
    }

    #[test]
    fn document_snapshot_exposes_hover_completion_and_definition() {
        let uri = Url::parse("file:///workspace/main.nx").unwrap();
        let source = r#"
fn calcular() {
  return 1
}

let total = calcular()
"#;
        let snapshot = DocumentSnapshot::new(uri.clone(), Some(1), source.to_string());

        let hover = snapshot
            .hover(Position::new(1, 3))
            .expect("hover over function name");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markdown hover");
        };
        assert!(markup.value.contains("identifier"));

        let completion = snapshot.completion();
        let CompletionResponse::Array(items) = completion else {
            panic!("expected completion array");
        };
        assert!(items.iter().any(|item| item.label == "calcular"));
        assert!(items.iter().any(|item| item.label == "let"));

        let definition = snapshot
            .goto_definition(Position::new(5, 14))
            .expect("definition for calcular call");
        assert_eq!(definition.uri, uri);
        assert_eq!(definition.range.start, Position::new(1, 3));
    }

    #[test]
    fn semantic_tokens_legend_exposes_mvp_categories() {
        let legend = semantic_tokens_legend();

        assert_eq!(
            legend.token_types,
            vec![
                SemanticTokenType::KEYWORD,
                SemanticTokenType::TYPE,
                SemanticTokenType::STRING,
                SemanticTokenType::NUMBER,
                SemanticTokenType::VARIABLE,
                SemanticTokenType::new("erpSymbol"),
            ]
        );
        assert!(legend.token_modifiers.is_empty());
    }

    #[test]
    fn document_snapshot_produces_semantic_tokens_for_mvp_categories() {
        let uri = Url::parse("file:///workspace/main.nx").unwrap();
        let source = r#"model Cliente { nome: string }
let total = 300 kz
let msg = "ok"
fn calcular() -> int { return total }
import Cliente as Pessoa from "./cliente.nx"
"#;
        let snapshot = DocumentSnapshot::new(uri, Some(1), source.to_string());
        let decoded = decode_semantic_tokens(&snapshot.semantic_tokens().data);

        assert_semantic_token(
            &decoded,
            source,
            0,
            "model",
            "model".len(),
            SEMANTIC_TOKEN_ERP_SYMBOL,
        );
        assert_semantic_token(
            &decoded,
            source,
            0,
            "string",
            "string".len(),
            SEMANTIC_TOKEN_TYPE,
        );
        assert_semantic_token(
            &decoded,
            source,
            1,
            "300",
            "300".len(),
            SEMANTIC_TOKEN_NUMBER,
        );
        assert_semantic_token(&decoded, source, 2, "\"ok\"", 4, SEMANTIC_TOKEN_STRING);
        assert_semantic_token(
            &decoded,
            source,
            1,
            "total",
            "total".len(),
            SEMANTIC_TOKEN_VARIABLE,
        );
        assert_semantic_token(
            &decoded,
            source,
            4,
            "import",
            "import".len(),
            SEMANTIC_TOKEN_KEYWORD,
        );
        assert_semantic_token(
            &decoded,
            source,
            4,
            "Pessoa",
            "Pessoa".len(),
            SEMANTIC_TOKEN_VARIABLE,
        );
    }

    #[test]
    fn lsp_core_returns_semantic_tokens_for_open_document() {
        let uri = Url::parse("file:///workspace/main.nx").unwrap();
        let mut core = LspCore::new();
        core.open_document(
            uri.clone(),
            Some(1),
            "route GET /clientes { return nil }".to_string(),
        );

        let tokens = core.semantic_tokens(&uri).expect("semantic tokens");
        let decoded = decode_semantic_tokens(&tokens.data);

        assert!(decoded
            .iter()
            .any(|token| token.token_type == SEMANTIC_TOKEN_ERP_SYMBOL));
        assert!(decoded
            .iter()
            .any(|token| token.token_type == SEMANTIC_TOKEN_KEYWORD));
    }

    #[test]
    fn document_snapshot_produces_document_symbols_for_erp_declarations() {
        let uri = Url::parse("file:///workspace/main.nx").unwrap();
        let source = r#"import Cliente as Pessoa from "./cliente.nx"

model Cliente {
  nome: string
  saldo: money
}

workflow Onboarding {
  step validar {
    return nil
  }
}

route GET /clientes {
  return nil
}

route GET /clientes ?(status: string) {
  return nil
}

auth Sessao {
  model: Cliente
  identity: nome
}

invoice {
  customer: "Ana"
  item "Servico" qty 1 price 100
}

fn calcular() {
  return 1
}
"#;
        let snapshot = DocumentSnapshot::new(uri, Some(1), source.to_string());
        let symbols = nested_symbols(snapshot.document_symbols());

        let import = find_symbol(&symbols, "Pessoa");
        assert_eq!(import.kind, SymbolKind::MODULE);
        assert_eq!(import.detail.as_deref(), Some("import"));

        let cliente = find_symbol(&symbols, "Cliente");
        assert_eq!(cliente.kind, SymbolKind::STRUCT);
        let cliente_children = cliente.children.as_ref().expect("model fields");
        let nome = find_symbol(cliente_children, "nome");
        assert_eq!(nome.kind, SymbolKind::FIELD);
        assert!(range_contains(&cliente.range, nome.selection_range.start));
        assert_eq!(
            find_symbol(cliente_children, "saldo").kind,
            SymbolKind::FIELD
        );

        let workflow = find_symbol(&symbols, "Onboarding");
        assert_eq!(workflow.kind, SymbolKind::EVENT);
        let workflow_children = workflow.children.as_ref().expect("workflow steps");
        let validar = find_symbol(workflow_children, "validar");
        assert_eq!(validar.kind, SymbolKind::METHOD);
        assert!(range_contains(
            &workflow.range,
            validar.selection_range.start
        ));

        let route = find_symbol(&symbols, "GET /clientes");
        assert_eq!(route.kind, SymbolKind::METHOD);
        assert_eq!(route.detail.as_deref(), Some("route"));

        let route_with_query = symbols
            .iter()
            .find(|symbol| {
                symbol.name == "GET /clientes"
                    && symbol
                        .children
                        .as_ref()
                        .is_some_and(|children| children.iter().any(|child| child.name == "status"))
            })
            .expect("route with query params");
        let query_children = route_with_query.children.as_ref().expect("query param");
        assert_eq!(
            find_symbol(query_children, "status").kind,
            SymbolKind::FIELD
        );
        assert!(range_contains(
            &route_with_query.range,
            find_symbol(query_children, "status").selection_range.start
        ));

        let auth = find_symbol(&symbols, "Sessao");
        assert_eq!(auth.kind, SymbolKind::CLASS);
        assert_eq!(auth.detail.as_deref(), Some("auth"));

        let invoice = find_symbol(&symbols, "invoice");
        assert_eq!(invoice.kind, SymbolKind::OBJECT);
        let invoice_children = invoice.children.as_ref().expect("invoice fields");
        assert_eq!(
            find_symbol(invoice_children, "customer").kind,
            SymbolKind::FIELD
        );
        assert_eq!(
            find_symbol(invoice_children, "item 1").kind,
            SymbolKind::OBJECT
        );

        let function = find_symbol(&symbols, "calcular");
        assert_eq!(function.kind, SymbolKind::FUNCTION);
    }

    #[test]
    fn lsp_core_returns_empty_document_symbols_for_invalid_snapshot() {
        let uri = Url::parse("file:///workspace/main.nx").unwrap();
        let mut core = LspCore::new();
        core.open_document(uri.clone(), Some(1), "model Cliente {".to_string());

        let symbols = core.document_symbols(&uri).expect("document symbols");

        assert!(nested_symbols(symbols).is_empty());
    }

    #[test]
    fn lsp_core_tracks_document_snapshots() {
        let uri = Url::parse("file:///workspace/main.nx").unwrap();
        let mut core = LspCore::new();

        core.open_document(uri.clone(), Some(1), "let total = 1".to_string());
        assert_eq!(core.document(&uri).unwrap().version(), Some(1));

        core.change_document(uri.clone(), Some(2), "let total = 2".to_string());
        assert_eq!(core.document(&uri).unwrap().version(), Some(2));
        assert!(core.completion(&uri).is_some());

        let closed = core.close_document(&uri).expect("closed snapshot");
        assert_eq!(closed.version(), Some(2));
        assert!(core.document(&uri).is_none());
    }

    #[test]
    fn lsp_core_publishes_imported_module_diagnostics_when_snapshot_matches_disk() {
        let dir = temp_project_dir("lsp_core_multi_file_diagnostics");
        let lib_path = dir.join("lib.nx");
        let main_path = dir.join("main.nx");
        fs::write(
            &lib_path,
            r#"
export fn broken() -> int {
    return "erro"
}
"#,
        )
        .unwrap();
        fs::write(
            &main_path,
            r#"
import broken from "./lib.nx"
"#,
        )
        .unwrap();

        let main_uri = Url::from_file_path(main_path.canonicalize().unwrap()).unwrap();
        let lib_uri = Url::from_file_path(lib_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(
            main_uri.clone(),
            Some(1),
            fs::read_to_string(&main_path).unwrap(),
        );

        let batches = core
            .diagnostic_publish_batches_for(&main_uri)
            .expect("diagnostic batches");
        let lib_batch = batches
            .iter()
            .find(|batch| batch.uri == lib_uri)
            .expect("imported module batch");

        assert_eq!(lib_batch.diagnostics.len(), 1);
        assert!(lib_batch.diagnostics[0]
            .message
            .contains("Tipo de retorno inválido"));
        assert_eq!(lib_batch.diagnostics[0].range.start, Position::new(2, 4));
        assert!(batches.iter().any(|batch| batch.uri == main_uri));
    }

    #[test]
    fn lsp_core_clears_all_loaded_module_diagnostics_when_project_passes() {
        let dir = temp_project_dir("lsp_core_multi_file_clean");
        let lib_path = dir.join("lib.nx");
        let main_path = dir.join("main.nx");
        fs::write(
            &lib_path,
            r#"
export fn ok() -> int {
    return 1
}
"#,
        )
        .unwrap();
        fs::write(
            &main_path,
            r#"
import ok from "./lib.nx"
"#,
        )
        .unwrap();

        let main_uri = Url::from_file_path(main_path.canonicalize().unwrap()).unwrap();
        let lib_uri = Url::from_file_path(lib_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(
            main_uri.clone(),
            Some(1),
            fs::read_to_string(&main_path).unwrap(),
        );

        let batches = core
            .diagnostic_publish_batches_for(&main_uri)
            .expect("diagnostic batches");

        assert!(batches
            .iter()
            .any(|batch| batch.uri == main_uri && batch.diagnostics.is_empty()));
        assert!(batches
            .iter()
            .any(|batch| batch.uri == lib_uri && batch.diagnostics.is_empty()));
    }

    #[test]
    fn lsp_core_falls_back_to_single_document_diagnostics_for_dirty_snapshot() {
        let dir = temp_project_dir("lsp_core_dirty_snapshot");
        let lib_path = dir.join("lib.nx");
        let main_path = dir.join("main.nx");
        fs::write(
            &lib_path,
            r#"
export fn broken() -> int {
    return "erro"
}
"#,
        )
        .unwrap();
        fs::write(
            &main_path,
            r#"
import broken from "./lib.nx"
"#,
        )
        .unwrap();

        let main_uri = Url::from_file_path(main_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(main_uri.clone(), Some(2), "let total =".to_string());

        let batches = core
            .diagnostic_publish_batches_for(&main_uri)
            .expect("diagnostic batches");

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].uri, main_uri);
        assert_eq!(batches[0].version, Some(2));
        assert_eq!(batches[0].diagnostics.len(), 1);
        assert!(!batches[0].diagnostics[0]
            .message
            .contains("Tipo de retorno inválido"));
    }

    #[test]
    fn lsp_core_clears_stale_imported_module_after_dirty_snapshot_fallback() {
        let dir = temp_project_dir("lsp_core_dirty_snapshot_stale_clear");
        let lib_path = dir.join("lib.nx");
        let main_path = dir.join("main.nx");
        fs::write(
            &lib_path,
            r#"
export fn broken() -> int {
    return "erro"
}
"#,
        )
        .unwrap();
        fs::write(
            &main_path,
            r#"
import broken from "./lib.nx"
"#,
        )
        .unwrap();

        let main_uri = Url::from_file_path(main_path.canonicalize().unwrap()).unwrap();
        let lib_uri = Url::from_file_path(lib_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(
            main_uri.clone(),
            Some(1),
            fs::read_to_string(&main_path).unwrap(),
        );

        let initial_batches = core
            .diagnostic_publish_batches_for(&main_uri)
            .expect("initial diagnostic batches");
        assert_eq!(batch_for(&initial_batches, &lib_uri).diagnostics.len(), 1);

        core.change_document(main_uri.clone(), Some(2), "let total =".to_string());

        let dirty_batches = core
            .diagnostic_publish_batches_for(&main_uri)
            .expect("dirty diagnostic batches");
        let main_batch = batch_for(&dirty_batches, &main_uri);
        let lib_batch = batch_for(&dirty_batches, &lib_uri);

        assert_eq!(dirty_batches.len(), 2);
        assert_eq!(main_batch.version, Some(2));
        assert_eq!(main_batch.diagnostics.len(), 1);
        assert_eq!(lib_batch.version, None);
        assert!(lib_batch.diagnostics.is_empty());
    }

    #[test]
    fn lsp_core_clears_stale_imported_module_when_import_graph_changes() {
        let dir = temp_project_dir("lsp_core_graph_stale_clear");
        let lib_path = dir.join("lib.nx");
        let main_path = dir.join("main.nx");
        fs::write(
            &lib_path,
            r#"
export fn broken() -> int {
    return "erro"
}
"#,
        )
        .unwrap();
        fs::write(
            &main_path,
            r#"
import broken from "./lib.nx"
"#,
        )
        .unwrap();

        let main_uri = Url::from_file_path(main_path.canonicalize().unwrap()).unwrap();
        let lib_uri = Url::from_file_path(lib_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(
            main_uri.clone(),
            Some(1),
            fs::read_to_string(&main_path).unwrap(),
        );

        let initial_batches = core
            .diagnostic_publish_batches_for(&main_uri)
            .expect("initial diagnostic batches");
        assert_eq!(batch_for(&initial_batches, &lib_uri).diagnostics.len(), 1);

        let clean_main = "let total = 1\n".to_string();
        fs::write(&main_path, &clean_main).unwrap();
        core.change_document(main_uri.clone(), Some(2), clean_main);

        let clean_batches = core
            .diagnostic_publish_batches_for(&main_uri)
            .expect("clean diagnostic batches");
        let main_batch = batch_for(&clean_batches, &main_uri);
        let lib_batch = batch_for(&clean_batches, &lib_uri);

        assert_eq!(clean_batches.len(), 2);
        assert_eq!(main_batch.version, Some(2));
        assert!(main_batch.diagnostics.is_empty());
        assert_eq!(lib_batch.version, None);
        assert!(lib_batch.diagnostics.is_empty());
    }

    #[test]
    fn lsp_core_close_document_clears_previous_publication_group() {
        let dir = temp_project_dir("lsp_core_close_stale_clear");
        let lib_path = dir.join("lib.nx");
        let main_path = dir.join("main.nx");
        fs::write(
            &lib_path,
            r#"
export fn broken() -> int {
    return "erro"
}
"#,
        )
        .unwrap();
        fs::write(
            &main_path,
            r#"
import broken from "./lib.nx"
"#,
        )
        .unwrap();

        let main_uri = Url::from_file_path(main_path.canonicalize().unwrap()).unwrap();
        let lib_uri = Url::from_file_path(lib_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(
            main_uri.clone(),
            Some(1),
            fs::read_to_string(&main_path).unwrap(),
        );

        let initial_batches = core
            .diagnostic_publish_batches_for(&main_uri)
            .expect("initial diagnostic batches");
        assert_eq!(batch_for(&initial_batches, &lib_uri).diagnostics.len(), 1);

        let close_batches = core.close_document_publish_batches(&main_uri);
        let main_batch = batch_for(&close_batches, &main_uri);
        let lib_batch = batch_for(&close_batches, &lib_uri);

        assert!(main_batch.diagnostics.is_empty());
        assert_eq!(main_batch.version, None);
        assert!(lib_batch.diagnostics.is_empty());
        assert_eq!(lib_batch.version, None);
        assert!(core.document(&main_uri).is_none());
        assert!(core.diagnostic_publish_batches_for(&main_uri).is_none());
    }

    #[test]
    fn lsp_core_keeps_stale_module_when_another_entry_still_publishes_it() {
        let dir = temp_project_dir("lsp_core_shared_module_stale_clear");
        let lib_path = dir.join("lib.nx");
        let main_a_path = dir.join("main_a.nx");
        let main_b_path = dir.join("main_b.nx");
        fs::write(
            &lib_path,
            r#"
export fn broken() -> int {
    return "erro"
}
"#,
        )
        .unwrap();
        let import_source = r#"
import broken from "./lib.nx"
"#;
        fs::write(&main_a_path, import_source).unwrap();
        fs::write(&main_b_path, import_source).unwrap();

        let main_a_uri = Url::from_file_path(main_a_path.canonicalize().unwrap()).unwrap();
        let main_b_uri = Url::from_file_path(main_b_path.canonicalize().unwrap()).unwrap();
        let lib_uri = Url::from_file_path(lib_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(
            main_a_uri.clone(),
            Some(1),
            fs::read_to_string(&main_a_path).unwrap(),
        );
        core.open_document(
            main_b_uri.clone(),
            Some(1),
            fs::read_to_string(&main_b_path).unwrap(),
        );

        let main_a_initial_batches = core
            .diagnostic_publish_batches_for(&main_a_uri)
            .expect("main_a diagnostic batches");
        assert_eq!(
            batch_for(&main_a_initial_batches, &lib_uri)
                .diagnostics
                .len(),
            1
        );
        let main_b_initial_batches = core
            .diagnostic_publish_batches_for(&main_b_uri)
            .expect("main_b diagnostic batches");
        assert_eq!(
            batch_for(&main_b_initial_batches, &lib_uri)
                .diagnostics
                .len(),
            1
        );

        let clean_main_a = "let total = 1\n".to_string();
        fs::write(&main_a_path, &clean_main_a).unwrap();
        core.change_document(main_a_uri.clone(), Some(2), clean_main_a);

        let main_a_clean_batches = core
            .diagnostic_publish_batches_for(&main_a_uri)
            .expect("main_a clean diagnostic batches");

        assert!(batch_for(&main_a_clean_batches, &main_a_uri)
            .diagnostics
            .is_empty());
        assert!(!main_a_clean_batches
            .iter()
            .any(|batch| batch.uri == lib_uri && batch.diagnostics.is_empty()));
    }

    #[test]
    fn lsp_core_diagnostic_snapshot_detects_changed_imported_document() {
        let dir = temp_project_dir("lsp_core_snapshot_changed_import");
        let lib_path = dir.join("lib.nx");
        let main_path = dir.join("main.nx");
        let lib_source = r#"
export fn value() -> int {
    return 1
}
"#;
        let main_source = r#"
import value from "./lib.nx"
print(value())
"#;
        fs::write(&lib_path, lib_source).unwrap();
        fs::write(&main_path, main_source).unwrap();

        let main_uri = Url::from_file_path(main_path.canonicalize().unwrap()).unwrap();
        let lib_uri = Url::from_file_path(lib_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(main_uri.clone(), Some(1), main_source.to_string());
        core.open_document(lib_uri.clone(), Some(1), lib_source.to_string());

        let mut diagnostic_core = core.clone();
        let mut group = std::collections::HashSet::new();
        group.insert(main_uri.clone());
        group.insert(lib_uri.clone());
        diagnostic_core
            .diagnostic_publication_groups
            .insert(main_uri.clone(), group);

        assert!(core.document_snapshot_matches(&main_uri, &diagnostic_core));

        core.change_document(
            lib_uri,
            Some(2),
            "export fn value() -> int { return 2 }\n".to_string(),
        );

        assert!(!core.document_snapshot_matches(&main_uri, &diagnostic_core));
    }

    #[test]
    fn lsp_core_goto_definition_resolves_import_alias_usage_to_exported_declaration() {
        let dir = temp_project_dir("lsp_core_cross_file_definition_alias");
        let lib_path = dir.join("cliente.nx");
        let main_path = dir.join("main.nx");
        let lib_source = r#"export model Cliente {
  nome: string
}
"#;
        let main_source = r#"import Cliente as Pessoa from "./cliente.nx"
let cliente = Pessoa { nome: "Ana" }
"#;
        fs::write(&lib_path, lib_source).unwrap();
        fs::write(&main_path, main_source).unwrap();

        let main_uri = Url::from_file_path(main_path.canonicalize().unwrap()).unwrap();
        let lib_uri = Url::from_file_path(lib_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(main_uri, Some(1), main_source.to_string());

        let definition = core
            .goto_definition(
                &Url::from_file_path(main_path.canonicalize().unwrap()).unwrap(),
                position_of(main_source, 1, "Pessoa"),
            )
            .expect("cross-file definition");

        assert_eq!(definition.uri, lib_uri);
        assert_eq!(
            definition.range.start,
            position_of(lib_source, 0, "Cliente")
        );
    }

    #[test]
    fn lsp_core_goto_definition_resolves_import_name_to_exported_declaration() {
        let dir = temp_project_dir("lsp_core_cross_file_definition_import_name");
        let lib_path = dir.join("math.nx");
        let main_path = dir.join("main.nx");
        let lib_source = r#"export fn calcular() -> int {
  return 1
}
"#;
        let main_source = r#"import calcular from "./math.nx"
let total = calcular()
"#;
        fs::write(&lib_path, lib_source).unwrap();
        fs::write(&main_path, main_source).unwrap();

        let main_uri = Url::from_file_path(main_path.canonicalize().unwrap()).unwrap();
        let lib_uri = Url::from_file_path(lib_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(main_uri.clone(), Some(1), main_source.to_string());

        let definition = core
            .goto_definition(&main_uri, position_of(main_source, 0, "calcular"))
            .expect("cross-file definition");

        assert_eq!(definition.uri, lib_uri);
        assert_eq!(
            definition.range.start,
            position_of(lib_source, 0, "calcular")
        );
    }

    #[test]
    fn lsp_core_goto_definition_keeps_same_document_fallback_for_dirty_snapshot() {
        let dir = temp_project_dir("lsp_core_cross_file_definition_dirty_entry");
        let lib_path = dir.join("cliente.nx");
        let main_path = dir.join("main.nx");
        let disk_main_source = r#"import Cliente as Pessoa from "./cliente.nx"
let cliente = Pessoa { nome: "Ana" }
"#;
        let dirty_main_source = r#"import Cliente as Pessoa from "./cliente.nx"
let cliente = Pessoa {
"#;
        fs::write(
            &lib_path,
            r#"export model Cliente {
  nome: string
}
"#,
        )
        .unwrap();
        fs::write(&main_path, disk_main_source).unwrap();

        let main_uri = Url::from_file_path(main_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(main_uri.clone(), Some(2), dirty_main_source.to_string());

        let definition = core
            .goto_definition(&main_uri, position_of(dirty_main_source, 1, "Pessoa"))
            .expect("same-document definition");

        assert_eq!(definition.uri, main_uri);
        assert_eq!(
            definition.range.start,
            position_of(dirty_main_source, 0, "Pessoa")
        );
    }

    #[test]
    fn lsp_core_goto_definition_uses_same_document_when_imported_snapshot_is_dirty() {
        let dir = temp_project_dir("lsp_core_cross_file_definition_dirty_import");
        let lib_path = dir.join("cliente.nx");
        let main_path = dir.join("main.nx");
        let lib_source = r#"export model Cliente {
  nome: string
}
"#;
        let main_source = r#"import Cliente as Pessoa from "./cliente.nx"
let cliente = Pessoa { nome: "Ana" }
"#;
        fs::write(&lib_path, lib_source).unwrap();
        fs::write(&main_path, main_source).unwrap();

        let main_uri = Url::from_file_path(main_path.canonicalize().unwrap()).unwrap();
        let lib_uri = Url::from_file_path(lib_path.canonicalize().unwrap()).unwrap();
        let mut core = LspCore::new();
        core.open_document(main_uri.clone(), Some(1), main_source.to_string());
        core.open_document(lib_uri, Some(2), "export model Cliente {".to_string());

        let definition = core
            .goto_definition(&main_uri, position_of(main_source, 1, "Pessoa"))
            .expect("same-document definition");

        assert_eq!(definition.uri, main_uri);
        assert_eq!(
            definition.range.start,
            position_of(main_source, 0, "Pessoa")
        );
    }

    #[test]
    fn find_definition_location_finds_local_declarations() {
        let source = r#"
fn calcular() {
  return 1
}

model Cliente {
  nome: String
}

let total = 10
"#;

        assert_eq!(find_definition_location(source, "calcular"), Some((2, 4)));
        assert_eq!(find_definition_location(source, "Cliente"), Some((6, 7)));
        assert_eq!(find_definition_location(source, "total"), Some((10, 5)));
        assert_eq!(find_definition_location(source, "missing"), None);
    }

    #[test]
    fn find_definition_location_finds_exported_declarations() {
        let source = r#"
export fn calcular() {
  return 1
}

export model Cliente {
  nome: String
}
"#;

        assert_eq!(find_definition_location(source, "calcular"), Some((2, 11)));
        assert_eq!(find_definition_location(source, "Cliente"), Some((6, 14)));
    }

    #[test]
    fn find_definition_location_resolves_import_aliases() {
        let source = r#"import Cliente as Pessoa from "./cliente.nx""#;

        assert_eq!(find_definition_location(source, "Pessoa"), Some((1, 19)));
        assert_eq!(find_definition_location(source, "Cliente"), None);
    }

    fn temp_project_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{name}_{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn batch_for<'a>(
        batches: &'a [DiagnosticPublishBatch],
        uri: &Url,
    ) -> &'a DiagnosticPublishBatch {
        batches
            .iter()
            .find(|batch| batch.uri == *uri)
            .expect("diagnostic batch")
    }

    fn position_of(source: &str, zero_based_line: usize, needle: &str) -> Position {
        let line = source.lines().nth(zero_based_line).expect("line");
        Position::new(
            zero_based_line as u32,
            line.find(needle).expect("needle") as u32,
        )
    }

    fn decode_semantic_tokens(tokens: &[SemanticToken]) -> Vec<AbsoluteSemanticToken> {
        let mut decoded = Vec::new();
        let mut line = 0;
        let mut start = 0;

        for token in tokens {
            line += token.delta_line;
            start = if token.delta_line == 0 {
                start + token.delta_start
            } else {
                token.delta_start
            };
            decoded.push(AbsoluteSemanticToken {
                line,
                start,
                length: token.length,
                token_type: token.token_type,
            });
        }

        decoded
    }

    fn assert_semantic_token(
        tokens: &[AbsoluteSemanticToken],
        source: &str,
        zero_based_line: usize,
        needle: &str,
        expected_length: usize,
        expected_type: u32,
    ) {
        let position = position_of(source, zero_based_line, needle);
        assert!(
            tokens.iter().any(|token| {
                token.line == position.line
                    && token.start == position.character
                    && token.length == expected_length as u32
                    && token.token_type == expected_type
            }),
            "missing semantic token for {needle:?}"
        );
    }

    fn nested_symbols(response: DocumentSymbolResponse) -> Vec<DocumentSymbol> {
        match response {
            DocumentSymbolResponse::Nested(symbols) => symbols,
            DocumentSymbolResponse::Flat(_) => panic!("expected nested document symbols"),
        }
    }

    fn find_symbol<'a>(symbols: &'a [DocumentSymbol], name: &str) -> &'a DocumentSymbol {
        symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap_or_else(|| panic!("missing document symbol {name:?}"))
    }

    fn range_contains(range: &Range, position: Position) -> bool {
        (position.line > range.start.line
            || position.line == range.start.line && position.character >= range.start.character)
            && (position.line < range.end.line
                || position.line == range.end.line && position.character <= range.end.character)
    }
}
