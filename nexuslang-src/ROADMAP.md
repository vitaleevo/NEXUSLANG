# NexusLang Development Roadmap

NexusLang is an ERP-first programming language. The current core is a Rust
lexer, parser, semantic checker, and interpreter, plus a standalone HTML
playground.

## Current baseline

- `nexus run [file.nx]` parses, checks, and executes a program; when no file
  is passed, it uses `[package].entry` from the nearest `nexus.toml`.
  `nexus run --json` captures program stdout and emits the versioned v1 JSON
  diagnostic/output contract.
- `nexus check [file.nx]` validates a program without executing it; when no
  file is passed, it uses `[package].entry` from the nearest `nexus.toml`.
  `nexus check --json` emits the versioned v1 structured multi-module
  diagnostic payload.
- `nexus tokens <file.nx>` prints lexer output.
- `nexus ast <file.nx>` prints parser output.
- `nexus docs [file.nx] [--output docs.md]` validates a single or
  multi-module program and emits Markdown documentation for ERP declarations.
- `nexus test [file-or-directory]` runs local `.nx` smoke tests or examples
  through the graph-aware loader; with no explicit target it prefers `tests/`
  and falls back to `examples/`. Optional `.out` sidecars compare captured
  stdout for simple regression tests, and optional `.err` sidecars validate
  expected diagnostics so intentional errors can pass when the diagnostic text
  matches. `--update` refreshes `.out` sidecars only for successful executions.
  `--update-err` refreshes `.err` sidecars from the current diagnostic only
  when a program fails. `--name <term>` filters discovered tests by path/name
  substring and composes with `--update` and `--update-err`. `--json` emits a
  machine-readable report with summary counts, per-case status/output,
  diagnostics, `.out` mismatches, expected diagnostics, diagnostic mismatches,
  and updated sidecar paths; `.out`/`.err` mismatches include compact first
  divergent line metadata. Human output blocks truncate after 20 lines while
  JSON keeps complete arrays. `--timeout <dur>`
  fails individual cases that do not finish within durations such as `500ms`,
  `5s`, or `1m`. `--isolate-data` gives each case a temporary
  `NEXUS_DATA_DIR` so runtime storage does not use the workspace `.nexus-data`.
  `--jobs <n>` runs cases with bounded parallelism while preserving
  deterministic report order. `--list` prints the deterministic
  discovered/filtered case list without executing programs or updating
  sidecars, and composes with `--json`. `--fail-fast` stops after the first
  failing case in sequential mode and after the first failing batch when
  `--jobs` is active. Test files can use native
  `assert_true(...)` and `assert_eq(actual, expected)`,
  `assert_ne(actual, expected)`, and `assert_contains(container, item)` helpers
  for behavioral assertions that fail the case with structured runtime
  diagnostics; assert helpers accept an optional final message string for CI
  context.
- `nexus install`, `nexus add <package>`, `nexus add <package> --path <dir>`,
  `nexus add <package> --registry <package@version>`, and `nexus update`
  provide the first package-manager MVP using `nexus.toml`, `nexus.lock`, and
  `.nexus/packages/`. Local `path:` dependencies can feed package-name imports
  in the module graph. Registry dependencies can now be installed read-only
  when `NEXUS_REGISTRY_URL` points at a filesystem, `file://`, or plain
  `http://` registry, with metadata validation, optional SHA-256 verification,
  safe tar extraction, lockfile checksum/resolved-path metadata, and
  package-name imports from the installed cache. Registry publish, auth,
  HTTPS, transitive dependencies, and semantic version solving remain out of
  scope.
- Supported ERP primitives: `model`, `workflow`, `route`, `auth`, `invoice`,
  `money`.
- Supported language primitives: functions, `let`, `const`, `if`, `while`,
  `for`, arrays, strings, numbers, booleans, and static model calls.
  Function declaration validation now lives in
  `src/checker/function_decl.rs`, covering function signature collection,
  parameter/return type validation, HIR parameter symbol setup, body checking,
  and required-return validation for non-void functions.
- Static model route operations are centralized through
  `ModelStaticOperation` descriptors in `model_ops.rs`, documented in
  `MODEL_OPERATIONS.md`, normalized through `CheckedModelOperationArgs`, and
  validated by family-based checker rules in `checker/model_ops.rs` and the
  quality gate.
- Static auth route operations are centralized through `AuthStaticOperation`
  descriptors in `auth_ops.rs`, documented in `AUTH_OPERATIONS.md`, normalized
  through `CheckedAuthOperationArgs`, and consumed by checker, route HIR,
  native auth runtime, and OpenAPI. A contract matrix validates every
  `AuthStaticOperation` through checker, route HIR, OpenAPI, and HTTP,
  including structured request bodies and `400` responses for register/login.
  Checker-side auth static operation validation now lives in the dedicated
  `src/checker/auth_static_ops.rs` submodule, covering AST return inference,
  auth config lookup, and HIR static-call validation while preserving the
  public AST contracts.
  Auth declaration validation now lives in `src/checker/auth_decl.rs`, covering
  auth collection, duplicate config checks, `AuthConfig` storage, HIR auth
  symbol registration, auth config model lookup, identity/role field
  validation, password minimum, and TTL relationship checks while preserving
  the static-operation module for `Auth::...` calls.
  Route static-call dispatch for model/auth operations now lives in
  `src/checker/route_static_ops.rs`, so AST and HIR route-return paths share
  the same Auth/Model decision layer while delegating operation-specific
  validation to the existing checker submodules.
  Route expression validation now lives in `src/checker/route_expr.rs`,
  covering AST and HIR route-return inference/ensuring while preserving the
  static-call dispatcher.
  Route declaration validation now lives in `src/checker/route_decl.rs`,
  covering route collection, method/path uniqueness, duplicate path/query
  parameter checks, route guards, route body shape, route/query parameter
  scope setup, query parameter type/default validation, and route HIR symbol
  registration while preserving the route expression/static-call delegation.
  Invoice declaration validation now lives in `src/checker/invoice_decl.rs`,
  covering required invoice contract checks, duplicate field detection, field
  type validation, and item description/qty/price validation while preserving
  source spans and diagnostics.
  Model declaration validation now lives in `src/checker/model_decl.rs`,
  covering model collection, duplicate/reserved-name checks, field type checks,
  model field defaults, and `unique`/`index`/`min`/`max` constraints while
  preserving `src/checker/model_ops.rs` for `Model::...` operations.
  Workflow declaration validation now lives in `src/checker/workflow_decl.rs`,
  covering workflow collection, duplicate-name checks, workflow step checking,
  and shared `run_workflow` arity/type/existence validation for AST and HIR
  call paths.
