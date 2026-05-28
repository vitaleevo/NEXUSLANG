//! Multi-module diagnostic report, tooling view, and JSON serialization.
//!
//! This module owns the `MultiModuleDiagnosticReport` family of types,
//! the in-memory tooling APIs (summary, flattened view, source-context view),
//! the stable v1 JSON formatters, and the multi-module load/check/run
//! pipeline that produces structured diagnostics.
//!
//! The public API surface is re-exported from `lib.rs` so that downstream
//! consumers see no change.

use crate::ast;
use crate::checker::Checker;
use crate::diagnostic;
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::interpreter::Interpreter;
use crate::module_loader;
use std::path::{Path, PathBuf};

// ─── Structs ────────────────────────────────────────────────────────────

/// Successful structured result for a graph-loaded multi-module program.
///
/// This is the tooling-oriented shape: it preserves the merged program, module
/// graph, declaration-to-module map, and source database in one named value.
#[derive(Debug, Clone)]
pub struct CheckedMultiModuleProgram {
    pub program: ast::Program,
    pub module_graph: module_loader::ModuleGraph,
    pub decl_module_map: Vec<hir::HirModuleId>,
    pub source_database: module_loader::SourceDatabase,
}

/// Public structured diagnostic for multi-module entry points.
///
/// Checker diagnostics loaded through the module graph can expose all fields:
/// module path, module ID, diagnostic owner, and source range. Loader/runtime
/// diagnostics may only have the shared `Diagnostic` plus an optional path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleDiagnostic {
    pub path: Option<PathBuf>,
    pub module_id: Option<hir::HirModuleId>,
    pub diagnostic: Diagnostic,
    pub source_range: Option<module_loader::SourceRange>,
}

/// Minimal diagnostics collection for multi-module tooling.
///
/// Loader/parser/runtime flows still stop at the first error, while report
/// paths can collect checker diagnostics from independent declaration bodies.
/// This report type is additive: it gives tooling a stable collection/grouping
/// shape without changing `MultiModuleDiagnostic` or the existing first-error
/// JSON emitted by the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleDiagnosticReport {
    pub diagnostics: Vec<MultiModuleDiagnostic>,
}

/// Group of diagnostics that share the same owning module path and module ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleDiagnosticGroup {
    pub path: Option<PathBuf>,
    pub module_id: Option<hir::HirModuleId>,
    pub diagnostic_indexes: Vec<usize>,
}

/// In-memory summary of a multi-module diagnostic report for Rust tooling.
///
/// This is intentionally not part of the JSON v1 report contract. It gives
/// tools a cheap way to build dashboards and status indicators without parsing
/// or extending the serialized CLI payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleDiagnosticReportSummary {
    pub total: usize,
    pub has_diagnostics: bool,
    pub has_errors: bool,
    pub has_warnings: bool,
    pub stages: Vec<MultiModuleDiagnosticStageCount>,
    pub severities: Vec<MultiModuleDiagnosticSeverityCount>,
    pub paths: Vec<PathBuf>,
    pub module_ids: Vec<hir::HirModuleId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleDiagnosticStageCount {
    pub stage: diagnostic::DiagnosticStage,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleDiagnosticSeverityCount {
    pub severity: Option<diagnostic::DiagnosticSeverity>,
    pub count: usize,
}

/// In-memory flattened view of a multi-module diagnostic report for tooling.
///
/// This mirrors the report as groups plus one item per diagnostic, enriched
/// with group indexes. It is not serialized by the JSON v1 contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleDiagnosticReportView {
    pub summary: MultiModuleDiagnosticReportSummary,
    pub groups: Vec<MultiModuleDiagnosticGroup>,
    pub items: Vec<MultiModuleDiagnosticToolingItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleDiagnosticToolingItem {
    pub diagnostic_index: usize,
    pub group_index: usize,
    pub path: Option<PathBuf>,
    pub module_id: Option<hir::HirModuleId>,
    pub stage: diagnostic::DiagnosticStage,
    pub severity: Option<diagnostic::DiagnosticSeverity>,
    pub code: Option<String>,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub source_range: Option<module_loader::SourceRange>,
}

/// In-memory flattened report view enriched with optional source snippets.
///
/// This is an opt-in tooling API. It is never serialized into the JSON v1
/// contract and does not change diagnostic collection or CLI rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleDiagnosticReportSourceView {
    pub summary: MultiModuleDiagnosticReportSummary,
    pub groups: Vec<MultiModuleDiagnosticGroup>,
    pub items: Vec<MultiModuleDiagnosticToolingItemWithSourceContext>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleDiagnosticToolingItemWithSourceContext {
    pub item: MultiModuleDiagnosticToolingItem,
    pub source_context: Option<MultiModuleDiagnosticSourceContext>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleDiagnosticSourceContext {
    pub module_id: hir::HirModuleId,
    pub path: PathBuf,
    pub line: usize,
    pub column: Option<usize>,
    pub source_range: Option<module_loader::SourceRange>,
    pub line_text: String,
    pub highlight_start_column: Option<usize>,
    pub highlight_end_column: Option<usize>,
}

/// Structured run diagnostic for captured multi-module execution.
///
/// Runtime failures can happen after the program has already emitted output.
/// This wrapper preserves that captured output without changing
/// `MultiModuleDiagnostic` itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleRunDiagnostic {
    pub diagnostic: MultiModuleDiagnostic,
    pub output: Vec<String>,
}

/// Structured run diagnostic report for captured multi-module execution.
///
/// This is the report-shaped counterpart to `MultiModuleRunDiagnostic`: checker
/// failures can carry more than one diagnostic, while runtime failures remain a
/// single diagnostic plus any output emitted before the failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiModuleRunDiagnosticReport {
    pub report: MultiModuleDiagnosticReport,
    pub output: Vec<String>,
}

