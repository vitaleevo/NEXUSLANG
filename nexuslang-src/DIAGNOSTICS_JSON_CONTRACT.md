# NexusLang Diagnostics JSON Contract

This document defines the current JSON contract for structured diagnostics
emitted by NexusLang multi-module tooling.

## Version

Current schema version: `1`.

The schema version is emitted as the top-level `schema_version` field. Version
1 mirrors `MultiModuleDiagnostic` and intentionally stays small: it does not
define LSP payloads, byte ranges, related diagnostics, or a remote registry
contract. It does include additive guidance metadata for labels, notes, and
suggestions.

## Producers

- CLI: `nexus check --json <file.nx>` emits this contract for validation.
- CLI: `nexus check --json-report <file.nx>` emits the additive report object
  for tooling. Loader/parser/setup failures still produce a one-item report,
  while checker declaration-body failures can include multiple diagnostics
  when declarations are independent enough to continue checking safely.
- CLI: `nexus run --json <file.nx>` emits this contract for captured
  execution output and diagnostics.
- CLI: `nexus run --json-report <file.nx>` emits the additive report object
  plus captured `output` for success, loader/checker diagnostics, and runtime
  diagnostics with partial output.
- Rust API: `multi_module_diagnostic_json(command, diagnostic)` formats any
  `MultiModuleDiagnostic`, including loader, checker, and runtime stages.
- Rust API: `multi_module_diagnostic_report_json(command, report)` formats a
  `MultiModuleDiagnosticReport` collection for tooling. This is the same
  additive report shape used by `nexus check --json-report`; plain CLI JSON
  still emits the first-error object below.
- Rust API: `MultiModuleDiagnosticReport` exposes tooling helpers for querying
  diagnostics by path, module ID, path/module pair, stage, severity, and
  report group without changing JSON v1.
- Rust API: `multi_module_diagnostic_report_output_json(command, report,
  output)` formats a report with captured run output. This is the same additive
  report shape used by `nexus run --json-report`.
- Rust API: `multi_module_success_json(command, path)` formats successful
  command responses.
- Rust API: `multi_module_success_output_json(command, path, output)` formats
  successful command responses with captured program output.
- Rust API: `multi_module_diagnostic_output_json(command, diagnostic, output)`
  formats diagnostics with captured program output.

`nexus check --json` never emits `runtime` diagnostics because `check` does not
execute the program. `nexus run --json` can emit `module_loader`, `checker`, or
`runtime` diagnostics and includes captured output as a top-level `output`
array.

## Diagnostic Report Object

The report object is an additive shape for tooling that wants a collection and
grouping contract before NexusLang has parser recovery or a full diagnostic
collector. It is available from `nexus check --json-report`,
`nexus run --json-report`, `multi_module_diagnostic_report_json()`, and
`multi_module_diagnostic_report_output_json()`. Loader/parser/setup and
runtime flows still stop at the first error; checker report paths can now
collect diagnostics from independent declaration bodies after successful
module loading and symbol collection.

```json
{
  "ok": false,
  "schema_version": 1,
  "command": "check",
  "diagnostic": {
    "...": "first diagnostic payload, same shape as Error Object"
  },
  "diagnostics": [
    {
      "...": "diagnostic payload"
    }
  ],
  "groups": [
    {
      "path": "lib.nx",
      "module_id": 1,
      "diagnostic_indexes": [0]
    }
  ],
  "output": ["line emitted before failure"]
}
```

Fields:

- `diagnostic`: the first diagnostic payload, or `null` when the report is
  empty. This keeps first-error consumers easy to bridge.
- `diagnostics`: all diagnostics in report order.
- `groups`: diagnostics grouped by owning `path` and `module_id`.
- `groups[].diagnostic_indexes`: indexes into the `diagnostics` array.
- `output`: captured program stdout lines for `run --json-report` and
  report-output Rust API responses. It is omitted by `check --json-report`.

Rust tooling helpers on `MultiModuleDiagnosticReport`:

- `diagnostics_for_path(path)`.
- `diagnostics_for_module_id(module_id)`.
- `diagnostics_for_path_and_module(path, module_id)`.
- `diagnostics_for_stage(stage)`.
- `diagnostics_for_severity(severity)`.
- `diagnostics_for_group(group)`.
- `first_diagnostic_for_group(group)`.
- `summary()`.
- `tooling_view()`.
- `tooling_items()`.
- `tooling_view_with_source_context(Option<&SourceDatabase>)`.
- `tooling_items_with_source_context(Option<&SourceDatabase>)`.