- A first general HIR exists in `src/hir.rs`, lowering checked AST programs
  into stable declaration, symbol, expression, and lexical scope IDs. It
  indexes functions, models, auth declarations, workflows, routes, local
  bindings, route/query params, model fields, invoice fields, expressions,
  non-expression references, and scope frames for top-level, functions,
  models, workflows, workflow steps, auth declarations, routes, invoices,
  blocks, and loops. The checker now uses a minimum HIR-backed resolver for
  top-level declarations, scoped lexical bindings, scope parent links, visible
  binding queries, and stable `HirRefId`s for auth declarations and route auth
  guards. HIR statements and expressions now retain their source scope,
  letting checker bindings for functions, routes, blocks, and loops resolve
  through `HirScopeId` before the compatibility fallback by declaration/span.
  `HirCheckedMetadata` records expression `TypeId`s, binding `TypeId`s for
  `HirSymbolId`s, non-expression reference links, and definition/use links.
  Typed-HIR metadata/cache helpers now live in
  `src/checker/hir_metadata.rs` as the query/cache layer, including the
  private expression context that lets checker expression validation consume
  `HirExprId`, `HirSymbolId`, and `HirTypeId` metadata before falling back to
  AST. The owner/writer entrypoint for metadata production now lives in
  `src/checker/typed_hir_pass.rs`, with a private `TypedHirMetadataStore`
  around `HirCheckedMetadata`. The store exposes read-only snapshot/query
  access to the metadata cache layer while keeping replace/write operations
  private to the owner module. That owner centralizes `HirCheckedMetadata`
  initialization, typed-HIR test counter resets, expression type/symbol writes,
  symbol type writes, and explicit expression metadata production/ensure
  helpers used by AST/HIR inference and route static-call validation. Isolated
  auth config symbol links and binding/parameter symbol type metadata now also
  go through that owner path. The main statement and route
  expression checks now pass
  through a dedicated typed-HIR expression checker module,
  `src/checker/hir_expr.rs`, over `HirExprId`/`HirExprKind` for resolved
  identifiers, assignments, field access, object literals, binary expressions,
  calls, static calls, and route return expressions. Static calls in route
  returns now also build a `HirOperationArgs` adapter so model/auth operation
  validation can associate source AST arguments with `HirExprId`s and route
  argument inference/ensuring can proceed through HIR while preserving the
  existing AST contracts. Model operation lookup, pagination, advanced filter
  (`where_compare`, `where_text`, `where_between`), and composite filter
  (`where_all`, `where_any`) validators now consume that adapter directly when
  available, with AST fallback for legacy callers. Model operation validation
  now carries `ModelOperationContext` through `checker/model_ops.rs`, so
  `HirProgram`, `HirOperationArgs`, and `Scope` travel as one internal context
  and the route static-call path no longer reinfers every normalized model
  argument in post-processing. A dedicated typed-HIR argument API now derives
  lookup, ordering, pagination, advanced, range, and composite filter arguments
  from `CheckedModelOperationArgs` through `CheckedHirModelOperationArgs`.
  Each internal argument carries its source expression for diagnostics/literal
  checks and an optional normalized `HirExprId` for inference, so model-op
  validators consume typed-HIR argument structures as the primary contract
  while preserving public AST wrappers and message compatibility. Type
  checking still keeps legacy maps for compatibility. The source+HIR argument
  wrapper is shared as `CheckedHirOperationArg`, with
  `CheckedHirModelOperationArgs` and `CheckedHirAuthOperationArgs` deriving
  typed-HIR operation-specific views from the same contract. Auth static calls
  also use the shared wrapper in the HIR route path to preserve source
  diagnostics, attach auth symbols by `HirExprId`, and count checked operation
  args through the same internal layer. Model operation field-name arguments
  now link their string-literal `HirExprId`s to referenced `ModelField`
  symbols in typed-HIR metadata, consume that symbol/type metadata as the
  HIR-first field type path for lookup, advanced filter, range, composite, and
  ordering validators, and keep legacy AST/model maps as diagnostics-preserving
  fallback. Declared model field symbols receive checked type metadata. This
  operation-argument layer now lives in
  `src/checker/hir_args.rs`, separate from `hir_expr.rs`, so expression
  validation and static-operation argument normalization can evolve with less
  coupling. Auth static operation validation now lives in
  `src/checker/auth_static_ops.rs`, auth declaration validation now lives in
  `src/checker/auth_decl.rs`, AST/HIR route static-call dispatch now lives in
  `src/checker/route_static_ops.rs`, route expression validation now lives in
  `src/checker/route_expr.rs`, route declaration validation now lives in
  `src/checker/route_decl.rs`, invoice declaration validation now lives in
  `src/checker/invoice_decl.rs`, and model declaration validation now lives in
  `src/checker/model_decl.rs`. Workflow declaration validation and the shared
  `run_workflow` helper now live in `src/checker/workflow_decl.rs`; static
  `run_workflow("Name")` literals now link their `HirExprId` to the referenced
  `Workflow` symbol and consume that symbol metadata in the typed-HIR path. Route
  declaration collection/default validation now also lives in
  `src/checker/route_decl.rs`. Function declaration signature/body checking
  now lives in `src/checker/function_decl.rs`. Auth declaration collection now
  also lives in `src/checker/auth_decl.rs`. Top-level statement declaration
  checking now delegates through `src/checker/statement_decl.rs`. Shared
  statement and binding checking now lives in `src/checker/stmt.rs`, covering
  `check_stmts`, `check_stmt`, binding annotations, assignments, returns,
  control-flow statement checking, HIR scope switching, and checked binding
  metadata. General AST expression inference and compatibility fallback now
  lives in `src/checker/expr.rs`, covering `infer_expr`, object literal field
  validation, field access, unary/binary fallback, function calls, simple
  `Model::all()` static-call fallback, and shared optional/comparable/numeric
  helpers consumed by `checker/hir_expr.rs`. The typed-HIR expression checker
  remains the preferred HIR path. Shared type/operator/filter helpers now live
  in `src/checker/type_rules.rs`, covering HIR-to-AST binary operator
  compatibility, ordering support, comparison operator support, comparison
  field-type checks, and text filter support used by `checker/model_ops.rs`
  and `checker/hir_expr.rs`. HIR symbol indexing now lives in
  `src/checker/symbols.rs`, covering `CheckerSymbols`, top-level symbol maps,
  expression IDs, statement scopes, statement binding scopes, and model-field
  symbol indexes consumed by checker metadata and typed-HIR paths. The local
  `Scope` type and its binding helpers now live in
  `src/checker/scope.rs`, covering local vars, const tracking, HIR symbols,
  current `HirScopeId`, assignment validation, and name resolution used by
  statement, route, model, function, workflow, and typed-HIR argument paths.
  Typed-HIR metadata/cache helpers now live in
  `src/checker/hir_metadata.rs`, covering checked metadata snapshots,
  expression context lookup, cache-hit counters, typed expression/symbol
  lookup, and checked symbol binding types through read-only owner-store
  access. Typed-HIR metadata initialization and writes now live in
  `src/checker/typed_hir_pass.rs`, covering expression type/symbol writes,
  explicit expression metadata production, route metadata completion, auth
  config symbol links, and checked symbol type writes through the hardened
  owner/writer API.
  Base type helpers now live in `src/checker/type_core.rs`, covering
  `ensure_assignable` and `type_name` while preserving assignment diagnostics,
  `Optional`/`Array` compatibility, and the existing internal `super::...`
  imports across checker submodules. Binding resolution helpers now live in
  `src/checker/binding_resolution.rs`, covering scoped/legacy binding lookup,
  assignment through typed-HIR metadata, `HirScopeId`-first symbol resolution,
  and the scoped binding cache-hit counter. Program-flow orchestration now
  lives in `src/checker/program_flow.rs`, covering declaration collection,
  declaration checking, known-type validation, and static-default validation.
  Symbol lookup helpers now live in `src/checker/symbol_lookup.rs`, and the
  typed-HIR/AST expression fallback bridge now lives with the AST expression
  fallback in `src/checker/expr.rs`. `src/checker/mod.rs` is now a small
  checker shell; the owner-pass trail for typed-HIR metadata is hardened as a
  centralized writer. Declarative auth references now link and consume auth
  model, identity field, and role field uses through their target HIR symbols;
  route auth guards now link and consume their auth config use through the
  target auth symbol. Object
  literal field keys now carry stable `HirReference`s and link
  `Customer { name: ... }` slots to their target `ModelField` symbols in
  typed-HIR metadata. Object literal validation now consumes the HIR model
  field list plus checked reference/symbol metadata as its primary typed-HIR
  path while preserving the AST fallback and public diagnostics. Current
  architecture judgement: the centralized writer in `typed_hir_pass.rs` remains
  sufficient for the near term; a standalone HIR-walking production pass is
  deferred until module references or broader cross-declaration metadata make
  the incremental writer visibly too scattered.

