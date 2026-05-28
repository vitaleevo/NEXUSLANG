# Modules, Imports, And Exports Architecture

Last updated: 2026-05-27

This note defines the smallest safe path for adding modules, imports, and
exports to NexusLang after the Phase 10 typed-HIR definition/use link closure.
It is a design boundary for the first implementation slice, not a complete
package system.

## Current Constraints

- Many public compiler APIs still parse, check, format, lint, and run one
  source string at a time through `Program { decls }`, but the graph-aware
  path APIs can now return a minimal `SourceDatabase` for tooling.
- `nexus check`, `nexus run`, and `nexus serve` can use one explicit `.nx`
  file or the nearest `nexus.toml [package].entry`. Debug/tooling commands
  such as `tokens`, `ast`, `fmt`, and `lint` remain mostly single-file.
- `nexus check` and `nexus run` now use the source-database-aware load/check
  path so checker diagnostics in imported modules can display the owning path.
  `nexus check --json` emits the same multi-module diagnostic as structured
  JSON for tooling.
- `nexus.toml` now feeds entrypoint loading and direct local `path:`
  dependencies into the module graph. `nexus.lock` and `.nexus/packages/`
  remain metadata/cache artifacts for now; registry dependencies are not
  compiler inputs yet.
- HIR IDs are stable inside one `HirProgram`, but raw `HirSymbolId` values are
  not globally meaningful across multiple modules.
- The checker still keeps AST compatibility maps for public diagnostics while
  typed-HIR metadata is produced through the centralized writer in
  `checker/typed_hir_pass.rs`.

## MVP Decisions

- A source file is an implicit module. Do not add a `module` keyword in the
  first slice.
- Use contextual keywords for `import`, `export`, `from`, and `as` so existing
  identifiers are not reserved globally.
- Support one imported symbol per import declaration. This avoids import-list
  parsing and gives every imported use one stable span.
- Use `export` as an explicit modifier for named top-level declarations.
- Keep imports declaration-only and side-effect-free. Imported modules should
  not execute top-level statements in the first multi-file implementation.
- Make module paths string literals. The current resolver supports relative
  local `.nx` paths, `std/<module>`, and package-name imports backed by direct
  local `path:` dependencies.
- Defer wildcard imports, namespace imports, default exports, re-exports,
  registry-backed package imports, route exports, invoice exports, and
  module-qualified names.
- Keep the loaded graph's named symbol surface flat by kind for the MVP:
  `fn`, `model`, `workflow`, and `auth` names may not be duplicated across
  loaded modules for the same kind. Namespace/package-qualified symbol
  surfaces are deferred.

## Minimal Syntax

```nexus
import Customer from "./crm.nx"
import BuildInvoice as InvoiceFlow from "./billing.nx"

export model Customer {
    name: string
}

export fn customer_label(customer: Customer) -> string {
    return customer.name
}

export workflow BuildInvoice {
    step start {
        print("invoice")
    }
}

export auth UserAuth {
    model: User
    identity: email
}
```

Grammar sketch:

```text
ImportDecl := "import" Ident ("as" Ident)? "from" StringLit
ExportDecl := "export" (FunctionDecl | ModelDecl | WorkflowDecl | AuthDecl)
```

Routes are intentionally excluded from the MVP export list. A route is part of
the HTTP/runtime surface and is identified by method/path, not by a normal
symbol name. Route contribution across modules should be designed after named
imports are stable, with duplicate route signature checks across the module
graph.

## AST Shape

Add an import declaration and preserve existing declaration variants as much
as possible:

```rust
pub struct ImportDecl {
    pub name: String,
    pub alias: Option<String>,
    pub source: String,
    pub span: Span,
    pub name_span: Span,
    pub alias_span: Option<Span>,
    pub source_span: Span,
}

pub enum Decl {
    Import { import: ImportDecl },
    Export {
        decl: Box<Decl>,
        export_span: Span,
        span: Span,
    },
    // existing declaration variants...
}
```