/// Version of the JSON contract emitted for multi-module diagnostics.
///
/// Version 1 is intentionally small: it mirrors `MultiModuleDiagnostic`, keeps
/// nullable fields explicit, and does not include LSP concepts, byte ranges, or
/// remote registry concepts. The first-error JSON remains unchanged; collection
/// reports are exposed through a separate additive formatter.
pub const MULTI_MODULE_DIAGNOSTIC_JSON_SCHEMA_VERSION: usize = 1;

// ─── MultiModuleDiagnostic impl ─────────────────────────────────────────

impl MultiModuleDiagnostic {
    pub fn from_module_error(error: module_loader::ModuleError) -> Self {
        let path = module_error_path(&error);
        let diagnostic = error.to_diagnostic();
        MultiModuleDiagnostic {
            path,
            module_id: None,
            diagnostic,
            source_range: None,
        }
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = diagnostic::runtime_code_for_message(&message);
        let diagnostic =
            Diagnostic::new(diagnostic::DiagnosticStage::Runtime, message).with_code(code);
        let diagnostic = diagnostic::enrich_runtime_diagnostic(diagnostic, code);
        MultiModuleDiagnostic {
            path: None,
            module_id: None,
            diagnostic,
            source_range: None,
        }
    }
}

impl From<module_loader::ModuleDiagnostic> for MultiModuleDiagnostic {
    fn from(module_diagnostic: module_loader::ModuleDiagnostic) -> Self {
        MultiModuleDiagnostic {
            path: Some(module_diagnostic.path),
            module_id: Some(module_diagnostic.module_id),
            diagnostic: module_diagnostic.diagnostic,
            source_range: module_diagnostic.source_range,
        }
    }
}

impl From<Diagnostic> for MultiModuleDiagnostic {
    fn from(diagnostic: Diagnostic) -> Self {
        MultiModuleDiagnostic {
            path: None,
            module_id: None,
            diagnostic,
            source_range: None,
        }
    }
}

impl std::fmt::Display for MultiModuleDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.module_id.is_some() || self.source_range.is_some() {
            if let Some(path) = &self.path {
                return match (self.diagnostic.line, self.diagnostic.column) {
                    (Some(line), Some(column)) => write!(
                        f,
                        "{}:{}:{}: {}",
                        path.display(),
                        line,
                        column,
                        self.diagnostic.message
                    ),
                    (Some(line), None) => {
                        write!(
                            f,
                            "{}:{}: {}",
                            path.display(),
                            line,
                            self.diagnostic.message
                        )
                    }
                    _ => write!(f, "{}: {}", path.display(), self.diagnostic.message),
                };
            }
        }

        write!(f, "{}", self.diagnostic)
    }
}

impl std::error::Error for MultiModuleDiagnostic {}

// ─── MultiModuleDiagnosticReport impl ───────────────────────────────────

impl MultiModuleDiagnosticReport {
    pub fn new(diagnostics: Vec<MultiModuleDiagnostic>) -> Self {
        MultiModuleDiagnosticReport { diagnostics }
    }

    pub fn empty() -> Self {
        MultiModuleDiagnosticReport {
            diagnostics: Vec::new(),
        }
    }