## Typed-HIR architecture closure

Phase 10.08 closed the current typed-HIR definition/use link track for the
language surface that exists today. The detailed status lives in
`TYPED_HIR_ARCHITECTURE.md`.

Current closure judgement:

- Link+consume paths are complete for the ERP-central references implemented in
  Phases 10.01-10.07: object literal field keys, model operation field-name
  args, declarative auth refs, route auth guards, and static
  `run_workflow("Name")` literals.
- Remaining AST fallback paths are intentional compatibility and diagnostic
  preservation paths, not blockers for the current surface.
- Module/import/export references are the next real definition/use gap because
  the syntax and HIR surface do not exist yet.
- The centralized writer in `checker/typed_hir_pass.rs` remains the right
  architecture for now. Revisit a standalone HIR-walking metadata pass when
  module references or broader cross-declaration metadata make incremental
  writer calls too scattered.

## Module/import/export architecture

Phase 11.01 defines the smallest safe module track in
`MODULES_IMPORTS_ARCHITECTURE.md`.

Current module plan:

- Treat each `.nx` source file as an implicit module; do not add a `module`
  keyword in the first slice.
- Use contextual `import`, `export`, `from`, and `as` keywords to avoid
  globally reserving existing identifiers.
- Start with one imported symbol per declaration:
  `import Customer [as Client] from "./crm.nx"`.
- Start exports as an explicit wrapper around named declarations:
  `export model`, `export fn`, `export workflow`, and `export auth`.