The export wrapper keeps existing named declaration payloads intact. Consumers
should use small helpers such as `Decl::as_exported()` or
`Decl::exported_inner()` so the checker, formatter, linter, interpreter, and
HIR lowerer do not each invent their own unwrapping rules.

Rules:

- `export import ...`, `export invoice ...`, `export route ...`, and
  `export` before top-level statements should produce parser diagnostics with
  the `export` span.
- Imports should be allowed only at the top level.
- Imported module top-level statements should later produce checker/module
  diagnostics unless the file is the entry module.
- Preserve separate spans for the imported name, alias, source path, and
  whole import declaration.

## HIR Shape

Extend current HIR without turning it into a multi-module graph immediately:

```rust
pub enum HirDeclKind {
    Import,
    // existing declaration kinds...
}

pub enum HirSymbolKind {
    ImportedSymbol,
    // existing symbol kinds...
}

pub enum HirReferenceKind {
    ModulePath,
    ImportSymbol,
    // existing reference kinds...
}

pub enum HirDeclBody<'a> {
    Import {
        module: HirRefId,
        imported: HirRefId,
        alias: HirSymbolId,
    },
    // existing declaration bodies...
}
```

Lowering should create:

- `ModulePath` reference for the source string literal.
- `ImportSymbol` reference for the imported exported name.
- `ImportedSymbol` symbol for the local alias binding. If no alias exists, the
  alias symbol name is the imported name.
- `HirVisibility` on named declarations or export metadata on `HirDecl` so
  module export collection does not need to inspect AST wrappers forever.

Cross-module links should not store a raw `HirSymbolId` from another
`HirProgram`. Once multiple files are loaded, introduce a global wrapper:

```rust
pub struct HirModuleId(usize);

pub struct HirSymbolRef {
    pub module: HirModuleId,
    pub symbol: HirSymbolId,
}
```

The local `HirRefId -> HirSymbolId` metadata can still link an import reference
to its local `ImportedSymbol` alias. The actual definition/use link to the
exported declaration should live in module graph metadata as
`HirRefId -> HirSymbolRef` or `ImportedSymbol -> HirSymbolRef`.

## Resolver Rules

The first real module resolver should sit above the current per-file resolver:

1. Load the entry source.
2. Parse top-level imports and recursively load relative `.nx` modules.
3. Canonicalize module paths against the importing file.
4. Reject paths that leave the package root when a `nexus.toml` root exists.
5. Reject import cycles with a diagnostic that points at the import path span.
6. Lower each file independently into HIR.
7. Collect exported named declarations from each module.
8. Resolve each import against the target module export table.
9. Reject missing exports, duplicate local import aliases, and collisions with
   local top-level declarations.
10. Reject duplicate `fn`, `model`, `workflow`, or `auth` names with the same
    kind across the loaded graph. This mirrors the current merged-program
    checker surface and prevents accidental ambiguity before namespaces exist.
11. Check each module with its local declarations plus the resolved import
    overlay.

The existing `checker/resolver.rs` can stay per-module for lexical and local
top-level indexing. A new module-level layer should own imported/exported
symbol tables and only feed the existing checker once an import is validated.

## Package And Visibility Rules

Visibility for the MVP has two states:

- Private: the default for named declarations. Visible inside the same module.
- Exported: visible to other modules through explicit imports.

- Relative imports, stdlib imports, and direct local path dependency imports
  are supported module sources. Package imports use the nearest
  `nexus.toml [dependencies]` entry when the dependency source is `path:`.
- `nexus.toml [package].entry` identifies the public package entry module for
  `import Name from "package_name"`.
- `import Name from "package_name/submodule"` can import exported symbols from
  a `.nx` submodule inside a path dependency.
- Registry dependencies remain declaration-only and are intentionally not
  source roots until a registry protocol, trust model, and lockfile integrity
  story exist.