These helpers are in-memory navigation APIs only. They do not add fields to the
JSON v1 report object.

Public tooling API stability matrix:

| Surface | Stable pre-LSP contract | Ordering | JSON v1 behavior |
| --- | --- | --- | --- |
| `MultiModuleDiagnosticReport` | Owns the diagnostic collection and exposes `diagnostics()`, `first()`, `len()`, `is_empty()`, `into_diagnostics()`, and `push()` for in-memory consumers. | Diagnostics stay in collection order. | Report JSON serializes `diagnostic`, `diagnostics`, and `groups`; first-error JSON serializes only `diagnostic`. |
| Filter/group helpers | Query by path, module ID, path/module pair, stage, severity, group, and first diagnostic for a group. | Filters preserve report order; groups preserve first-seen path/module order. | Helper names and derived filter results are never serialized. |
| `summary()` | Returns total/flags, stage counts, severity counts, unique paths, and unique module IDs. | Counts and unique lists preserve first-seen order. | `summary`, `has_errors`, count buckets, paths, and module IDs from the summary are not serialized as a summary object. |
| `tooling_view()` / `tooling_items()` | Return flattened items with diagnostic/group indexes, path, module ID, stage, severity, code, message, line/column, and source range. | Items stay in diagnostic order; `group_index` points into the view groups. | `items`, `diagnostic_index`, and `group_index` are in-memory only. |
| Source-context view | Adds optional `source_context` with module path/ID, source line, existing line/column/range, and highlight columns when a `SourceDatabase` resolves the item. | Mirrors the flattened item order. | `source_context`, `line_text`, and highlight columns are in-memory only. |

The stability promise is additive: new helper methods or optional in-memory
fields can be added, but the current JSON v1 shapes and legacy first-error
wrappers must not be widened by these tooling conveniences. These APIs are
pre-LSP: they do not define document URIs, byte ranges, related diagnostics,
workspace edits, or incremental invalidation.

`summary()` returns a `MultiModuleDiagnosticReportSummary` with:

- `total`, `has_diagnostics`, `has_errors`, and `has_warnings`.
- Counts by `stage`, preserving the first-seen stage order.
- Counts by optional `severity`, including a `None` bucket when present and
  preserving first-seen severity order.
- Unique affected `paths` and `module_ids`, preserving first-seen order.

The summary is a Rust tooling convenience only. It is not serialized by
`nexus check --json-report`, `nexus run --json-report`, or any first-error JSON
formatter.

`tooling_view()` returns a `MultiModuleDiagnosticReportView` with:

- `summary`: the same in-memory summary returned by `summary()`.
- `groups`: path/module groups from `groups_by_path_and_module()`.
- `items`: one `MultiModuleDiagnosticToolingItem` per diagnostic in report
  order.

Each flattened tooling item carries:

- `diagnostic_index`: index into `report.diagnostics()`.
- `group_index`: index into `view.groups`.
- `path` and `module_id`.
- `stage`, optional `severity`, optional `code`, and `message`.
- `line` and `column` when known.
- `source_range` when the source database could attach one.

`tooling_items()` returns only the flattened `items` vector. Both methods are
Rust in-memory APIs only; `diagnostic_index`, `group_index`, and the view shape
are not serialized into JSON v1.

Source context is opt-in:

- `tooling_view_with_source_context(Some(&source_database))` returns a
  `MultiModuleDiagnosticReportSourceView`.
- `tooling_items_with_source_context(Some(&source_database))` returns flattened
  items with optional `source_context`.
- Passing `None` is valid and produces the same flattened items with no source
  snippets attached.

`MultiModuleDiagnosticSourceContext` carries only in-memory source data:

- `module_id` and `path`.
- Existing diagnostic `line`, optional `column`, and `source_range` when known.
- `line_text`: the source line containing the diagnostic/range.
- Optional `highlight_start_column` and `highlight_end_column`.

These APIs use `SourceDatabase` when the caller has one. Diagnostics without a
source owner, or reports created before a source database is available, keep
`source_context: None`. The snippet fields are not byte ranges and are not
serialized into JSON v1.