- Represent imports in HIR with stable `ModulePath` and `ImportSymbol`
  references plus a local `ImportedSymbol` alias.
- Do not link cross-module definitions with a raw `HirSymbolId`; introduce a
  module-qualified symbol reference when the module graph exists.
- Keep the package manager local-first: `nexus.toml` entrypoints and `path:`
  dependencies feed the module graph, while registry downloads stay deferred.
- Defer wildcard imports, namespace imports, re-exports, registry-backed
  imports, route exports, invoice exports, and module-qualified names.

Current implementation status:

- Parser/AST/HIR support exists for `import Name [as Alias] from "path"` and
  `export` on named declarations.
- The module loader builds a deterministic `ModuleGraph`, resolves relative
  local modules, detects cycles, validates exported symbols, and supports
  `std/<module>` imports through stdlib discovery.
- The graph-aware loader also resolves package-name imports for direct local
  `path:` dependencies in the nearest `nexus.toml`, including imports to the
  dependency entrypoint (`"crm_core"`) and submodules (`"crm_core/models"`).
- The module graph now enforces the MVP symbol-surface contract: duplicate
  import aliases in one module, import aliases that collide with local
  top-level names, and duplicate `fn`/`model`/`workflow`/`auth` names with the
  same kind across loaded modules are rejected with module-loader diagnostics.
- A minimal `SourceDatabase` now mirrors `ModuleGraph` IDs, stores canonical
  paths and source text per module, records import edges and declaration source
  ranges, and can attach diagnostics to module IDs or paths for future tooling.
- The stdlib now ships as real `.nx` modules for `std/math`, `std/string`,
  `std/collections`, `std/validation`, `std/date`, `std/money`, `std/number`,
  `std/inventory`, `std/crm`, `std/invoice`, `std/json`, `std/csv`,
  `std/http`, `std/crypto`, `std/time`, `std/env`, `std/log`, and `std/path`,
  plus business modules `std/sales`, `std/tax`, `std/discount`,
  `std/payment`, `std/banking`, `std/accounting`, `std/ledger`,
  `std/shipping`, `std/warehouse`, `std/procurement`, `std/supplier`,
  `std/customer`, `std/project`, `std/task`, `std/kpi`, `std/report`,
  `std/pagination`, `std/security`, `std/config`, and `std/commerce`.
  Rust-backed internal builtins are used only where the language does not yet
  have string/array/date/money/data/crypto/runtime/path primitives; the
  business batch is implemented in NexusLang puro.
- Checker diagnostics from graph-loaded programs now carry explicit
  declaration/module owner metadata where available, then map back to the
  owning source module/range for display. Public structured diagnostic APIs
  expose path, owner and source range for tooling while legacy `String`
  wrappers remain available. `nexus check`/`nexus run` render imported-module
  checker errors with a path, and `nexus check --json` / `nexus run --json`
  expose a versioned JSON contract for tooling. JSON run mode captures stdout
  into an `output` array and preserves partial output on runtime diagnostics.
  Structured diagnostics now include optional `code`, `severity`, `labels`,
  `notes`, and `suggestions` metadata, with a granular v1 code catalog for
  lexer, parser, module-loader, checker, and runtime error families. The first
  high-impact producers now populate those fields for parser import/export;
  checker type, symbol, argument, model, route, auth, workflow, and invoice;
  module-loader missing exports, duplicate names/aliases, alias collisions,
  path/package/stdlib resolution; and runtime division-by-zero plus undefined
  variable/function/model/workflow diagnostics, without changing textual
  messages. Tooling also has a minimal `MultiModuleDiagnosticReport` API and
  report JSON formatter that group diagnostics by path/module while preserving
  the existing first-error CLI JSON contract. The report is exposed through the
  opt-in `nexus check --json-report` and `nexus run --json-report` modes; the
  run report includes captured `output`, and these report modes now collect
  checker diagnostics from independent declaration bodies when the global
  checker setup succeeds. This checker-report collection is covered for
  function, route, workflow, and invoice declaration bodies, while global setup
  failures and top-level statements remain first-error. The report API also has
  in-memory tooling helpers for querying diagnostics by path/module, stage,
  severity, and group, plus an in-memory summary with counts by stage/severity,
  affected paths/modules, and flags such as `has_errors`. A flattened
  in-memory tooling view now exposes one item per diagnostic with path,
  module_id, stage, severity, code, message, source range when present, and
  group index. An opt-in source-context view can enrich those items from
  `SourceDatabase` with source line snippets and highlight columns when
  available. The public pre-LSP tooling surfaces now have an explicit contract
  matrix covering report helpers, summary, flattened view, source-context view,
  ordering, JSON boundaries, and unsupported byte-range concepts. A first
  separate `nexus-lsp` crate now uses `tower-lsp` over stdio for diagnostics,
  hover, completion, and same-document go-to-definition while keeping protocol
  code outside the core crate. The LSP transport adapter is now thin:
  `nexus-lsp/src/lib.rs` owns the testable `DocumentSnapshot`/`LspCore` layer
  and `src/main.rs` only bridges LSP lifecycle events to that core. Clean
  disk-matching entry snapshots can now opt into `SourceDatabase`-backed
  multi-file diagnostic publication batches, including clearing diagnostics for
  imported modules when the loaded project checks cleanly; dirty unsaved
  snapshots keep the single-document fallback and now clear stale diagnostics
  from any modules that were only known by the previous publication group.
  Close events also publish empty batches for the closed entry's previous group,
  while modules still covered by another active entry are preserved. The same
  disk-backed bridge now resolves go-to-definition for imported names and
  aliases through `SourceDatabase` import edges plus `ModuleGraph` export
  metadata, with dirty entry/imported snapshots falling back to same-document
  navigation. The adapter also exposes full-document semantic tokens generated
  from the existing lexer token stream with a small legend for keywords, types,
  strings, numbers, identifiers, and ERP symbols. It also exposes a
  document-local symbols MVP backed by the parser/AST for declarations, model
  fields, workflow steps, route query params, invoice fields, and invoice items,
  returning an empty symbol list while a document is partially invalid.
  Fixture-backed Rust examples cover checker, module-loader, and runtime
  report consumption, and `examples/diagnostic_report_tooling.rs` compiles as a
  minimal consumer. `nexus check --json` and `nexus run --json` keep their
  previous first-error shapes.