    pub fn from_diagnostic(diagnostic: MultiModuleDiagnostic) -> Self {
        MultiModuleDiagnosticReport {
            diagnostics: vec![diagnostic],
        }
    }

    pub fn diagnostics(&self) -> &[MultiModuleDiagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<MultiModuleDiagnostic> {
        self.diagnostics
    }

    pub fn push(&mut self, diagnostic: MultiModuleDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn first(&self) -> Option<&MultiModuleDiagnostic> {
        self.diagnostics.first()
    }

    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn summary(&self) -> MultiModuleDiagnosticReportSummary {
        let mut stages = Vec::new();
        let mut severities = Vec::new();
        let mut paths = Vec::new();
        let mut module_ids = Vec::new();
        let mut has_errors = false;
        let mut has_warnings = false;

        for diagnostic in &self.diagnostics {
            increment_stage_count(&mut stages, diagnostic.diagnostic.stage);
            increment_severity_count(&mut severities, diagnostic.diagnostic.severity);

            match diagnostic.diagnostic.severity {
                Some(diagnostic::DiagnosticSeverity::Error) => has_errors = true,
                Some(diagnostic::DiagnosticSeverity::Warning) => has_warnings = true,
                _ => {}
            }

            if let Some(path) = &diagnostic.path {
                push_unique_path(&mut paths, path);
            }

            if let Some(module_id) = diagnostic.module_id {
                push_unique_module_id(&mut module_ids, module_id);
            }
        }

        MultiModuleDiagnosticReportSummary {
            total: self.diagnostics.len(),
            has_diagnostics: !self.diagnostics.is_empty(),
            has_errors,
            has_warnings,
            stages,
            severities,
            paths,
            module_ids,
        }
    }

    pub fn tooling_view(&self) -> MultiModuleDiagnosticReportView {
        let groups = self.groups_by_path_and_module();
        let items = self.tooling_items_for_groups(&groups);
        MultiModuleDiagnosticReportView {
            summary: self.summary(),
            groups,
            items,
        }
    }

    pub fn tooling_items(&self) -> Vec<MultiModuleDiagnosticToolingItem> {
        let groups = self.groups_by_path_and_module();
        self.tooling_items_for_groups(&groups)
    }

    pub fn tooling_view_with_source_context(
        &self,
        source_database: Option<&module_loader::SourceDatabase>,
    ) -> MultiModuleDiagnosticReportSourceView {
        let groups = self.groups_by_path_and_module();
        let items = self.tooling_items_with_source_context_for_groups(&groups, source_database);
        MultiModuleDiagnosticReportSourceView {
            summary: self.summary(),
            groups,
            items,
        }
    }

    pub fn tooling_items_with_source_context(
        &self,
        source_database: Option<&module_loader::SourceDatabase>,
    ) -> Vec<MultiModuleDiagnosticToolingItemWithSourceContext> {
        let groups = self.groups_by_path_and_module();
        self.tooling_items_with_source_context_for_groups(&groups, source_database)
    }

    pub fn diagnostics_for_path(&self, path: &Path) -> Vec<&MultiModuleDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.path.as_deref() == Some(path))
            .collect()
    }