- Package-internal visibility is deferred. There is no `pub(crate)` equivalent
  in the first design.

## Checker And Metadata Integration

Checker behavior should remain validation-ordered:

- Parser/AST keeps source spans and syntax shape.
- Module resolver validates module paths and export availability.
- HIR lowering creates stable module/import references.
- Typed-HIR metadata links references only after validation succeeds.
- AST fallbacks remain available for single-file APIs and diagnostics.

The centralized writer in `checker/typed_hir_pass.rs` remains sufficient for
the first import/export skeleton. Revisit a standalone HIR-walking metadata
pass when the module graph starts producing cross-module `HirSymbolRef`
metadata or when import validation needs a deterministic whole-graph walk.

The Phase 11.17 `SourceDatabase` is intentionally minimal and additive:

- It mirrors `ModuleGraph` IDs and stores canonical module paths plus original
  source text.
- It records import edges with source module, resolved target module when
  known, imported name, alias, source path, and import/name/alias/path spans.
- It records minimal declaration source ranges for the merged program, linking
  each `decl_index` back to a module ID plus start/end line-column span.
- It exposes helpers to attach an existing `Diagnostic` to a module ID or path
  as `ModuleDiagnostic`.
- It can map checker diagnostics from the merged `Program` back to the owning
  module using declaration/module metadata and derived source ranges, giving
  CLI and tooling a path-aware diagnostic without changing checker semantics.
- Checker diagnostics can carry an explicit declaration/module owner, allowing
  the source database to use ranges for display rather than as the primary
  ownership heuristic.
- Public tooling APIs can now return structured multi-module diagnostics with
  path, owner, module ID, and source range while preserving legacy
  string-returning wrappers.
- It does not change import semantics, checker merging, namespace policy, or
  registry behavior.

## Diagnostics To Preserve

The first implementation should have focused diagnostics for:

- invalid import syntax;
- non-string module path;
- non-relative path in the local MVP;
- missing module file;
- module path escaping the package root;
- import cycle;
- missing exported symbol;
- duplicate import alias in one module;
- import alias colliding with a local top-level declaration;
- duplicate `fn`/`model`/`workflow`/`auth` names with the same kind across the
  loaded module graph;
- `export` used with unsupported declaration kinds;
- top-level statements inside imported modules.

Each diagnostic should point to the smallest stable span: name, alias, path,
or export keyword. Multi-module tooling should use `SourceDatabase` module IDs
and paths to display those diagnostics per source file.

Phase 11.18 uses this rule for checker diagnostics produced after the merged
program is built. Phase 11.19 improves that path with minimal end spans stored
as `SourceRange`/`SourceDeclRange` in the `SourceDatabase`. Because NexusLang
still has line/column spans rather than byte ranges, those ranges are inferred
from the source text and are intentionally not an LSP-grade source map yet.

## Implementation Slices

Phase 11.02 should implement only the parser/AST/HIR skeleton:

- Parse contextual `import Name [as Alias] from "path"`.
- Parse contextual `export` wrappers for function, model, workflow, and auth.
- Lower imports to `HirDeclBody::Import`.
- Add `ModulePath`, `ImportSymbol`, and `ImportedSymbol`.
- Keep single-file checking behavior stable; imports can produce a clear
  semantic diagnostic until the module graph exists.
- Add focused lexer/parser/HIR tests for syntax, spans, and lowering.

Phase 11.03 should add the local module loader and relative path graph.

Phase 11.04 should resolve exported/imported symbols and produce typed-HIR
definition/use metadata with `HirSymbolRef` rather than cross-program raw IDs.

Phase 11.05 through 11.15 integrated cross-module HIR resolution, runtime
aliases, stdlib imports, CLI/server graph loading, `nexus.toml` entrypoints,
and local `path:` package imports.