- Cross-module HIR import resolutions now use the imported module path and
  source module, not only the exported symbol name.
- `nexus check`, `nexus run`, and `nexus serve` use the graph-aware loader for
  file entrypoints and manifest entrypoints, so the multi-module ERP example,
  initial stdlib, and local path dependencies work through the user-facing CLI.
- Remaining module limits: no wildcard imports, namespace imports, re-exports,
  route/invoice exports, module-qualified names, registry-backed packages,
  transitive package solving, persistent caches, broad cross-file LSP navigation
  beyond imports/aliases, workspace symbols, or LSP-grade incremental source
  database yet.

## Post-0.1 release line

Current judgement: `v0.1.1` is published and publicly install-validated for
evaluation, demos, and QA, with public artifact checksum/signature verification
in place. The next work should reduce real user friction and narrow
compatibility risk before adding broad new language surface.

Current source line: post-`v0.2.0` stable development, with the read-only
registry MVP merged on `main` after PR #5.

### 0.1.1 maintenance focus

- Validate the published GitHub Release install path from a clean temporary
  directory. DONE
- Keep `scripts/validate-public-release-install.sh` as the repeatable
  post-release check for archive download, fingerprint, GPG signatures,
  checksum, extraction, packaged smoke, and playground asset smoke.
- Polish public install docs so they start from GitHub Release assets, not only
  locally built `dist/` artifacts.
- Make Linux/WSL package expectations explicit until cross-platform installers
  exist.
- Define the JSON/SQLite storage compatibility policy more concretely,
  especially backup, migration, and schema-change expectations. DONE in
  `COMPATIBILITY.md`, validated by
  `scripts/validate-storage-compatibility-policy.sh`,
  `storage_schema_evolution_allows_additive_optional_and_defaulted_fields`,
  and `sqlite_storage_matches_json_storage_for_crud_and_critical_filters`.
- Add one or two small public examples that show realistic inventory/CRM
  workflows without depending on unstable storage guarantees. STARTED with
  `storage_backup_restore_inventory.nx`, `STORAGE_BACKUP_RESTORE.md`, and
  `scripts/smoke-storage-backup-restore.sh`.
- Add a local package-manager MVP for project manifests and lockfiles. DONE
  with `nexus.toml`, `nexus.lock`, `nexus install`, `nexus add <package>`,
  `nexus update`, generated local `.nexus/packages/` cache, and CLI tests.
- Harden the package-manager MVP with local path dependencies, stronger
  `nexus.toml` validation, safe stale-cache cleanup, and an initial registry
  declaration contract. DONE.
- Add a read-only remote registry MVP for package installs. DONE in PR #5 with
  `NEXUS_REGISTRY_URL`, `nexus-package.toml`, `.tar` download/cache, optional
  SHA-256 verification, safe extraction, lockfile metadata, and package-name
  imports from the installed cache.
- Wire the graph-aware module loader into user-facing `nexus check`,
  `nexus run`, and `nexus serve`. DONE locally, including CLI regression tests
  for the real multi-module ERP example and `std/math`.
- Integrate local package manifests with compiler inputs. DONE locally:
  manifest entrypoints work for `run`/`check`/`serve`, and direct `path:`
  dependencies can be imported by package name without adding remote registry
  downloads.
- Define the MVP duplicate-name contract for the module graph. DONE locally:
  the loaded graph uses a flat symbol surface per declaration kind and rejects
  duplicate import aliases, alias/local collisions, and duplicate names across
  loaded modules before checker merge.

Real risks to retire in `0.1.1`:

- The first package is still local-platform oriented; there are no Windows or
  macOS installers yet.
- The language package manager is still MVP-level: local `path:` dependencies
  and read-only registry downloads now feed the module graph, but there is
  still no HTTPS requirement, semantic version solver, package publishing,
  auth, transitive dependency resolution, or package signature verification.
- JSON/SQLite persistence works for the supported QA flows, but migrations and
  long-term schema compatibility are limited to the documented `0.1.x` policy:
  additive optional/defaulted fields are supported, while renames, removals,
  required fields without defaults, type changes, and physical SQLite schema
  assumptions remain breaking/experimental.
- `index` remains declarative metadata and does not create physical indexes.
- The playground is distributed as static package assets, not hosted as a
  public web product.
- Public release validation now exists, but it should stay part of every
  release handoff so regressions are caught after upload, not only before tag.
- Backup/restore is now documented and smoked for JSON storage, but SQLite
  remains a behavioral parity backend without a stable public `nexus serve`
  selection flag.
- `v0.1.1` has completed commit/push, GitHub Actions observation, strict
  dry-run, tag/release publication, and post-release public install validation.

### 0.2.0 product focus

- Choose one durable ERP vertical slice, such as inventory plus billing or CRM
  plus orders, and make it excellent end to end.
- Implement the first SQLite/migrations MVP with schema introspection,
  migration plan/dry-run, and JSON/SQLite compatibility tests before larger
  runtime features. DONE locally in Fase 11.68 with `nexus storage-plan`,
  conservative blockers, safe table/index creation, and focused tests.