    pub fn diagnostics_for_module_id(
        &self,
        module_id: hir::HirModuleId,
    ) -> Vec<&MultiModuleDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.module_id == Some(module_id))
            .collect()
    }

    pub fn diagnostics_for_path_and_module(
        &self,
        path: Option<&Path>,
        module_id: Option<hir::HirModuleId>,
    ) -> Vec<&MultiModuleDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.path.as_deref() == path && diagnostic.module_id == module_id
            })
            .collect()
    }

    pub fn diagnostics_for_stage(
        &self,
        stage: diagnostic::DiagnosticStage,
    ) -> Vec<&MultiModuleDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.diagnostic.stage == stage)
            .collect()
    }

    pub fn diagnostics_for_severity(
        &self,
        severity: diagnostic::DiagnosticSeverity,
    ) -> Vec<&MultiModuleDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.diagnostic.severity == Some(severity))
            .collect()
    }

    pub fn diagnostics_for_group(
        &self,
        group: &MultiModuleDiagnosticGroup,
    ) -> Vec<&MultiModuleDiagnostic> {
        group
            .diagnostic_indexes
            .iter()
            .filter_map(|index| self.diagnostics.get(*index))
            .collect()
    }

    pub fn first_diagnostic_for_group(
        &self,
        group: &MultiModuleDiagnosticGroup,
    ) -> Option<&MultiModuleDiagnostic> {
        group
            .diagnostic_indexes
            .first()
            .and_then(|index| self.diagnostics.get(*index))
    }

    pub fn groups_by_path_and_module(&self) -> Vec<MultiModuleDiagnosticGroup> {
        let mut groups: Vec<MultiModuleDiagnosticGroup> = Vec::new();

        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if let Some(group) = groups.iter_mut().find(|group| {
                group.path == diagnostic.path && group.module_id == diagnostic.module_id
            }) {
                group.diagnostic_indexes.push(index);
            } else {
                groups.push(MultiModuleDiagnosticGroup {
                    path: diagnostic.path.clone(),
                    module_id: diagnostic.module_id,
                    diagnostic_indexes: vec![index],
                });
            }
        }

        groups
    }

    fn tooling_items_for_groups(
        &self,
        groups: &[MultiModuleDiagnosticGroup],
    ) -> Vec<MultiModuleDiagnosticToolingItem> {
        let mut diagnostic_group_indexes = vec![0; self.diagnostics.len()];
        for (group_index, group) in groups.iter().enumerate() {
            for diagnostic_index in &group.diagnostic_indexes {
                if *diagnostic_index < diagnostic_group_indexes.len() {
                    diagnostic_group_indexes[*diagnostic_index] = group_index;
                }
            }
        }

        self.diagnostics
            .iter()
            .enumerate()
            .map(
                |(diagnostic_index, diagnostic)| MultiModuleDiagnosticToolingItem {
                    diagnostic_index,
                    group_index: diagnostic_group_indexes[diagnostic_index],
                    path: diagnostic.path.clone(),
                    module_id: diagnostic.module_id,
                    stage: diagnostic.diagnostic.stage,
                    severity: diagnostic.diagnostic.severity,
                    code: diagnostic.diagnostic.code.clone(),
                    message: diagnostic.diagnostic.message.clone(),
                    line: diagnostic.diagnostic.line,
                    column: diagnostic.diagnostic.column,
                    source_range: diagnostic.source_range,
                },
            )
            .collect()
    }

    fn tooling_items_with_source_context_for_groups(
        &self,
        groups: &[MultiModuleDiagnosticGroup],
        source_database: Option<&module_loader::SourceDatabase>,
    ) -> Vec<MultiModuleDiagnosticToolingItemWithSourceContext> {
        self.tooling_items_for_groups(groups)
            .into_iter()
            .map(|item| {
                let source_context = source_database.and_then(|source_database| {
                    source_context_for_tooling_item(&item, source_database)
                });
                MultiModuleDiagnosticToolingItemWithSourceContext {
                    item,
                    source_context,
                }
            })
            .collect()
    }
}

impl From<MultiModuleDiagnostic> for MultiModuleDiagnosticReport {
    fn from(diagnostic: MultiModuleDiagnostic) -> Self {
        MultiModuleDiagnosticReport::from_diagnostic(diagnostic)
    }
}

// ─── Source context helpers ─────────────────────────────────────────────

fn source_context_for_tooling_item(
    item: &MultiModuleDiagnosticToolingItem,
    source_database: &module_loader::SourceDatabase,
) -> Option<MultiModuleDiagnosticSourceContext> {
    let source_module = item
        .module_id
        .and_then(|module_id| source_database.module(module_id))
        .or_else(|| {
            item.path
                .as_deref()
                .and_then(|path| source_database.module_by_path(path))
        })?;
    let line = item
        .line
        .or_else(|| item.source_range.map(|range| range.start.line))
        .filter(|line| *line != 0)?;
    let line_text = source_line(&source_module.source, line)?;
    let source_range = item.source_range.or_else(|| {
        source_database.source_range_for_module_location(source_module.module_id, line, item.column)
    });
    let (highlight_start_column, highlight_end_column) =
        source_highlight_columns(&line_text, line, item.column, source_range);

    Some(MultiModuleDiagnosticSourceContext {
        module_id: source_module.module_id,
        path: source_module.path.clone(),
        line,
        column: item.column,
        source_range,
        line_text,
        highlight_start_column,
        highlight_end_column,
    })
}

fn source_line(source: &str, line: usize) -> Option<String> {
    source
        .lines()
        .nth(line.checked_sub(1)?)
        .map(|line| line.to_string())
}