Phase 11.16 defines the current duplicate-name contract: the MVP keeps a flat
symbol surface per declaration kind across the loaded graph and rejects
duplicate import aliases or aliases that collide with local top-level names.

Phase 11.17 adds a minimal `SourceDatabase` for the module graph, source text,
import edges, and diagnostics-by-module helpers. It is a tooling foundation and
does not change import resolution semantics.

Phase 11.18 connects that database to checker diagnostics and the CLI: imported
module semantic errors can now render with their source path while preserving
the legacy string-returning wrappers and import behavior.

Phase 11.19 adds minimal source ranges/end spans to the `SourceDatabase`.
Diagnostics now prefer declaration ranges before falling back to the older
line-count heuristic, and `ModuleDiagnostic` can carry the matched source range
for tooling. This reconciles the module tooling track with the current stdlib
modules without changing stdlib import semantics.

Phase 11.20 moves checker diagnostics from range ownership to explicit
ownership: `DiagnosticOwner` records the merged declaration index and graph
module ID when available. `SourceDatabase` now consumes that owner first and
uses declaration ranges for path/range rendering, with the F11.19 range
heuristic kept only as fallback for ownerless diagnostics.

Phase 11.21 exposes that shape as public API. `CheckedMultiModuleProgram`
returns the merged program, graph, declaration-module map, and source database,
while `MultiModuleDiagnostic` carries the structured diagnostic payload for
tooling and CLI code. Existing `String` wrappers remain available and now sit
on top of the structured path.

Phase 11.22 adds `nexus check --json`, a CLI formatter for
`MultiModuleDiagnostic`. Text output remains the default; JSON mode emits
success or the first structured diagnostic on stdout and keeps import semantics
unchanged.

Phase 11.23 stabilizes that formatter as JSON schema version 1. The public
formatter now lives in the crate, the CLI includes `schema_version` and
`command`, and tests cover module-loader, checker, and runtime diagnostic
variants. At that point, `nexus check --json` was still the only JSON-emitting
CLI path; runtime diagnostics were covered through the structured Rust API
because `check` does not execute programs.

Phase 11.24 adds `nexus run --json`. Textual `run` still prints program output
directly, while JSON mode uses captured execution so the CLI emits exactly one
JSON envelope containing `output`. Runtime errors also include partial captured
output plus the v1 `MultiModuleDiagnostic` payload.

Phase 11.25 adds optional diagnostic `code` and `severity` metadata to the
shared structured diagnostic type and to the JSON v1 payload. The fields are
additive tooling metadata: text rendering, legacy `String` wrappers, and import
semantics remain unchanged.

Phase 11.26 replaces the stage-only code defaults at tooling boundaries with a
granular v1 diagnostic-code catalog for lexer, parser, module-loader, checker,
and runtime error families. The JSON shape, severities, text rendering, legacy
`String` wrappers, and import semantics remain unchanged.

Phase 11.27 adds optional diagnostic labels, notes, and suggestions to the
shared structured diagnostic type and JSON v1 payload. The fields are additive:
messages, codes, severities, text rendering, legacy `String` wrappers, and
import semantics remain unchanged.

Phase 11.28 starts populating those metadata fields in the highest-impact
producers: parser import/export syntax, checker type diagnostics,
module-loader symbol-not-exported diagnostics, and runtime division/modulo by
zero. This still does not change import semantics, text rendering, legacy
`String` wrappers, or the JSON v1 shape.

Phase 11.29 expands the populated metadata to additional high-impact families:
checker symbol/argument/model/route/auth/workflow/invoice diagnostics,
module-loader duplicate-symbol, duplicate-alias, alias-collision, path,
package, and stdlib diagnostics, and runtime undefined variable/function,
model, and workflow diagnostics. The added metadata stays inside the existing
JSON v1 `labels`, `notes`, and `suggestions` arrays and does not add LSP,
remote registry, full byte ranges, or import-semantic changes.