- Decide whether docs generation belongs in the CLI as a first-class command
  before expanding documentation UI in the playground.
- Improve runtime diagnostics with structured locations where feasible, so
  server/runtime errors can match lexer/parser/checker diagnostic quality.
- Decide whether to ship a hosted playground or keep the web surface as a
  package-local learning/debugging tool for another release.
- Harden the first native auth slice with rate limiting, CSRF protection for
  cookie-backed unsafe methods, hosted TLS guidance, refresh-token policy, and
  SQLite auth-store parity.
- Expand typed HIR consumption beyond identifiers and assignments, complete
  definition/use links, and start making expression validation read from HIR
  metadata as the primary path so the checker can move further away from raw
  AST and ad hoc string lookups. STARTED for model field access, model
  operation field-name arguments, static `run_workflow("Name")` literals,
  auth declaration references, and route auth guard auth references.

## Phase 1: Core stability

- Keep semantic checks in Rust as the source of truth.
- Expand tests for lexer, parser, checker, and interpreter.
- Add line/column spans to key AST nodes and semantic errors. DONE for checker
  diagnostics and literal spans.
- Stop silently skipping unknown lexer characters. DONE
- Add a small diagnostics type instead of returning plain `String`. STARTED
  with lexer/parser/checker diagnostics, playground JSON, and versioned
  multi-module diagnostics JSON for tooling, including granular v1 diagnostic
  codes and populated labels/notes/suggestions for selected high-impact
  compiler/runtime families.

## Phase 2: Type system

- Support explicit array types, for example `[string]` and `[money]`. DONE
- Support model types, for example `[Employee]`. DONE
- Validate user function parameters and return types. DONE
- Add model instance types and object literals. DONE
- Add field access for model instances, for example `customer.name`. DONE
- Add optional values and fields, for example `string?`. DONE
- Add model field defaults, for example `status: string = "active"`. DONE
- Validate all route and invoice return values. STARTED with direct route
  return contracts and required invoice fields/items.
- Add clearer rules for numeric promotion between `int` and `float`.

## Phase 3: ERP primitives

- Make `invoice` structured with line items, tax, discount, and totals. DONE
- Make `workflow` executable with step bodies. DONE
- Add model field rules such as `required`, `unique`, `min`, `max`. `default`
  is DONE for static model field values; `unique` STARTED for JSON-backed
  `Model::create()` and `Model::update()`; `index` STARTED as declarative
  scalar metadata with an OpenAPI marker; `min`/`max` STARTED for scalar
  model fields, including static default validation and JSON-backed
  `Model::create()`/`Model::update()` enforcement.
- Add route parameters such as `/employees/:id`. DONE
- Add JSON/object values for API responses. DONE for direct model instances in
  route returns.

## Phase 4: Tooling

- Add `nexus fmt`. DONE
- Add `nexus lint`. DONE
- Add `nexus repl`. DONE
- Add `nexus new <project>`. DONE
- Generate docs from `model`, `route`, `workflow`, and `invoice` declarations.
  DONE with `nexus docs [file.nx] [--output docs.md]`, validating the
  multi-module program before emitting Markdown.
- Add `nexus test`. DONE with a local-first smoke runner for `.nx` files and
  directories, using the same multi-module run pipeline as `nexus run`.
  Optional `.out` sidecars, `--update`, simple `--name <term>` filtering, and
  `--json` reports are DONE for stdout regression checks and CI/tooling
  integration. Per-case `--timeout <dur>` is DONE as a local CI guard against
  hanging tests. Per-case `--isolate-data` is DONE for temporary runtime
  storage isolation. `--jobs <n>` is DONE for bounded parallel test execution
  with deterministic reporting. Native `assert_true`, `assert_eq`,
  `assert_ne`, and `assert_contains` helpers are DONE for behavioral `.nx` test
  assertions, including optional failure messages. `--list` is DONE for
  deterministic discovery/debug output without executing cases.

Python is useful here for quick developer scripts, fixtures, and migration
tools. It should not replace the Rust core.

## Phase 5: Runtime services

- Add `nexus serve <file.nx>`. DONE
- Serve declared `route` blocks over HTTP. DONE
- Generate OpenAPI from route declarations. DONE for paths, params, model
  response schemas, optional fields, defaults, model arrays, and request bodies
  for `Model::create()`/`Model::update()`, `400` invalid-body responses for
  `Model::create()`/`Model::update()`, plus `404` responses for
  `Model::find()`/`Model::update()`/`Model::delete()` and `409` responses for
  unique conflicts using a reusable `NexusError` schema, reserved internal
  component-name validation, stable `operationId` values, stable resource
  tags and top-level tags, reusable path/query parameter components, reusable
  request body components, reusable success response components,
  grouped OpenAPI path items for multiple methods on the same route path,
  duplicate route method+path validation,
  compact golden QA coverage for the OpenAPI 1.0 contract,
  generated OpenAPI JSON parseability QA,
  generated OpenAPI minimum structure QA,
  generated OpenAPI Path Item/Operation structure QA,
  generated OpenAPI internal component reference QA,
  generated OpenAPI operationId uniqueness/tag consistency QA,
  generated OpenAPI reusable component minimum-structure QA,
  generated OpenAPI model-schema semantic QA,
  generated OpenAPI operation/component contract consistency QA,
  generated OpenAPI 1.0 coherence-suite QA,
  unique/index/min/max field extensions, plus typed
  `Model::where()`/`Model::where_not()`/
  `Model::where_in()`/`Model::where_not_in()`/
  `Model::where_not_in_optional()`/`Model::where_all()`/`Model::where_any()` array
  responses via reusable `NexusList_<Model>` schemas, total-count page response envelopes for simple, `where_in`,
  `where_not_in`, `where_not_in_optional`, `where_in_optional`, exclusion, OR, and advanced filters via reusable `NexusPage_<Model>` schemas, composite-filter extension markers,
  exclusion-filter extension markers, or-filter extension markers, pagination extension markers,
  total-count extension markers,
  ordering extension markers, optional-filter extension markers,
  in-filter extension markers,
  comparison-filter extension markers, text-filter extension markers,
  range-filter extension markers, typed/optional/defaulted query params
  including `money` and simple arrays, and `400` responses for query param
  validation errors.