fn source_highlight_columns(
    line_text: &str,
    line: usize,
    column: Option<usize>,
    source_range: Option<module_loader::SourceRange>,
) -> (Option<usize>, Option<usize>) {
    if let Some(column) = column.filter(|column| *column != 0) {
        return (Some(column), Some(column.saturating_add(1)));
    }

    if let Some(range) = source_range {
        if range.start.line <= line && line <= range.end.line {
            let start = if line == range.start.line {
                range.start.column.max(1)
            } else {
                1
            };
            let end = if line == range.end.line {
                range.end.column.max(start.saturating_add(1))
            } else {
                line_text.chars().count().saturating_add(1).max(start)
            };
            return (Some(start), Some(end));
        }
    }

    (None, None)
}

// ─── Count helpers ──────────────────────────────────────────────────────

fn increment_stage_count(
    counts: &mut Vec<MultiModuleDiagnosticStageCount>,
    stage: diagnostic::DiagnosticStage,
) {
    if let Some(count) = counts.iter_mut().find(|count| count.stage == stage) {
        count.count += 1;
    } else {
        counts.push(MultiModuleDiagnosticStageCount { stage, count: 1 });
    }
}

fn increment_severity_count(
    counts: &mut Vec<MultiModuleDiagnosticSeverityCount>,
    severity: Option<diagnostic::DiagnosticSeverity>,
) {
    if let Some(count) = counts.iter_mut().find(|count| count.severity == severity) {
        count.count += 1;
    } else {
        counts.push(MultiModuleDiagnosticSeverityCount { severity, count: 1 });
    }
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: &Path) {
    if !paths.iter().any(|existing| existing.as_path() == path) {
        paths.push(path.to_path_buf());
    }
}

fn push_unique_module_id(module_ids: &mut Vec<hir::HirModuleId>, module_id: hir::HirModuleId) {
    if !module_ids.contains(&module_id) {
        module_ids.push(module_id);
    }
}

// ─── JSON formatters ────────────────────────────────────────────────────

/// Format a successful multi-module tooling response as JSON.
pub fn multi_module_success_json(command: &str, path: &Path) -> String {
    format!(
        "{{\"ok\":true,\"schema_version\":{},\"command\":{},\"path\":{}}}",
        MULTI_MODULE_DIAGNOSTIC_JSON_SCHEMA_VERSION,
        json_string(command),
        json_string(path.to_string_lossy().as_ref())
    )
}

/// Format a successful run/tooling response with captured output as JSON.
pub fn multi_module_success_output_json(command: &str, path: &Path, output: &[String]) -> String {
    format!(
        "{{\"ok\":true,\"schema_version\":{},\"command\":{},\"path\":{},\"output\":{}}}",
        MULTI_MODULE_DIAGNOSTIC_JSON_SCHEMA_VERSION,
        json_string(command),
        json_string(path.to_string_lossy().as_ref()),
        string_array_json(output)
    )
}

/// Format a `MultiModuleDiagnostic` using the stable v1 JSON contract.
pub fn multi_module_diagnostic_json(command: &str, error: &MultiModuleDiagnostic) -> String {
    format!(
        "{{\"ok\":false,\"schema_version\":{},\"command\":{},\"diagnostic\":{}}}",
        MULTI_MODULE_DIAGNOSTIC_JSON_SCHEMA_VERSION,
        json_string(command),
        diagnostic_payload_json(error)
    )
}

/// Format a diagnostic with captured output using the stable v1 JSON contract.
pub fn multi_module_diagnostic_output_json(
    command: &str,
    error: &MultiModuleDiagnostic,
    output: &[String],
) -> String {
    format!(
        "{{\"ok\":false,\"schema_version\":{},\"command\":{},\"diagnostic\":{},\"output\":{}}}",
        MULTI_MODULE_DIAGNOSTIC_JSON_SCHEMA_VERSION,
        json_string(command),
        diagnostic_payload_json(error),
        string_array_json(output)
    )
}

/// Format a multi-module diagnostic report for tooling.
///
/// This formatter is additive. `multi_module_diagnostic_json` remains the
/// stable first-error JSON used by `nexus check --json` and `nexus run --json`.
pub fn multi_module_diagnostic_report_json(
    command: &str,
    report: &MultiModuleDiagnosticReport,
) -> String {
    multi_module_diagnostic_report_json_fields(command, report, None)
}

/// Format a multi-module diagnostic report with captured run output.
///
/// This is the report-shaped counterpart to
/// `multi_module_diagnostic_output_json`. It is used by opt-in tooling paths
/// and does not change the stable first-error `run --json` shape.
pub fn multi_module_diagnostic_report_output_json(
    command: &str,
    report: &MultiModuleDiagnosticReport,
    output: &[String],
) -> String {
    multi_module_diagnostic_report_json_fields(command, report, Some(output))
}