`nexus-lsp` consumes this in-memory report/source-database surface for local
projects whose opened entry snapshot matches disk. It converts report
diagnostics into LSP publication batches grouped by document URI, but this is
only an editor-adapter concern: JSON v1 does not gain LSP fields, document
versions, workspace edits, or incremental invalidation data.

Rust tooling fixtures and examples:

- `examples/diagnostic_report_tooling.rs` is a compilable Cargo example that
  reads an entry `.nx` file, consumes `MultiModuleDiagnosticReport`,
  `tooling_view_with_source_context()`, flattened items, optional source
  snippets, `summary()`, groups, and captured run output.
- `tests/core.rs` keeps fixture-backed examples for checker, module-loader, and
  runtime reports:
  - `diagnostic_report_tooling_example_consumes_checker_fixture`;
  - `diagnostic_report_tooling_example_consumes_module_loader_fixture`;
  - `diagnostic_report_tooling_example_consumes_runtime_fixture`.

Minimal Rust consumption pattern:

```rust
let loaded = nexuslang::module_loader::load_program_full_with_source_database(path)
    .expect("source database is available");
let source_database = loaded.source_database;
let report = nexuslang::load_and_check_with_source_database_diagnostic_report(path)
    .expect_err("tooling example expects diagnostics");
let view = report.tooling_view_with_source_context(Some(&source_database));

if view.summary.has_errors {
    for item in view.items {
        let diagnostic = &item.item;
        let snippet = item.source_context.as_ref();
        // Tooling can render `diagnostic.path`, `diagnostic.group_index`,
        // `diagnostic.stage`, `diagnostic.message`, and optional snippet
        // context without parsing JSON or changing the CLI contract.
    }
}
```

Current collection limits:

- Parser recovery is not implemented, so parse errors remain one-item reports.
- Loader graph failures remain one-item reports.
- Checker collection is limited to declaration-body diagnostics after the
  global collection/setup pass succeeds.
- The checker report matrix is covered for independent `function`, `route`,
  `workflow`, and `invoice` declaration-body diagnostics.
- Global checker setup failures, such as duplicate top-level declarations,
  remain one-item reports.
- Top-level statements remain order-dependent and stop at the first statement
  diagnostic to avoid cascading errors.
- Runtime diagnostics remain single-error, with partial output when execution
  already emitted lines.

## Success Object

For `check`:

```json
{
  "ok": true,
  "schema_version": 1,
  "command": "check",
  "path": "main.nx"
}
```

For `run`:

```json
{
  "ok": true,
  "schema_version": 1,
  "command": "run",
  "path": "main.nx",
  "output": ["line 1", "line 2"]
}
```

Fields:

- `ok`: always `true` for success.
- `schema_version`: JSON contract version.
- `command`: command or producer label, currently `check` or `run` for CLI
  output.
- `path`: path received by the command or API formatter.
- `output`: captured program stdout lines for `run` responses.

## Error Object

```json
{
  "ok": false,
  "schema_version": 1,
  "command": "run",
  "diagnostic": {
    "code": "NXL5001",
    "severity": "error",
    "stage": "runtime",
    "message": "Divisão por zero",
    "line": null,
    "column": null,
    "path": null,
    "module_id": null,
    "owner": null,
    "source_range": null,
    "labels": [
      {
        "message": "operacao aritmetica em runtime",
        "line": null,
        "column": null
      }
    ],
    "notes": ["A execucao tentou dividir ou calcular modulo por zero."],
    "suggestions": [
      {
        "message": "Garanta que o divisor seja diferente de zero antes da operacao.",
        "replacement": null
      }
    ],
    "text": "Divisão por zero"
  },
  "output": ["line emitted before failure"]
}
```

Fields:

- `ok`: always `false` for diagnostics.
- `schema_version`: JSON contract version.
- `command`: command or producer label.
- `diagnostic.code`: optional stable diagnostic code, or `null` when a producer
  has no code yet.
- `diagnostic.severity`: optional severity, or `null` when a producer has no
  severity yet. Current emitted values are `error`, `warning`, `info`, or
  `hint`.
- `diagnostic.stage`: one of `input`, `lexer`, `parser`, `checker`,
  `module_loader`, or `runtime`.
- `diagnostic.message`: human-readable diagnostic message.
- `diagnostic.line` / `diagnostic.column`: one-based source location, or
  `null` when unknown.
- `diagnostic.path`: owning source path, or `null` when the diagnostic is not
  attached to a file.