- Add JSON file storage first, then SQLite. JSON FILE DONE; public
  `nexus serve --storage json|sqlite` driver selection DONE; typed create via
  `Model::create()`, typed read via `Model::find()`, typed filter via
  `Model::where()` and `Model::where_all()`, paginated list routes via
  `Model::all(limit, offset)`, `Model::where(..., limit, offset)`, and
  `Model::where_all(..., limit, offset)`, total-count paged list routes via
  `Model::page()`, `Model::where_page()`, `Model::where_in_page()`,
  `Model::where_not_page()`, `Model::where_not_in_page()`,
  `Model::where_not_in_optional_page()`, `Model::where_in_optional_page()`,
  `Model::where_any_page()` and advanced `*_page` filters,
  ordered list routes via
  `Model::all("field", "asc|desc")`, `Model::where(..., "field", "asc|desc")`,
  exclusion filters via `Model::where_not()`/`Model::where_not_in()`/
  `Model::where_not_in_optional()`, optional list filters via `Model::where_optional()`, in-list filters via
  `Model::where_in()`/`Model::where_not_in()`/
  `Model::where_not_in_optional()`/`Model::where_in_optional()`, OR filters via
  `Model::where_any()`, comparison list filters via `Model::where_compare()`,
  text list filters via `Model::where_text()` including simple case-insensitive
  operators, range filters via
  `Model::where_between()`, and
  `Model::where_all(..., "field", "asc|desc", limit, offset)`, and typed update
  via `Model::update()`, plus typed delete via `Model::delete()` STARTED.

Go is a good option if NexusLang later needs a separate high-performance API
gateway or deployment helper, but Rust can handle the first server runtime.

## Phase 5.5: Native Auth And Secure Backend

- Add `auth` declarations bound to a model identity field. DONE for the first
  JSON-backed runtime slice.
- Require auth identity fields to be `string unique`. DONE.
- Add route guards with `auth(Name)` and `auth(Name, role: "admin")`. DONE.
- Add `Auth::register()`, `Auth::login()`, `Auth::logout()`, and
  `Auth::user()` route returns. DONE.
- Store passwords with Argon2id and per-password salts. DONE.
- Issue opaque server-side session cookies and revocable bearer tokens. DONE.
- Generate OpenAPI security schemes plus `401` and `403` responses for guarded
  routes. DONE.
- Add an executable auth example in `examples/auth_secure_crm.nx`. DONE.
- Add production hardening: rate limits, CSRF tokens for cookie sessions,
  SQLite auth-store parity, reverse-proxy/TLS deployment docs, and real HTTP
  auth smoke. DONE.
- Add next auth hardening: secret rotation, password reset, MFA, and finer
  policy primitives beyond role strings.

## Phase 6: Web product

- Replace the duplicated JavaScript interpreter in the playground with the Rust
  core compiled to WebAssembly. DONE
- Keep the playground as a learning/debugging surface. DONE
- Add examples for payroll, inventory, billing, banking, e-commerce, and CRM. DONE
- Add focused playground examples for Phases 3, 4, and 5. DONE
- Surface Rust diagnostics with line/column in the UI when available. DONE
- Reduce WASM size and prefer streaming playground loading with fallback. DONE
- Generate playground docs from model, route, workflow, and invoice data. DONE

Ruby on Rails is useful if NexusLang becomes a hosted product with accounts,
projects, billing, teams, templates, and a marketplace. It is not necessary for
the language core.

## Phase 7: 1.0 target

- Stable CLI.
- Stable syntax. STARTED
  - Document the 1.0 syntax baseline in `SYNTAX_1_0.md`. DONE
  - Reject missing commas in function parameters, call arguments, static call
    arguments, and array literals. DONE
  - Reject non-`step` tokens inside workflow declarations. DONE
  - Require route paths to start with `/`. DONE
- Strong semantic checker. STARTED
  - Require non-void functions to return on every checked path. DONE
  - Reject value returns in functions without declared return type. DONE
  - Require routes to contain one direct HTTP return expression. DONE
  - Restrict route return expressions to the HTTP subset currently supported by
    the server runtime. DONE
  - Require invoices to declare `customer`, `currency`, and at least one
    structured item or `total`. DONE
  - Reject duplicate invoice fields. DONE
  - Validate model instance object literals against declared model fields. DONE
  - Allow routes to return checked model instances as JSON objects. DONE
  - Validate model field access expressions against declared model fields. DONE
  - Reject `nil` for non-optional types and allow omitted optional model
    fields. DONE
  - Validate model field defaults and fill omitted defaulted fields. DONE
  - Generate OpenAPI schemas from route return models, optional fields, and
    defaults. DONE
  - Validate initial model field constraint `unique`. STARTED for scalar
    fields in JSON-backed `Model::create()` and `Model::update()`.
  - Validate initial model field constraint `index`. STARTED for scalar and
    optional scalar fields as declarative metadata.
  - Validate initial model field constraints `min`/`max`. STARTED for string
    length, int/float numeric, money, date, optional scalar fields, and static
    defaults.
  - Centralize model operation metadata for route checking, storage dispatch,
    return typing, and OpenAPI generation. DONE with
    `ModelStaticOperation` descriptors and quality-gated documentation.