fn multi_module_diagnostic_report_json_fields(
    command: &str,
    report: &MultiModuleDiagnosticReport,
    output: Option<&[String]>,
) -> String {
    let first_diagnostic = report
        .first()
        .map(diagnostic_payload_json)
        .unwrap_or_else(|| "null".to_string());
    let output_json = output
        .map(|output| format!(",\"output\":{}", string_array_json(output)))
        .unwrap_or_default();
    format!(
        "{{\"ok\":{},\"schema_version\":{},\"command\":{},\"diagnostic\":{},\"diagnostics\":{},\"groups\":{}{}}}",
        if report.is_empty() { "true" } else { "false" },
        MULTI_MODULE_DIAGNOSTIC_JSON_SCHEMA_VERSION,
        json_string(command),
        first_diagnostic,
        diagnostic_array_json(report.diagnostics()),
        diagnostic_groups_json(&report.groups_by_path_and_module()),
        output_json
    )
}

fn diagnostic_payload_json(error: &MultiModuleDiagnostic) -> String {
    format!(
        "{{\"code\":{},\"severity\":{},\"stage\":{},\"message\":{},\"line\":{},\"column\":{},\"path\":{},\"module_id\":{},\"owner\":{},\"source_range\":{},\"labels\":{},\"notes\":{},\"suggestions\":{},\"text\":{}}}",
        optional_string_json(error.diagnostic.code.as_deref()),
        optional_severity_json(error.diagnostic.severity),
        json_string(error.diagnostic.stage.as_str()),
        json_string(&error.diagnostic.message),
        optional_usize_json(error.diagnostic.line),
        optional_usize_json(error.diagnostic.column),
        optional_path_json(error.path.as_deref()),
        optional_module_id_json(error.module_id),
        diagnostic_owner_json(error.diagnostic.owner),
        source_range_json(error.source_range),
        diagnostic_labels_json(&error.diagnostic.labels),
        string_array_json(&error.diagnostic.notes),
        diagnostic_suggestions_json(&error.diagnostic.suggestions),
        json_string(&error.to_string())
    )
}

fn diagnostic_array_json(diagnostics: &[MultiModuleDiagnostic]) -> String {
    let mut out = String::from("[");
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&diagnostic_payload_json(diagnostic));
    }
    out.push(']');
    out
}

fn diagnostic_groups_json(groups: &[MultiModuleDiagnosticGroup]) -> String {
    let mut out = String::from("[");
    for (index, group) in groups.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"path\":{},\"module_id\":{},\"diagnostic_indexes\":{}}}",
            optional_path_json(group.path.as_deref()),
            optional_module_id_json(group.module_id),
            usize_array_json(&group.diagnostic_indexes)
        ));
    }
    out.push(']');
    out
}

fn usize_array_json(values: &[usize]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&value.to_string());
    }
    out.push(']');
    out
}

fn diagnostic_owner_json(owner: Option<diagnostic::DiagnosticOwner>) -> String {
    owner
        .map(|owner| {
            format!(
                "{{\"decl_index\":{},\"module_id\":{}}}",
                owner.decl_index,
                optional_usize_json(owner.module_id)
            )
        })
        .unwrap_or_else(|| "null".to_string())
}

fn source_range_json(range: Option<module_loader::SourceRange>) -> String {
    range
        .map(|range| {
            format!(
                "{{\"start\":{},\"end\":{}}}",
                span_json(range.start),
                span_json(range.end)
            )
        })
        .unwrap_or_else(|| "null".to_string())
}

fn diagnostic_labels_json(labels: &[diagnostic::DiagnosticLabel]) -> String {
    let mut out = String::from("[");
    for (index, label) in labels.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"message\":{},\"line\":{},\"column\":{}}}",
            json_string(&label.message),
            optional_usize_json(label.line),
            optional_usize_json(label.column)
        ));
    }
    out.push(']');
    out
}

fn diagnostic_suggestions_json(suggestions: &[diagnostic::DiagnosticSuggestion]) -> String {
    let mut out = String::from("[");
    for (index, suggestion) in suggestions.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"message\":{},\"replacement\":{}}}",
            json_string(&suggestion.message),
            optional_string_json(suggestion.replacement.as_deref())
        ));
    }
    out.push(']');
    out
}

fn span_json(span: ast::Span) -> String {
    format!("{{\"line\":{},\"column\":{}}}", span.line, span.column)
}

fn optional_module_id_json(module_id: Option<hir::HirModuleId>) -> String {
    optional_usize_json(module_id.map(|module_id| module_id.0))
}

