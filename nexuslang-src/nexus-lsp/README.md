# NexusLang LSP

`nexus-lsp` is the first editor-facing adapter for NexusLang.

It is intentionally a separate crate from the core compiler/runtime. The LSP
server depends on `nexuslang`, keeps editor protocol code out of the core, and
reuses the structured diagnostics produced by the lexer, parser, and checker.

## Run

From `nexuslang-src`:

```bash
cargo run -p nexus-lsp
```

The server speaks Language Server Protocol over stdio and is meant to be
launched by an editor client.

## Internal Shape

- `src/lib.rs` owns the testable LSP core: `DocumentSnapshot`, `LspCore`,
  diagnostic conversion, multi-file diagnostic publication batches, hover,
  completion, semantic tokens, document symbols, and definition helpers.
- `src/main.rs` is a thin `tower-lsp` adapter that stores snapshots and
  publishes responses without embedding compiler/editor logic in the transport
  layer.
- The snapshot API records document URI, version, and source text. When the
  entry snapshot maps to a local file and matches disk, `LspCore` can load the
  module graph through `SourceDatabase` and publish diagnostics for every
  loaded module without changing the compiler core or the JSON v1 contract.
- The same disk-backed bridge is used for cross-file go-to-definition on
  imports and aliases: `SourceDatabase` supplies import edges and `ModuleGraph`
  confirms exported names before the LSP returns a target-module location.
- `LspCore` remembers the last diagnostic publication group per entry URI. If
  an import graph changes, a dirty snapshot falls back to single-document
  diagnostics, or a document is closed, the core returns empty batches for stale
  URIs while keeping diagnostics that are still covered by another active entry.
- Semantic tokens are lexical and full-document for now. The legend is
  `keyword`, `type`, `string`, `number`, `variable`, and `erpSymbol`; the core
  reuses the existing lexer token stream.
- Document symbols are document-local and AST-backed. The MVP returns nested
  symbols for declarations and ERP children that already have spans, separates
  enclosing declaration ranges from name selection ranges, and returns an empty
  nested list while a document is partially invalid.

## Current Capabilities

- Full-document sync.
- Publish diagnostics on open/change. Clean disk-matching entry snapshots use
  the module loader and checker report APIs to publish multi-file diagnostics;
  dirty snapshots fall back to single-document diagnostics and clear stale
  imported-module diagnostics from earlier publications.
- Publish empty diagnostics on close for the entry's previous publication group.
- Hover for keywords, identifiers, literals, types, HTTP methods, and operators.
- Completion for NexusLang keywords plus identifiers from the current document.
- Basic same-document go-to-definition for `fn`, `model`, `route`, `auth`,
  `workflow`, `let`, and import aliases.
- Cross-file go-to-definition for imported names and aliases when the entry and
  loaded open-module snapshots match disk.
- Full-document semantic tokens for keywords, types, strings, numbers,
  identifiers, and ERP symbols.
- Document symbols for functions, models, workflows, auth declarations, routes,
  invoices, imports, exports, top-level bindings, and ERP children such as model
  fields, workflow steps, route query params, and invoice fields/items.

## Current Limits

- Multi-file diagnostics are opt-in and disk-backed: unsaved entry text falls
  back to single-document diagnostics.
- In-memory document snapshots only; no persistent or incremental
  `SourceDatabase`.
- Cross-file go-to-definition currently covers imports/aliases only.
- No formatting, rename, code actions, or workspace symbols.
- LSP ranges are still line/column based and derived from AST spans; broad
  byte-precise source ranges remain future work.

## Validation

```bash
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo check -p nexus-lsp
CARGO_TARGET_DIR=/tmp/nexuslang-target-codex cargo test -p nexus-lsp
```