- HTTP routes. STARTED with path params and typed query params, including
  optional/defaulted query params, `money` query params, simple array query
  params, optional typed model filters, typed `where_in`, `where_not_in`,
  `where_not_in_optional`, and `where_in_optional` filters, typed exclusion filters, typed OR filters, and typed comparison/text/range
  filters, including simple case-insensitive text operators, plus explicit total-count paged list responses for simple,
  `where_in`, `where_not_in`, `where_not_in_optional`, `where_in_optional`, exclusion, OR, and advanced filters.
- CRUD over models. STARTED with typed `POST` create, typed `GET`
  find/filter/exclusion-filter/optional-filter/comparison-filter/text-filter/case-insensitive-text-filter/range-filter/
  composite-filter/or-filter/in-filter/optional-in-filter/ordered and
  paginated list, total-count page envelopes for simple, `where_in`,
  `where_not_in`, `where_not_in_optional`, `where_in_optional`, exclusion, OR, and advanced filters,
  query-driven list controls, `min`/`max` field validation in create/update,
  typed `PUT` update, and typed `DELETE` delete over JSON storage.
- OpenAPI generation. DONE for model-based response contracts, create/update
  request bodies, create/update `400` invalid-body responses,
  find/update/delete `404` responses, unique/index/min/max extensions,
  unique conflict `409` responses, reusable `NexusError` error schema, reserved
  internal component-name validation, stable `operationId` values, stable
  resource tags and top-level tags, reusable path/query parameter components,
  reusable request body components, reusable success response components,
  grouped OpenAPI path items for multiple methods on the same route path,
  duplicate route method+path validation,
  compact golden QA coverage for the OpenAPI 1.0 contract,
  generated OpenAPI JSON parseability QA,
  generated OpenAPI minimum structure QA,
  generated OpenAPI Path Item/Operation structure QA,
  generated OpenAPI internal component reference QA,
  generated OpenAPI operationId uniqueness/tag consistency QA,
  generated OpenAPI reusable component minimum-structure QA,
  generated OpenAPI model-schema semantic QA,
  generated OpenAPI operation/component contract consistency QA,
  generated OpenAPI 1.0 coherence-suite QA, plus
  where/where_not/where_in/where_not_in_optional/where_all/where_any array
  responses via reusable `NexusList_<Model>` schemas,
  total-count page response envelopes for simple, `where_in`,
  `where_not_in`, `where_not_in_optional`, `where_in_optional`, exclusion, OR, and advanced filters using reusable `NexusPage_<Model>` schemas,
  composite-filter extension markers, exclusion-filter extension markers, or-filter extension markers, pagination extension markers,
  total-count extension markers, ordering extension markers, optional-filter
  extension markers, in-filter extension markers, comparison-filter extension
  markers, text-filter extension markers, range-filter extension markers, and
  typed/optional/defaulted query params including `money` and simple arrays,
  plus `400` responses for query param validation errors.
- Playground powered by the same Rust core as the CLI.
- Documentation and examples for real ERP workflows.

## OpenAPI 1.0 release readiness

Current judgement: READY WITH RISK for an internal release candidate. The
OpenAPI 1.0 subset is implemented for the supported route/runtime shapes and
has regression QA for golden fragments, JSON parseability, root/components
structure, Path Items, operations, reusable components, internal refs,
`operationId` uniqueness, tag consistency, operation/component contract
consistency, an aggregate coherence suite, external OpenAPI 3.0 validation
via Python, and a smoke test suite against the real HTTP server.

Remaining release risks:

- `x-nexus-*` extension semantics are Nexus-specific and may be ignored by
  external tooling.
- JSON file storage is still the first backend; `index` remains declarative and
  does not create physical indexes.
- Python validator is lightweight and does not cover the full OpenAPI 3.0 spec.

Release checklist:

- DONE: OpenAPI route paths, params, responses, request bodies, reusable
  schemas and reusable components are generated for the supported HTTP subset.
- DONE: Semantic guards reject reserved internal component names and duplicate
  route method+path declarations.
- DONE: Compact OpenAPI golden snapshot covers representative 1.0 contract
  fragments.
- DONE: Internal QA covers parseability, minimum structure, reusable component
  structure, `$ref` resolution, `operationId` uniqueness and tag consistency.
- DONE: Internal QA validates operation request bodies, `200`/`201`/`400`/
  `404`/`409` responses and success schemas against the real route contracts
  and reusable components.
- DONE: The aggregate OpenAPI 1.0 coherence suite runs the central structural
  validations as one release-readiness gate.
- DONE: External OpenAPI 3.0 validation via `scripts/validate-openapi.sh`
  (Python `openapi-schema-validator` + structural checks).
- DONE: Smoke test suite via `scripts/smoke-test.sh` covering all CRUD, listagens,
  filtros (where_in, where_not, where_optional, compare, range, text, OR) e
  respostas de erro (400, 404, 409).
- DONE: Representative OpenAPI QA example file `examples/openapi_qa.nx` covering
  todos os tipos de route, filtro e query params suportados pelo alvo 1.0.
- DONE: Lightweight Python validator `scripts/validate-openapi.py` que verifica:
  - `openapi` version, `info`, `paths`, `components`
  - estrutura minima de operations (`operationId`, `responses`)
  - resolução de todos os `$ref` internos
  - schemas, parameters, requestBodies, responses
- DONE: Publish release notes documenting the supported subset and JSON storage
  limitations.
- DONE: Document version/tag policy for local release artifacts.
- DONE: Document language/runtime/storage compatibility levels.
- DONE: Document and script the GPG signing path for release artifacts.
- DONE: Add final local dry-run and Docker-based second-environment validation.
- DONE: Add strict GitHub/GPG/remote-CI release dry-run gate.