fn optional_path_json(path: Option<&Path>) -> String {
    path.map(|path| json_string(path.to_string_lossy().as_ref()))
        .unwrap_or_else(|| "null".to_string())
}

fn optional_usize_json(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn optional_string_json(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_string())
}

fn optional_severity_json(severity: Option<diagnostic::DiagnosticSeverity>) -> String {
    severity
        .map(|severity| json_string(severity.as_str()))
        .unwrap_or_else(|| "null".to_string())
}

fn string_array_json(values: &[String]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&json_string(value));
    }
    out.push(']');
    out
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch < '\u{20}' => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

// ─── Module error helpers ───────────────────────────────────────────────

fn module_error_path(error: &module_loader::ModuleError) -> Option<PathBuf> {
    match error {
        module_loader::ModuleError::IoError { path, .. }
        | module_loader::ModuleError::ParseError { path, .. }
        | module_loader::ModuleError::CircularDependency { path, .. }
        | module_loader::ModuleError::DuplicateImportAlias { path, .. }
        | module_loader::ModuleError::ImportAliasCollision { path, .. } => Some(path.clone()),
        module_loader::ModuleError::SymbolNotExported { target, .. } => Some(target.clone()),
        module_loader::ModuleError::DuplicateGraphSymbol { duplicate_path, .. } => {
            Some(duplicate_path.clone())
        }
        module_loader::ModuleError::NonRelativePath { .. }
        | module_loader::ModuleError::PackageImport { .. }
        | module_loader::ModuleError::PathResolution { .. }
        | module_loader::ModuleError::StdLibNotFound { .. } => None,
    }
}

// ─── Load/check/run pipeline ────────────────────────────────────────────

/// Load a multi-file project with graph and source database, then check it
/// with cross-module import resolution.
///
/// This is the first tooling-oriented API for multi-module NexusLang: it keeps
/// paths, source text, and import edges available without changing import
/// semantics.
pub fn load_and_check_with_source_database(
    entry_path: &std::path::Path,
) -> Result<
    (
        ast::Program,
        module_loader::ModuleGraph,
        module_loader::SourceDatabase,
    ),
    String,
> {
    let checked = load_and_check_with_source_database_diagnostic(entry_path)
        .map_err(|diagnostic| diagnostic.to_string())?;
    Ok((
        checked.program,
        checked.module_graph,
        checked.source_database,
    ))
}

/// Load and check a multi-module program, returning structured diagnostics.
pub fn load_and_check_with_source_database_diagnostic(
    entry_path: &Path,
) -> Result<CheckedMultiModuleProgram, MultiModuleDiagnostic> {
    let (program, module_graph, decl_module_map, source_database) =
        module_loader::load_program_full_with_source_database(entry_path)
            .map_err(MultiModuleDiagnostic::from_module_error)?;
    check_with_source_database(&program, &module_graph, &decl_module_map, &source_database)
        .map_err(MultiModuleDiagnostic::from)?;
    Ok(CheckedMultiModuleProgram {
        program,
        module_graph,
        decl_module_map,
        source_database,
    })
}

/// Check an already-loaded multi-file program and attach checker diagnostics
/// to the owning module path when possible.
pub fn check_with_source_database(
    program: &ast::Program,
    module_graph: &module_loader::ModuleGraph,
    decl_module_map: &[hir::HirModuleId],
    source_database: &module_loader::SourceDatabase,
) -> Result<(), module_loader::ModuleDiagnostic> {
    let mut checker = Checker::new();
    checker
        .check_with_module_graph(program, module_graph, decl_module_map)
        .map_err(|diagnostic| {
            attach_checker_diagnostic(
                program,
                module_graph,
                decl_module_map,
                source_database,
                diagnostic,
            )
        })
}

fn attach_checker_diagnostic(
    program: &ast::Program,
    module_graph: &module_loader::ModuleGraph,
    decl_module_map: &[hir::HirModuleId],
    source_database: &module_loader::SourceDatabase,
    diagnostic: Diagnostic,
) -> module_loader::ModuleDiagnostic {
    source_database
        .attach_program_diagnostic(program, decl_module_map, diagnostic.clone())
        .or_else(|| source_database.attach_diagnostic(module_graph.entry_id, diagnostic.clone()))
        .unwrap_or(module_loader::ModuleDiagnostic {
            module_id: module_graph.entry_id,
            path: std::path::PathBuf::new(),
            diagnostic,
            source_range: None,
        })
}