Phase 11.30 introduces a minimal `MultiModuleDiagnosticReport` for tooling.
The report can hold multiple `MultiModuleDiagnostic` values and groups them by
owning path/module ID. It was introduced as an additive collection envelope
beside `multi_module_diagnostic_json`, so CLI `--json` could keep the existing
first-error shape while `multi_module_diagnostic_report_json` exposed the
collection shape for future tools.

Phase 11.31 exposes that report shape through the opt-in
`nexus check --json-report` CLI mode. This command uses the same first-error
compiler flow as the report API today, but returns the collection envelope with
`diagnostic`, `diagnostics`, and `groups`. Existing `nexus check --json` and
`nexus run --json` outputs remain first-error compatible.

Phase 11.32 extends the same opt-in report shape to `nexus run --json-report`.
The run report adds captured `output` to the report envelope, preserving full
success output and runtime partial output. Loader/checker failures still report
`output: []`, and plain `nexus run --json` keeps its previous first-error
shape.

Phase 11.33 starts feeding real checker collections into the opt-in report
paths. After module loading, import resolution, and checker symbol collection
succeed, `nexus check --json-report` and `nexus run --json-report` can collect
diagnostics from independent declaration bodies in the checker. Parser
recovery, loader graph failures, top-level statement cascades, runtime
multi-error reporting, and regular `--json` first-error behavior remain
unchanged.

Phase 11.34 stabilizes that checker-report contract with a coverage matrix for
function, route, workflow, and invoice declaration-body diagnostics. It also
locks in the conservative boundaries: global checker setup failures stay as a
one-item report, and top-level statements remain first-error because they share
order-dependent scope state.

Phase 11.35 adds public in-memory tooling helpers on
`MultiModuleDiagnosticReport`. Tools can query diagnostics by path, module ID,
path/module pair, stage, severity, and report group, including the first
diagnostic in each group. These helpers do not change JSON v1 or any CLI
first-error output.

Phase 11.36 adds a public in-memory summary API on
`MultiModuleDiagnosticReport`. `summary()` returns total/flag fields, counts by
stage and optional severity, and unique affected paths/module IDs for Rust
tooling. The summary is not serialized and does not change the JSON v1 report
or first-error CLI output.

Phase 11.37 adds fixture-backed Rust consumption examples for the report API.
The core tests exercise checker, module-loader, and runtime reports through
filters, groups, `summary()`, legacy `String` wrappers, and JSON v1 stability.
`examples/diagnostic_report_tooling.rs` is a compilable Cargo example for
tooling consumers.

Phase 11.38 adds a public in-memory flattened tooling view on
`MultiModuleDiagnosticReport`. `tooling_view()` returns the summary, groups,
and one item per diagnostic with diagnostic/group indexes, path, module ID,
stage, severity, code, message, line/column, and source range when available.
`tooling_items()` returns just the flattened item list. These APIs do not
change JSON v1, CLI text, wrappers, imports, or collection semantics.

Phase 11.39 adds opt-in in-memory source context for flattened tooling items.
`tooling_view_with_source_context(Some(&source_database))` and
`tooling_items_with_source_context(...)` can attach module path/ID, source line
text, existing line/column/range, and highlight columns when `SourceDatabase`
can resolve the item. Passing `None` or diagnostics without source owners keeps
the context absent. JSON v1 and CLI behavior are unchanged.

Phase 11.40 consolidates the public pre-LSP tooling diagnostics contract as a
documented stability matrix. The contract covers `MultiModuleDiagnosticReport`
collection helpers, filter/group APIs, `summary()`, flattened tooling views,
source-context views, ordering guarantees, JSON v1 boundaries, and explicit
non-goals such as LSP document URIs, byte ranges, parser recovery, registry
metadata, and import semantic changes. Core tests now exercise that matrix in a
single fixture-backed scenario so the in-memory APIs can keep evolving
additively without widening first-error JSON or report JSON v1 unexpectedly.