- `diagnostic.module_id`: module graph ID, or `null` when unavailable.
- `diagnostic.owner`: declaration owner metadata, or `null`.
- `diagnostic.source_range`: owning declaration/source range, or `null`.
- `diagnostic.labels`: optional source labels. Each label is an object with
  `message`, `line`, and `column`; unknown locations are emitted as `null`.
- `diagnostic.notes`: optional human-readable notes for tooling.
- `diagnostic.suggestions`: optional suggested fixes. Each suggestion has a
  `message` and optional `replacement` string.
- `diagnostic.text`: current human-readable rendering of the diagnostic.
- `output`: captured program stdout lines for `run` diagnostics. Loader/checker
  failures usually emit `[]`; runtime failures can include partial output.

## Stage Notes

- Diagnostic codes are stable within schema version 1. The current catalog is:

| Code | Stage | Family |
| --- | --- | --- |
| `NXL0001` | `input` | generic input error |
| `NXL1001` | `lexer` | invalid character |
| `NXL1002` | `lexer` | unterminated string |
| `NXL1003` | `lexer` | invalid operator |
| `NXL2001` | `parser` | generic syntax error |
| `NXL2002` | `parser` | import syntax |
| `NXL2003` | `parser` | export syntax |
| `NXL2004` | `parser` | declaration syntax |
| `NXL2005` | `parser` | expression syntax |
| `NXL2006` | `parser` | statement syntax |
| `NXL3001` | `checker` | type compatibility |
| `NXL3002` | `checker` | symbol resolution |
| `NXL3003` | `checker` | assignment |
| `NXL3004` | `checker` | model operation or field |
| `NXL3005` | `checker` | route |
| `NXL3006` | `checker` | auth |
| `NXL3007` | `checker` | workflow |
| `NXL3008` | `checker` | invoice |
| `NXL3009` | `checker` | argument arity/value |
| `NXL3099` | `checker` | generic checker error |
| `NXL4001` | `module_loader` | IO |
| `NXL4002` | `module_loader` | parse failure while loading |
| `NXL4003` | `module_loader` | circular dependency |
| `NXL4004` | `module_loader` | symbol not exported |
| `NXL4005` | `module_loader` | duplicate graph symbol |
| `NXL4006` | `module_loader` | duplicate import alias |
| `NXL4007` | `module_loader` | import alias collision |
| `NXL4008` | `module_loader` | path resolution |
| `NXL4009` | `module_loader` | local path dependency resolution |
| `NXL4010` | `module_loader` | stdlib module resolution |
| `NXL5001` | `runtime` | division or modulo by zero |
| `NXL5002` | `runtime` | undefined variable |
| `NXL5003` | `runtime` | undefined function |
| `NXL5004` | `runtime` | model operation or field |
| `NXL5005` | `runtime` | workflow |
| `NXL5099` | `runtime` | generic runtime error |

- `module_loader` diagnostics can have a path and line/column, but usually do
  not have `module_id`, `owner`, or `source_range`.
- `checker` diagnostics from graph-loaded programs can expose path,
  `module_id`, `owner`, and `source_range`.
- `runtime` diagnostics emitted by `nexus run --json` expose the runtime stage,
  message, and any output emitted before the failure. They do not yet carry
  source path/range.
- As of this contract revision, labels/notes/suggestions are populated for
  parser import/export syntax; checker type, symbol, argument, model, route,
  auth, workflow, and invoice diagnostics; module-loader symbol-not-exported,
  duplicate-symbol, duplicate-alias, alias-collision, path, package, and stdlib
  diagnostics; and runtime division/modulo-by-zero, undefined-variable,
  undefined-function, model, and workflow diagnostics. Assignment, generic,
  IO/parse/cycle, and other lower-signal families may still emit empty arrays.

## Compatibility Rules

- Existing textual CLI output remains the default.
- Existing `String` wrappers remain available.
- `multi_module_diagnostic_json` and CLI `--json` keep the first-error object
  shape. The report JSON is a separate API and must not be required by existing
  consumers.
- `nexus check --json-report` and `nexus run --json-report` are opt-in report
  modes. They may include multiple checker diagnostics, but must not change the
  shape of `check --json` or `run --json`.
- `code`, `severity`, `labels`, `notes`, and `suggestions` are additive
  metadata and must not be required to render the human message.
- New fields should be additive for schema version 1 whenever possible.
- Breaking shape changes should use a new `schema_version`.