/// Check an already-loaded multi-file program and collect report-safe checker
/// diagnostics where declarations can be checked independently.
pub fn check_with_source_database_diagnostic_report(
    program: &ast::Program,
    module_graph: &module_loader::ModuleGraph,
    decl_module_map: &[hir::HirModuleId],
    source_database: &module_loader::SourceDatabase,
) -> Result<(), MultiModuleDiagnosticReport> {
    let mut checker = Checker::new();
    checker
        .check_with_module_graph_diagnostics(program, module_graph, decl_module_map)
        .map_err(|diagnostics| {
            MultiModuleDiagnosticReport::new(
                diagnostics
                    .into_iter()
                    .map(|diagnostic| {
                        attach_checker_diagnostic(
                            program,
                            module_graph,
                            decl_module_map,
                            source_database,
                            diagnostic,
                        )
                        .into()
                    })
                    .collect(),
            )
        })
}

/// Load and check a multi-module program, returning a diagnostic report.
///
/// Loader/parser/setup failures still surface as one diagnostic, while checker
/// declaration-body diagnostics are collected when continuing is safe.
pub fn load_and_check_with_source_database_diagnostic_report(
    entry_path: &Path,
) -> Result<CheckedMultiModuleProgram, MultiModuleDiagnosticReport> {
    let (program, module_graph, decl_module_map, source_database) =
        module_loader::load_program_full_with_source_database(entry_path)
            .map_err(MultiModuleDiagnostic::from_module_error)
            .map_err(MultiModuleDiagnosticReport::from_diagnostic)?;
    check_with_source_database_diagnostic_report(
        &program,
        &module_graph,
        &decl_module_map,
        &source_database,
    )?;
    Ok(CheckedMultiModuleProgram {
        program,
        module_graph,
        decl_module_map,
        source_database,
    })
}

/// Load a multi-file project with graph, check with cross-module import
/// resolution (including aliases), and run the merged program through the
/// interpreter.
///
/// This is the recommended entry point for multi-module NexusLang programs
/// that use `import X as Y` aliases and need the full runtime pipeline.
pub fn load_and_run_with_source_database(entry_path: &std::path::Path) -> Result<(), String> {
    load_and_run_with_source_database_diagnostic(entry_path)
        .map_err(|diagnostic| diagnostic.to_string())
}

/// Load, check, and run a multi-module program with structured diagnostics for
/// loading/checking failures.
pub fn load_and_run_with_source_database_diagnostic(
    entry_path: &Path,
) -> Result<(), MultiModuleDiagnostic> {
    let checked = load_and_check_with_source_database_diagnostic(entry_path)?;
    let mut interp = Interpreter::new();
    interp
        .run(&checked.program)
        .map_err(MultiModuleDiagnostic::from)
}

/// Load, check, and run a multi-module program while capturing program output.
///
/// This is intended for JSON/tooling integrations. Textual execution should
/// keep using `load_and_run_with_source_database_diagnostic`, which prints
/// program output directly as before.
pub fn load_and_run_with_source_database_captured_diagnostic(
    entry_path: &Path,
) -> Result<Vec<String>, MultiModuleRunDiagnostic> {
    let checked =
        load_and_check_with_source_database_diagnostic(entry_path).map_err(|diagnostic| {
            MultiModuleRunDiagnostic {
                diagnostic,
                output: Vec::new(),
            }
        })?;
    let mut interp = Interpreter::new_captured();
    match interp.run(&checked.program) {
        Ok(()) => Ok(interp.output().to_vec()),
        Err(diagnostic) => Err(MultiModuleRunDiagnostic {
            diagnostic: diagnostic.into(),
            output: interp.output().to_vec(),
        }),
    }
}

/// Load, check, and run a multi-module program while capturing output and using
/// the report-shaped diagnostics contract for opt-in tooling callers.
pub fn load_and_run_with_source_database_captured_diagnostic_report(
    entry_path: &Path,
) -> Result<Vec<String>, MultiModuleRunDiagnosticReport> {
    let checked =
        load_and_check_with_source_database_diagnostic_report(entry_path).map_err(|report| {
            MultiModuleRunDiagnosticReport {
                report,
                output: Vec::new(),
            }
        })?;
    let mut interp = Interpreter::new_captured();
    match interp.run(&checked.program) {
        Ok(()) => Ok(interp.output().to_vec()),
        Err(diagnostic) => Err(MultiModuleRunDiagnosticReport {
            report: MultiModuleDiagnosticReport::from_diagnostic(diagnostic.into()),
            output: interp.output().to_vec(),
        }),
    }
}
