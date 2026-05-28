# Typed HIR Architecture Status

Last updated: 2026-05-27

This note closes the Phase 10 typed-HIR definition/use link track. It records
what is now stable, what intentionally remains as AST compatibility fallback,
and when the current metadata writer should become a standalone HIR-walking
pass.

## Current Shape

- `hir.rs` lowers checked AST into stable `HirDeclId`, `HirSymbolId`,
  `HirExprId`, `HirRefId`, and `HirScopeId` values.
- `checker/typed_hir_pass.rs` owns all writes to `HirCheckedMetadata`.
- `checker/hir_metadata.rs` is the read/query/cache layer for checked
  expression, symbol, and reference metadata.
- Checker submodules validate public AST contracts first, then attach and
  consume typed-HIR metadata where stable IDs already exist.

## Complete Link And Consume Paths

- Lexical bindings, route/query params, field access, function calls and model
  static calls produce expression type/symbol metadata through the typed-HIR
  expression checker.
- Object literal field keys use `HirReferenceKind::ObjectField` and consume
  `HirRefId -> ModelField` metadata during HIR object validation.
- Model operation field-name arguments link string-literal `HirExprId`s to
  `ModelField` symbols and consume symbol/type metadata in lookup, advanced
  filter, range, composite, and ordering validation.
- Declarative auth references link and consume:
  - `AuthModel -> Model`
  - `AuthIdentityField -> ModelField`
  - `AuthRoleField -> ModelField`
- Route auth guards link and consume `RouteAuthGuard -> Auth`.
- Static `run_workflow("Name")` literals link and consume
  `HirExprId -> Workflow`.

## Intentional AST Fallbacks

These are compatibility paths, not current blockers:

- `checker/expr.rs` remains the AST fallback for callers without a reliable
  `HirExprId` context and for public diagnostic preservation.
- Model declarations still validate static defaults, min, and max through the
  existing AST path because those checks are literal-focused and diagnostics are
  already stable.
- Route expression wrappers keep AST-facing shape checks before delegating into
  HIR inference, so HTTP route diagnostics stay unchanged.
- Legacy symbol maps in `CheckerSymbols` remain compatibility indexes while the
  checker still accepts AST entrypoints.

## Remaining Gaps

- There is no module/import/export syntax yet, so module references cannot be
  linked until the language surface exists.
- Future module work should add stable HIR references for imported modules,
  imported symbols, qualified names, and package-level symbol visibility.
- Standard library symbols are still ordinary built-ins or checker branches,
  not HIR-indexed declarations.
- Some AST fallback paths can be retired later, but only after every public
  checker entrypoint has a reliable HIR context and diagnostics are locked.

## Writer Decision

Keep the centralized writer in `checker/typed_hir_pass.rs` for now.

Rationale:

- Metadata writes are still local and validation-ordered.
- Each current writer has nearby semantic context needed for diagnostics.
- The owner API already prevents scattered direct mutation of
  `HirCheckedMetadata`.
- A standalone pass would mostly duplicate checker knowledge today.

Revisit a standalone HIR-walking metadata pass when one of these becomes true:

- module/import/export references exist;
- cross-declaration metadata needs a deterministic whole-HIR traversal;
- metadata production starts spreading across many unrelated checker modules;
- AST compatibility fallbacks are ready to be removed from public entrypoints.

## Recommended Next Track

The Phase 10 typed-HIR link/consume track can be considered architecturally
closed for the current language surface. The next high-leverage compiler track
is module planning: syntax, AST/HIR representation, resolver rules, and
definition/use links for module and imported-symbol references.

Phase 11.01 records that module/import/export plan in
`MODULES_IMPORTS_ARCHITECTURE.md`. The important typed-HIR constraint is that
raw `HirSymbolId` values stay local to one `HirProgram`; cross-module
definition/use links should use a module-qualified symbol reference once the
module graph exists.
